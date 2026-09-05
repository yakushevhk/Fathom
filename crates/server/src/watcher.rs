use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

/// Filesystem Watcher configuration for proactive background autonomous operations.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    pub root_dir: PathBuf,
    pub debounce_ms: u64,
    pub auto_heal_on_compiler_error: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            root_dir: PathBuf::from("."),
            debounce_ms: 300,
            auto_heal_on_compiler_error: true,
        }
    }
}

/// Continuous background file watcher and self-healing supervisor.
pub struct FilesystemWatcher {
    config: WatcherConfig,
    tx: mpsc::Sender<PathBuf>,
    rx: tokio::sync::Mutex<Option<mpsc::Receiver<PathBuf>>>,
}

impl FilesystemWatcher {
    pub fn new(config: WatcherConfig) -> Self {
        let (tx, rx) = mpsc::channel::<PathBuf>(100);
        Self {
            config,
            tx,
            rx: tokio::sync::Mutex::new(Some(rx)),
        }
    }

    /// Access the file change notification sender to trigger watcher evaluation.
    pub fn sender(&self) -> mpsc::Sender<PathBuf> {
        self.tx.clone()
    }

    /// Start watching filesystem events in background task.
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut rx_guard = self.rx.lock().await;
        let Some(mut rx) = rx_guard.take() else {
            return Ok(());
        };
        let debounce = Duration::from_millis(self.config.debounce_ms);
        let root = self.config.root_dir.clone();
        let auto_heal = self.config.auto_heal_on_compiler_error;

        tokio::spawn(async move {
            let mut last_trigger = std::time::Instant::now();
            while let Some(changed_path) = rx.recv().await {
                if last_trigger.elapsed() < debounce {
                    continue;
                }
                last_trigger = std::time::Instant::now();

                // If auto-heal enabled, run compiler checks with bounded timeout
                if auto_heal {
                    if let Some(ext) = changed_path.extension().and_then(|e| e.to_str()) {
                        if ext == "rs" {
                            let _ = tokio::time::timeout(
                                Duration::from_secs(60),
                                tokio::process::Command::new("cargo")
                                    .arg("check")
                                    .current_dir(&root)
                                    .kill_on_drop(true)
                                    .output(),
                            )
                            .await;
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
