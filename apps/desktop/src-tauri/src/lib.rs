//! Fathom Desktop backend.
//!
//! A thin, hostile-to-failure desktop shell over the `fathom` binary:
//! it finds a `fathom` executable, spawns `fathom serve` on a loopback
//! port when needed, proxies HTTP to it, and forwards SSE event streams
//! to the UI over Tauri events.

pub mod daemon;
pub mod proxy;
pub mod types;

use daemon::DaemonManager;
use proxy::*;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

/// Tauri-managed shared state: a daemon manager that controls the fathom
/// subprocess.
pub struct AppState {
    pub daemon: DaemonManager,
}

/// Background task: poll the engine health periodically and emit events.
fn spawn_health_watcher(app: &tauri::AppHandle) {
    let handle = app.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            if let Some(s) = handle.try_state::<Mutex<AppState>>() {
                let status = s.lock().await.daemon.health().await.unwrap_or_default();
                let _ = handle.emit("daemon:status", &status);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let state = AppState {
                daemon: DaemonManager::new(handle.clone()),
            };
            app.manage(Mutex::new(state));
            spawn_health_watcher(&handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            daemon_status,
            daemon_start,
            daemon_stop,
            list_sessions,
            create_session,
            get_session,
            cancel_session,
            steer_session,
            get_session_results,
            list_agents,
            get_agent,
            list_jobs,
            create_job,
            cancel_job,
            list_memories,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}