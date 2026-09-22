//! Durable store: resources table + append-only event log on SQLite (WAL).
//!
//! The event log is the source of truth: every applied manifest, status
//! transition and lifecycle action is appended with a monotonically
//! increasing `seq`. Replaying it rebuilds state after a crash; scanning it
//! backs `watch` and fork-by-sequence.
//!
//! The connection is mutex-guarded (rusqlite Connection is !Sync), which
//! mechanically enforces the single-writer invariant of the AX controller.

use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::error::{AxError, AxResult};
use crate::manifest::{AxManifest, ObjectMeta};

/// One row of the durable event log.
#[derive(Debug, Clone)]
pub struct Event {
    pub seq: i64,
    /// "task" | "gateway" | "workspace" | "model"
    pub kind: String,
    pub atespace: String,
    pub name: String,
    /// "upsert" | "delete" | "phase" | "condition" | "fork"
    pub action: String,
    pub payload: serde_json::Value,
    pub created_at: String,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS resources (
    kind       TEXT NOT NULL,
    atespace   TEXT NOT NULL,
    name       TEXT NOT NULL,
    manifest   TEXT NOT NULL,
    status     TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (kind, atespace, name)
);
CREATE TABLE IF NOT EXISTS events (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT NOT NULL,
    atespace   TEXT NOT NULL,
    name       TEXT NOT NULL,
    action     TEXT NOT NULL,
    payload    TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS events_obj ON events(kind, atespace, name, seq);
CREATE TABLE IF NOT EXISTS actors (
    task_key   TEXT PRIMARY KEY,
    pid        INTEGER,
    actor_id   TEXT NOT NULL,
    log_path   TEXT NOT NULL,
    started_at TEXT NOT NULL
);
";

pub struct AxStore {
    conn: Mutex<Connection>,
    root: PathBuf,
}

impl AxStore {
    /// Open (or create) a store under `root`. `root` is the `.ax` state dir
    /// — the equivalent of AX's `AX_STATE_DIR`/PVC mount.
    pub fn open(root: impl AsRef<Path>) -> AxResult<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(root.join("logs"))?;
        std::fs::create_dir_all(root.join("workspaces"))?;
        let conn = Connection::open(root.join("store.sqlite3"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            root,
        })
    }

    /// In-memory store for tests — SQLite lives in RAM, but `root` is a
    /// real directory so log/workspace paths stay valid.
    pub fn in_memory(root: impl AsRef<Path>) -> AxResult<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(root.join("logs"))?;
        std::fs::create_dir_all(root.join("workspaces"))?;
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
            root,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn now() -> String {
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }

    // ── Resources ────────────────────────────────────────────────────────

    /// Insert or update a resource manifest (desired state).
    pub fn upsert(&self, manifest: &AxManifest) -> AxResult<()> {
        let meta = manifest.metadata();
        let kind = manifest.kind().to_lowercase();
        let doc = serde_json::to_string(manifest)?;
        let now = Self::now();
        let atespace = meta.atespace_or_default().to_string();
        let conn = self.conn.lock();
        // Preserve an existing status row across re-apply.
        let existing: Option<String> = conn
            .query_row(
                "SELECT status FROM resources WHERE kind=?1 AND atespace=?2 AND name=?3",
                params![kind, atespace, meta.name],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        conn.execute(
            "INSERT INTO resources (kind,atespace,name,manifest,status,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?6)
             ON CONFLICT(kind,atespace,name)
             DO UPDATE SET manifest=excluded.manifest, updated_at=excluded.updated_at",
            params![
                kind,
                atespace,
                meta.name,
                doc,
                existing.unwrap_or_default(),
                now
            ],
        )?;
        Ok(())
    }

    /// Load a manifest by identity.
    pub fn get(&self, kind: &str, atespace: &str, name: &str) -> AxResult<AxManifest> {
        let kind = kind.to_lowercase();
        let conn = self.conn.lock();
        let doc: String = conn
            .query_row(
                "SELECT manifest FROM resources WHERE kind=?1 AND atespace=?2 AND name=?3",
                params![kind, atespace, name],
                |r| r.get(0),
            )
            .map_err(|_| AxError::NotFound {
                kind: kind_static(&kind),
                name: name.to_string(),
                atespace: atespace.to_string(),
            })?;
        Ok(serde_json::from_str(&doc)?)
    }

    /// List manifests of a kind within an atespace ("" = all atespaces).
    pub fn list(&self, kind: &str, atespace: &str) -> AxResult<Vec<AxManifest>> {
        let kind = kind.to_lowercase();
        let conn = self.conn.lock();
        let docs: Vec<String> = if atespace.is_empty() {
            let mut stmt =
                conn.prepare("SELECT manifest FROM resources WHERE kind=?1 ORDER BY name")?;
            let out = stmt
                .query_map(params![kind], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>();
            out?
        } else {
            let mut stmt = conn.prepare(
                "SELECT manifest FROM resources WHERE kind=?1 AND atespace=?2 ORDER BY name",
            )?;
            let out = stmt
                .query_map(params![kind, atespace], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>();
            out?
        };
        docs.iter()
            .map(|d| serde_json::from_str(d).map_err(AxError::from))
            .collect()
    }

    /// Two-phase delete entry point: mark a task Terminating; the
    /// controller's reconciler finishes teardown and calls `delete`.
    pub fn mark_terminating(&self, atespace: &str, name: &str) -> AxResult<()> {
        let mut task = self.get_task(atespace, name)?;
        task.status.phase = crate::manifest::phase::TERMINATING.to_string();
        self.save_status(&task)?;
        self.append_event(
            "task",
            &task.metadata,
            "phase",
            &serde_json::json!({"phase": "Terminating"}),
        )?;
        Ok(())
    }

    /// Remove a resource row entirely (post-teardown).
    /// Returns false when it did not exist.
    pub fn delete(&self, kind: &str, atespace: &str, name: &str) -> AxResult<bool> {
        let kind = kind.to_lowercase();
        let conn = self.conn.lock();
        let n = conn.execute(
            "DELETE FROM resources WHERE kind=?1 AND atespace=?2 AND name=?3",
            params![kind, atespace, name],
        )?;
        if n > 0 && kind == "task" {
            conn.execute(
                "DELETE FROM actors WHERE task_key=?1",
                params![format!("{atespace}/{name}")],
            )?;
        }
        Ok(n > 0)
    }

    // ── Task status ──────────────────────────────────────────────────────

    /// Persist the status subresource of a stored task. Status is kept both
    /// in its own column (cheap phase queries) and inside the serialized
    /// manifest so `get`/`list` round-trip the full object.
    pub fn save_status(&self, task: &crate::manifest::Task) -> AxResult<()> {
        let status = serde_json::to_string(&task.status)?;
        let doc = serde_json::to_string(&AxManifest::Task(task.clone()))?;
        self.conn.lock().execute(
            "UPDATE resources SET manifest=?3, status=?4, updated_at=?5
             WHERE kind='task' AND atespace=?1 AND name=?2",
            params![
                task.metadata.atespace_or_default(),
                task.metadata.name,
                doc,
                status,
                Self::now()
            ],
        )?;
        Ok(())
    }

    /// Load a task — the `status` column (written by `save_status`) is
    /// authoritative and overlaid onto the manifest, so status survives
    /// re-applies that carry an empty status in the spec document.
    pub fn get_task(&self, atespace: &str, name: &str) -> AxResult<crate::manifest::Task> {
        match self.get("task", atespace, name)? {
            AxManifest::Task(mut t) => {
                if let Some(status) = self.task_status(atespace, name)? {
                    t.status = status;
                }
                Ok(t)
            }
            _ => Err(AxError::NotFound {
                kind: "task",
                name: name.into(),
                atespace: atespace.into(),
            }),
        }
    }

    /// List tasks with their authoritative status overlaid.
    pub fn list_tasks(&self, atespace: &str) -> AxResult<Vec<crate::manifest::Task>> {
        self.list("task", atespace)?
            .into_iter()
            .filter_map(|m| match m {
                AxManifest::Task(t) => {
                    let meta = t.metadata.clone();
                    match self.task_status(meta.atespace_or_default(), &meta.name) {
                        Ok(Some(status)) => {
                            let mut t = t;
                            t.status = status;
                            Some(Ok(t))
                        }
                        Ok(None) => Some(Ok(t)),
                        Err(e) => Some(Err(e)),
                    }
                }
                _ => None,
            })
            .collect()
    }

    /// The status column for a task, if it has ever been written.
    fn task_status(
        &self,
        atespace: &str,
        name: &str,
    ) -> AxResult<Option<crate::manifest::TaskStatus>> {
        let conn = self.conn.lock();
        let raw: Option<String> = conn
            .query_row(
                "SELECT status FROM resources WHERE kind='task' AND atespace=?1 AND name=?2",
                params![atespace, name],
                |r| r.get(0),
            )
            .ok()
            .flatten();
        match raw {
            Some(s) if !s.is_empty() => Ok(Some(serde_json::from_str(&s).unwrap_or_default())),
            _ => Ok(None),
        }
    }

    // ── Actor registry ───────────────────────────────────────────────────

    pub fn record_actor(
        &self,
        task_key: &str,
        pid: Option<i64>,
        actor_id: &str,
        log_path: &Path,
    ) -> AxResult<()> {
        self.conn.lock().execute(
            "INSERT INTO actors (task_key,pid,actor_id,log_path,started_at)
             VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(task_key) DO UPDATE SET pid=excluded.pid, actor_id=excluded.actor_id,
                 log_path=excluded.log_path, started_at=excluded.started_at",
            params![
                task_key,
                pid,
                actor_id,
                log_path.to_string_lossy(),
                Self::now()
            ],
        )?;
        Ok(())
    }

    /// Returns (pid, actor_id, log_path) for a task key.
    pub fn get_actor(&self, task_key: &str) -> AxResult<Option<(Option<i64>, String, PathBuf)>> {
        let conn = self.conn.lock();
        let row = conn
            .query_row(
                "SELECT pid, actor_id, log_path FROM actors WHERE task_key=?1",
                params![task_key],
                |r| {
                    Ok((
                        r.get::<_, Option<i64>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .ok();
        Ok(row.map(|(pid, id, lp)| (pid, id, PathBuf::from(lp))))
    }

    pub fn clear_actor_pid(&self, task_key: &str) -> AxResult<()> {
        self.conn.lock().execute(
            "UPDATE actors SET pid=NULL WHERE task_key=?1",
            params![task_key],
        )?;
        Ok(())
    }

    // ── Event log ────────────────────────────────────────────────────────

    /// Append an event; returns its assigned sequence number.
    pub fn append_event(
        &self,
        kind: &str,
        meta: &ObjectMeta,
        action: &str,
        payload: &serde_json::Value,
    ) -> AxResult<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO events (kind,atespace,name,action,payload,created_at)
             VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                kind,
                meta.atespace_or_default(),
                meta.name,
                action,
                serde_json::to_string(payload)?,
                Self::now()
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Scan events for one object starting at `from_seq` (inclusive).
    pub fn scan_events(
        &self,
        kind: &str,
        atespace: &str,
        name: &str,
        from_seq: i64,
    ) -> AxResult<Vec<Event>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq,kind,atespace,name,action,payload,created_at
             FROM events WHERE kind=?1 AND atespace=?2 AND name=?3 AND seq>=?4
             ORDER BY seq",
        )?;
        let rows = stmt
            .query_map(
                params![kind.to_lowercase(), atespace, name, from_seq],
                event_from_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Scan all events (debugging / `ax events`).
    pub fn scan_all(&self, from_seq: i64, limit: i64) -> AxResult<Vec<Event>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq,kind,atespace,name,action,payload,created_at
             FROM events WHERE seq>=?1 ORDER BY seq LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![from_seq, limit], event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn latest_seq(&self) -> AxResult<i64> {
        Ok(self
            .conn
            .lock()
            .query_row("SELECT COALESCE(MAX(seq),0) FROM events", [], |r| r.get(0))?)
    }
}

fn event_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let payload_str: String = r.get(5)?;
    Ok(Event {
        seq: r.get(0)?,
        kind: r.get(1)?,
        atespace: r.get(2)?,
        name: r.get(3)?,
        action: r.get(4)?,
        payload: serde_json::from_str(&payload_str)
            .unwrap_or(serde_json::Value::String(payload_str)),
        created_at: r.get(6)?,
    })
}

fn kind_static(kind: &str) -> &'static str {
    match kind {
        "task" => "task",
        "gateway" => "gateway",
        "workspace" => "workspace",
        "model" => "model",
        _ => "resource",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_static_recognizes_known_kinds() {
        for k in ["task", "workspace", "gateway", "model"] {
            assert_eq!(kind_static(k), k);
        }
        // Unknown kinds get a stable fallback name.
        assert_eq!(kind_static("SomethingElse"), "resource");
    }

    #[test]
    fn now_returns_nonempty_timestamp() {
        assert!(!AxStore::now().is_empty());
    }
}
