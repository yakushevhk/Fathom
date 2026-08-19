//! Durable background-job registry.
//!
//! Jobs survive process restarts: the registry lives in its own SQLite
//! database (default `~/.parallel-research/jobs.db`), and the runner
//! (`parallel-research job-run <id>`) updates its row as attempts start,
//! fail and complete. Failed attempts are retried with an augmented task
//! that carries the previous error, so the agent can diagnose and fix its
//! own failure.

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

/// Environment variable overriding the jobs registry location.
pub const JOBS_DB_ENV: &str = "PR_JOBS_DB";
/// Environment variable overriding the per-job workspace root.
pub const JOBS_DIR_ENV: &str = "PR_JOBS_DIR";

/// Global jobs registry: `~/.parallel-research/jobs.db` unless the
/// `PR_JOBS_DB` environment variable points elsewhere.
pub fn default_jobs_db_path() -> std::path::PathBuf {
    if let Ok(p) = std::env::var(JOBS_DB_ENV) {
        return std::path::PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".parallel-research")
        .join("jobs.db")
}

/// Root directory holding one workspace per job:
/// `~/.parallel-research/jobs/<job-id>` unless `PR_JOBS_DIR` overrides it.
pub fn default_jobs_root() -> std::path::PathBuf {
    if let Ok(p) = std::env::var(JOBS_DIR_ENV) {
        return std::path::PathBuf::from(p);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".parallel-research")
        .join("jobs")
}

/// Check whether a process is alive (`kill -0`).
pub fn pid_alive(pid: i64) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Best-effort SIGTERM of a process id.
pub fn terminate_pid(pid: i64) {
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
}

