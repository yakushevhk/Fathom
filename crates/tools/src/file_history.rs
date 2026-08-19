use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Unique identifier for a snapshot checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SnapshotId(String);

impl SnapshotId {
    fn new() -> Self {
        Self(uuid::Uuid::now_v7().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single file snapshot entry within a checkpoint.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub file_path: PathBuf,
    pub content_hash: String,
    pub backup_path: PathBuf,
    pub timestamp: DateTime<Utc>,
}

/// A named checkpoint that groups multiple file snapshots together.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub id: SnapshotId,
    pub snapshots: Vec<Snapshot>,
    pub created_at: DateTime<Utc>,
}

const MAX_HISTORY_SIZE: usize = 100;

/// Manages file edit history with snapshot-based checkpointing.
///
/// Tracks which files have been edited and allows creating named checkpoints
/// (snapshots of all tracked files at a point in time). Supports rewinding
/// to any previous checkpoint to restore file contents.
pub struct FileHistory {
    /// Ring buffer of checkpoints, capped at MAX_HISTORY_SIZE.
    checkpoints: VecDeque<Checkpoint>,
    /// Directory where backup files are stored.
    backup_dir: PathBuf,
    /// Set of file paths currently being tracked for the next checkpoint.
    tracked_files: Vec<PathBuf>,
}

impl FileHistory {
    /// Create a new FileHistory with backups stored under `base_dir/.file_history/`.
    pub fn new(base_dir: PathBuf) -> Self {
        let backup_dir = base_dir.join(".file_history");
        Self {
            checkpoints: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            backup_dir,
            tracked_files: Vec::new(),
        }
    }

    /// Mark a file path as tracked so it will be included in the next snapshot.
    ///
    /// Call this *before* editing a file. The actual backup content is captured
    /// at `make_snapshot` time to avoid redundant reads.
    pub fn track_edit(&mut self, path: &Path) -> Result<()> {
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
        if !self.tracked_files.contains(&canonical) {
            self.tracked_files.push(canonical);
        }
        Ok(())
    }

    /// Create a checkpoint (named snapshot) of all currently tracked files.
    ///
    /// Reads each tracked file, computes a content hash, writes a backup copy,
    /// and records the snapshot. Returns the checkpoint ID.
    pub fn make_snapshot(&mut self) -> Result<SnapshotId> {
        if self.tracked_files.is_empty() {
            anyhow::bail!("No files tracked for snapshot");
        }

        std::fs::create_dir_all(&self.backup_dir)
            .context("Failed to create backup directory")?;

        let checkpoint_id = SnapshotId::new();
        let timestamp = Utc::now();
        let mut snapshots = Vec::new();

        for tracked_path in &self.tracked_files {
            if !tracked_path.exists() {
                // File was deleted or never existed — record the state but skip backup.
                snapshots.push(Snapshot {
                    file_path: tracked_path.clone(),
                    content_hash: String::new(),
                    backup_path: PathBuf::new(),
                    timestamp,
                });
                continue;
            }

            let content = std::fs::read(tracked_path)
                .with_context(|| format!("Failed to read {}", tracked_path.display()))?;

            let content_hash = blake3_hash(&content);

            let backup_name = format!(
                "{}_{}_{}",
                checkpoint_id.as_str(),
                content_hash,
                sanitize_filename(tracked_path)
            );
            let backup_path = self.backup_dir.join(&backup_name);

            std::fs::write(&backup_path, &content)
                .with_context(|| format!("Failed to write backup to {}", backup_path.display()))?;

            snapshots.push(Snapshot {
                file_path: tracked_path.clone(),
                content_hash,
                backup_path,
                timestamp,
            });
        }

        let checkpoint = Checkpoint {
            id: checkpoint_id.clone(),
            snapshots,
            created_at: timestamp,
        };

        // Ring buffer eviction: drop oldest if at capacity.
        if self.checkpoints.len() >= MAX_HISTORY_SIZE {
            self.checkpoints.pop_front();
        }
        self.checkpoints.push_back(checkpoint);

        // Clear tracked files after snapshot.
        self.tracked_files.clear();

        Ok(checkpoint_id)
    }

    /// Restore all files in the checkpoint identified by `snapshot_id`.
    ///
    /// Each file is restored from its backup copy. If a backup file is missing,
    /// that file is skipped with a warning.
    pub fn rewind(&self, snapshot_id: &SnapshotId) -> Result<()> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.id == *snapshot_id)
            .with_context(|| format!("Snapshot not found: {snapshot_id}"))?;

        for snap in &checkpoint.snapshots {
            if snap.backup_path.as_os_str().is_empty() {
                // File was deleted at snapshot time — remove it on rewind.
                if snap.file_path.exists() {
                    std::fs::remove_file(&snap.file_path).with_context(|| {
                        format!("Failed to remove {}", snap.file_path.display())
                    })?;
                }
                continue;
            }

            if !snap.backup_path.exists() {
                tracing::warn!(
                    "Backup file missing for {}, skipping restore",
                    snap.file_path.display()
                );
                continue;
            }

            let content = std::fs::read(&snap.backup_path).with_context(|| {
                format!("Failed to read backup {}", snap.backup_path.display())
            })?;

            if let Some(parent) = snap.file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&snap.file_path, &content).with_context(|| {
                format!("Failed to restore {}", snap.file_path.display())
            })?;
        }

        Ok(())
    }

    /// List all available checkpoint IDs (oldest first).
    pub fn list_checkpoints(&self) -> Vec<&SnapshotId> {
        self.checkpoints.iter().map(|c| &c.id).collect()
    }

    /// Get a checkpoint by ID.
    pub fn get_checkpoint(&self, id: &SnapshotId) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.id == *id)
    }

    /// Number of checkpoints currently stored.
    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    /// Whether there are no checkpoints.
    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }

    /// Number of files currently tracked (pending snapshot).
    pub fn tracked_count(&self) -> usize {
        self.tracked_files.len()
    }

    /// Clear all tracked files without creating a snapshot.
    pub fn clear_tracked(&mut self) {
        self.tracked_files.clear();
    }
}