/// Spawn `<exe> job-run <job_id>` fully detached: stdin closed, stdout and
/// stderr appended to `log_path` when given, and on unix placed in its own
/// session (`setsid`) so the job survives the submitting terminal closing.
/// Returns the child pid.
pub fn spawn_detached_runner(
    exe: &Path,
    job_id: &str,
    log_path: Option<&Path>,
) -> std::io::Result<u32> {
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("job-run").arg(job_id).stdin(std::process::Stdio::null());
    if let Some(log) = log_path {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)?;
        let err = file.try_clone()?;
        cmd.stdout(file).stderr(err);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    let child = cmd.spawn()?;
    Ok(child.id())
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<JobRow> {
    Ok(JobRow {
        id: r.get(0)?,
        task: r.get(1)?,
        status: r.get(2)?,
        attempt: r.get(3)?,
        max_attempts: r.get(4)?,
        output_dir: r.get(5)?,
        error: r.get(6)?,
        pid: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        started_at: r.get(10)?,
        completed_at: r.get(11)?,
    })
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JobRow {
    pub id: String,
    pub task: String,
    pub status: String,
    pub attempt: i64,
    pub max_attempts: i64,
    pub output_dir: String,
    pub error: Option<String>,
    pub pid: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

impl JobRow {
    pub fn is_terminal(&self) -> bool {
        matches!(self.status.as_str(), "completed" | "failed" | "cancelled")
    }
}

pub struct JobsDb {
    conn: Mutex<Connection>,
}

impl JobsDb {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;",
        )?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    pub fn in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS jobs (
                id            TEXT PRIMARY KEY,
                task          TEXT NOT NULL,
                status        TEXT NOT NULL DEFAULT 'queued',
                attempt       INTEGER NOT NULL DEFAULT 0,
                max_attempts  INTEGER NOT NULL DEFAULT 1,
                output_dir    TEXT NOT NULL,
                error         TEXT,
                pid           INTEGER,
                created_at    TEXT NOT NULL,
                updated_at    TEXT NOT NULL,
                started_at    TEXT,
                completed_at  TEXT
            )",
            [],
        )?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs (status)", [])?;
        Ok(())
    }

    const SELECT_COLS: &'static str = "SELECT id, task, status, attempt, max_attempts, \
        output_dir, error, pid, created_at, updated_at, started_at, completed_at FROM jobs";

    pub fn create(&self, task: &str, max_attempts: i64, output_dir: &str) -> anyhow::Result<JobRow> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO jobs (id, task, status, attempt, max_attempts, output_dir, created_at, updated_at)
             VALUES (?1, ?2, 'queued', 0, ?3, ?4, ?5, ?5)",
            params![id, task, max_attempts, output_dir, now],
        )?;
        drop(conn);
        self.get(&id)?
            .ok_or_else(|| anyhow::anyhow!("job row vanished after insert"))
    }

    pub fn get(&self, id: &str) -> anyhow::Result<Option<JobRow>> {
        let conn = self.conn.lock().unwrap();
        let exact = conn
            .prepare(&format!("{} WHERE id = ?1", Self::SELECT_COLS))?
            .query_row(params![id], map_row)
            .optional()?;
        if exact.is_some() {
            return Ok(exact);
        }
        let mut stmt = conn.prepare(&format!("{} WHERE id LIKE ?1", Self::SELECT_COLS))?;
        let mut rows = stmt.query_map(params![format!("{id}%")], map_row)?;
        match rows.next().transpose()? {
            Some(row) => {
                if rows.next().is_some() {
                    anyhow::bail!("ambiguous job id prefix: {id}");
                }
                Ok(Some(row))
            }
            None => Ok(None),
        }
    }

    pub fn list(&self) -> anyhow::Result<Vec<JobRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("{} ORDER BY created_at DESC", Self::SELECT_COLS))?;
        let rows = stmt
            .query_map([], map_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_output_dir(&self, id: &str, dir: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET output_dir=?2, updated_at=?3 WHERE id=?1",
            params![id, dir, now],
        )?;
        Ok(())
    }

    pub fn mark_running(&self, id: &str, attempt: i64, pid: i64) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status='running', attempt=?2, pid=?3, \
             started_at=COALESCE(started_at, ?4), updated_at=?4 WHERE id=?1",
            params![id, attempt, pid, now],
        )?;
        Ok(())
    }

    pub fn mark_completed(&self, id: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status='completed', error=NULL, pid=NULL, \
             completed_at=?2, updated_at=?2 WHERE id=?1",
            params![id, now],
        )?;
        Ok(())
    }

    pub fn mark_failed(&self, id: &str, error: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status='failed', error=?2, pid=NULL, \
             completed_at=?3, updated_at=?3 WHERE id=?1",
            params![id, error, now],
        )?;
        Ok(())
    }

    pub fn record_attempt_error(&self, id: &str, error: &str) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET error=?2, updated_at=?3 WHERE id=?1",
            params![id, error, now],
        )?;
        Ok(())
    }

    pub fn mark_cancelled(&self, id: &str) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE jobs SET status='cancelled', pid=NULL, completed_at=?2, updated_at=?2 \
             WHERE id=?1 AND status IN ('queued', 'running')",
            params![id, now],
        )?;
        Ok(n > 0)
    }

    /// Reset a job in a terminal state (failed/cancelled/completed) back to
    /// `queued` so it can be re-run. Returns true if a row was reset.
    pub fn reset_for_rerun(&self, id: &str) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE jobs SET status='queued', attempt=0, error=NULL, pid=NULL, \
             completed_at=NULL, updated_at=?2 \
             WHERE id=?1 AND status IN ('failed','cancelled','completed')",
            params![id, now],
        )?;
        Ok(n > 0)
    }

    /// Reset a job still marked `running` whose recorded pid matches
    /// `dead_pid` (the caller must have verified the pid is gone). Guards
    /// against racing a live runner.
    pub fn reset_running_with_pid(&self, id: &str, dead_pid: i64) -> anyhow::Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        let n = conn.execute(
            "UPDATE jobs SET status='queued', attempt=0, error=NULL, pid=NULL, \
             completed_at=NULL, updated_at=?3 \
             WHERE id=?1 AND status='running' AND pid=?2",
            params![id, dead_pid, now],
        )?;
        Ok(n > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get_roundtrip() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("do the thing", 3, "/tmp/job-x").unwrap();
        assert_eq!(job.status, "queued");
        assert_eq!(job.attempt, 0);
        assert_eq!(job.max_attempts, 3);

        let fetched = db.get(&job.id).unwrap().unwrap();
        assert_eq!(fetched.task, "do the thing");
        assert!(!fetched.is_terminal());
    }

    #[test]
    fn get_by_unique_prefix() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("t", 1, "/tmp").unwrap();
        let prefix = &job.id[..8];
        let fetched = db.get(prefix).unwrap().unwrap();
        assert_eq!(fetched.id, job.id);
        assert!(db.get("zzz").unwrap().is_none());
    }

    #[test]
    fn status_transitions() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("t", 2, "/tmp").unwrap();

        db.mark_running(&job.id, 1, 4242).unwrap();
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.status, "running");
        assert_eq!(j.pid, Some(4242));
        assert!(j.started_at.is_some());

        db.record_attempt_error(&job.id, "boom").unwrap();
        db.mark_running(&job.id, 2, 4243).unwrap();
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.attempt, 2);
        // started_at keeps the first attempt's timestamp.
        assert!(j.error.is_some());

        db.mark_completed(&job.id).unwrap();
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.status, "completed");
        assert!(j.error.is_none());
        assert!(j.pid.is_none());
        assert!(j.completed_at.is_some());
        assert!(j.is_terminal());
    }

    #[test]
    fn cancel_only_active_jobs() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("t", 1, "/tmp").unwrap();
        assert!(db.mark_cancelled(&job.id).unwrap());
        // Cancelling again is a no-op.
        assert!(!db.mark_cancelled(&job.id).unwrap());
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.status, "cancelled");

        // Completed jobs cannot be cancelled.
        let done = db.create("t2", 1, "/tmp").unwrap();
        db.mark_completed(&done.id).unwrap();
        assert!(!db.mark_cancelled(&done.id).unwrap());
    }

    #[test]
    fn list_orders_newest_first() {
        let db = JobsDb::in_memory().unwrap();
        let a = db.create("first", 1, "/tmp").unwrap();
        let b = db.create("second", 1, "/tmp").unwrap();
        let list = db.list().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }

    #[test]
    fn set_output_dir_updates_row() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("t", 1, "").unwrap();
        assert_eq!(job.output_dir, "");
        db.set_output_dir(&job.id, "/tmp/job-dir").unwrap();
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.output_dir, "/tmp/job-dir");
    }

    #[test]
    fn reset_for_rerun_only_terminal_states() {
        let db = JobsDb::in_memory().unwrap();
        let done = db.create("done", 1, "/tmp/a").unwrap();
        db.mark_running(&done.id, 1, 42).unwrap();
        db.mark_completed(&done.id).unwrap();
        let running = db.create("running", 1, "/tmp/b").unwrap();
        db.mark_running(&running.id, 1, 43).unwrap();

        assert!(db.reset_for_rerun(&done.id).unwrap());
        let j = db.get(&done.id).unwrap().unwrap();
        assert_eq!(j.status, "queued");
        assert_eq!(j.attempt, 0);
        assert!(j.error.is_none());
        assert!(j.completed_at.is_none());

        assert!(!db.reset_for_rerun(&running.id).unwrap());
        assert_eq!(db.get(&running.id).unwrap().unwrap().status, "running");
    }

    #[test]
    fn reset_running_requires_matching_pid() {
        let db = JobsDb::in_memory().unwrap();
        let job = db.create("t", 2, "/tmp/x").unwrap();
        db.mark_running(&job.id, 1, 999).unwrap();

        assert!(!db.reset_running_with_pid(&job.id, 111).unwrap());
        assert_eq!(db.get(&job.id).unwrap().unwrap().status, "running");

        assert!(db.reset_running_with_pid(&job.id, 999).unwrap());
        let j = db.get(&job.id).unwrap().unwrap();
        assert_eq!(j.status, "queued");
        assert!(j.pid.is_none());
    }

    #[test]
    fn open_migrates_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.db");
        let db = JobsDb::open(&path).unwrap();
        let job = db.create("persistent", 2, "/tmp").unwrap();
        drop(db);

        let db2 = JobsDb::open(&path).unwrap();
        let fetched = db2.get(&job.id).unwrap().unwrap();
        assert_eq!(fetched.task, "persistent");
    }
}