/// Compute a BLAKE3-like hash using a fast non-crypto hash for content fingerprinting.
fn blake3_hash(data: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Sanitize a path for use as a filename component.
fn sanitize_filename(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "_")
        .replace('\\', "_")
        .replace(':', "_")
        .replace("..", "__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_history_is_empty() {
        let tmp = TempDir::new().unwrap();
        let history = FileHistory::new(tmp.path().to_path_buf());
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert_eq!(history.tracked_count(), 0);
    }

    #[test]
    fn test_track_and_snapshot() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "hello world").unwrap();

        let mut history = FileHistory::new(tmp.path().to_path_buf());
        history.track_edit(&file).unwrap();
        assert_eq!(history.tracked_count(), 1);

        let snap_id = history.make_snapshot().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history.tracked_count(), 0);

        let checkpoint = history.get_checkpoint(&snap_id).unwrap();
        assert_eq!(checkpoint.snapshots.len(), 1);
        assert_eq!(checkpoint.snapshots[0].file_path, file.canonicalize().unwrap());
        assert!(!checkpoint.snapshots[0].content_hash.is_empty());
        assert!(checkpoint.snapshots[0].backup_path.exists());
    }

    #[test]
    fn test_rewind_restores_content() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("data.txt");
        std::fs::write(&file, "original").unwrap();

        let mut history = FileHistory::new(tmp.path().to_path_buf());
        history.track_edit(&file).unwrap();
        let snap_id = history.make_snapshot().unwrap();

        // Modify the file.
        std::fs::write(&file, "modified").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "modified");

        // Rewind restores original.
        history.rewind(&snap_id).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "original");
    }

    #[test]
    fn test_multiple_files_in_one_checkpoint() {
        let tmp = TempDir::new().unwrap();
        let f1 = tmp.path().join("a.txt");
        let f2 = tmp.path().join("b.txt");
        std::fs::write(&f1, "aaa").unwrap();
        std::fs::write(&f2, "bbb").unwrap();

        let mut history = FileHistory::new(tmp.path().to_path_buf());
        history.track_edit(&f1).unwrap();
        history.track_edit(&f2).unwrap();
        let snap_id = history.make_snapshot().unwrap();

        std::fs::write(&f1, "changed_a").unwrap();
        std::fs::write(&f2, "changed_b").unwrap();

        history.rewind(&snap_id).unwrap();
        assert_eq!(std::fs::read_to_string(&f1).unwrap(), "aaa");
        assert_eq!(std::fs::read_to_string(&f2).unwrap(), "bbb");
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("evict.txt");
        std::fs::write(&file, "v0").unwrap();

        let mut history = FileHistory::new(tmp.path().to_path_buf());

        // Create MAX_HISTORY_SIZE + 1 snapshots to trigger eviction.
        for i in 0..=MAX_HISTORY_SIZE {
            std::fs::write(&file, format!("v{i}")).unwrap();
            history.track_edit(&file).unwrap();
            history.make_snapshot().unwrap();
        }

        // Only MAX_HISTORY_SIZE checkpoints should remain.
        assert_eq!(history.len(), MAX_HISTORY_SIZE);
    }

    #[test]
    fn test_snapshot_with_no_tracked_files_fails() {
        let tmp = TempDir::new().unwrap();
        let mut history = FileHistory::new(tmp.path().to_path_buf());
        assert!(history.make_snapshot().is_err());
    }

    #[test]
    fn test_rewind_nonexistent_snapshot_fails() {
        let tmp = TempDir::new().unwrap();
        let history = FileHistory::new(tmp.path().to_path_buf());
        let fake_id = SnapshotId("nonexistent".to_string());
        assert!(history.rewind(&fake_id).is_err());
    }

    #[test]
    fn test_deduplicate_tracked_files() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("dedup.txt");
        std::fs::write(&file, "x").unwrap();

        let mut history = FileHistory::new(tmp.path().to_path_buf());
        history.track_edit(&file).unwrap();
        history.track_edit(&file).unwrap();
        assert_eq!(history.tracked_count(), 1);
    }

    #[test]
    fn test_list_checkpoints_order() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("order.txt");

        let mut history = FileHistory::new(tmp.path().to_path_buf());
        let mut ids = Vec::new();

        for i in 0..3 {
            std::fs::write(&file, format!("v{i}")).unwrap();
            history.track_edit(&file).unwrap();
            ids.push(history.make_snapshot().unwrap());
        }

        let listed: Vec<&str> = history
            .list_checkpoints()
            .iter()
            .map(|id| id.as_str())
            .collect();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0], ids[0].as_str());
        assert_eq!(listed[2], ids[2].as_str());
    }
}
