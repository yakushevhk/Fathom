//! Post-task reflection + bounded Pattern Register (ouroboros `reflection.py`
//! inspired).
//!
//! After a non-trivial task the agent writes a short reflection and folds what
//! it learned into a bounded **Pattern Register** — a Markdown table of the form
//! `Error class | Count | Root cause | Structural fix | Status`. Rather than an
//! ever-growing log, the register is *merged* by the LLM: a new class appends a
//! row; a recurring class bumps its count and (optionally) improves the root
//! cause / fix. The register stays ≤ a fixed number of rows, so it stays useful
//! and cheap to inject into context.
//!
//! Reflections and register history are both append-only JSONL, so the review
//! trail survives restarts and is traceable back to the run that produced it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

/// Max rows in the pattern register (bounded table).
pub const MAX_PATTERN_ROWS: usize = 20;
/// Cap on the register file (chars).
pub const PATTERN_FILE_CAP: usize = 16_000;
/// Non-trivial task threshold: at least this many agent rounds/errors to bother.
pub const NONTRIVIAL_ROUNDS: usize = 15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatternRow {
    pub class: String,
    pub count: u64,
    pub root_cause: String,
    pub structural_fix: String,
    pub status: String,
}

/// One reflection record (JSONL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionRecord {
    pub ts: String,
    pub session_id: String,
    pub task_summary: String,
    pub observations: Vec<String>,
    /// Pattern-register upserts derived from this reflection.
    pub pattern_upserts: Vec<PatternRow>,
    /// Backlog candidate summaries to fold into the improvement backlog.
    pub backlog_candidates: Vec<String>,
}

/// Bounded pattern register persisted as Markdown, with append-only JSONL
/// history of every mutation.
pub struct PatternRegister {
    path: PathBuf,
    history: PathBuf,
    inner: Arc<TokioMutex<HashMap<String, PatternRow>>>,
}

impl PatternRegister {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        let base = dir.as_ref();
        Self {
            path: base.join("patterns.md"),
            history: base.join("patterns_history.jsonl"),
            inner: Arc::new(TokioMutex::new(HashMap::new())),
        }
    }

    pub async fn load(&self) -> anyhow::Result<()> {
        let raw = match tokio::fs::read_to_string(&self.path).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("read patterns {}: {e}", self.path.display())),
        };
        let mut m = self.inner.lock().await;
        for row in parse_patterns(&raw) {
            m.insert(row.class.clone(), row);
        }
        Ok(())
    }

    pub async fn rows(&self) -> Vec<PatternRow> {
        let m = self.inner.lock().await;
        let mut v: Vec<PatternRow> = m.values().cloned().collect();
        v.sort_by(|a, b| b.count.cmp(&a.count));
        v
    }

    /// Deterministic upsert (no LLM): bump count if the class exists, else add
    /// a new row. Used when no fast_llm is available; the LLM merge path is
    /// [`Self::upsert_with_llm`].
    pub async fn upsert(&self, class: &str, root_cause: &str, fix: &str) -> anyhow::Result<()> {
        let mut m = self.inner.lock().await;
        let old = m.get(class).cloned();
        if let Some(row) = m.get_mut(class) {
            row.count += 1;
            if !root_cause.is_empty() {
                row.root_cause = root_cause.to_string();
            }
            if !fix.is_empty() {
                row.structural_fix = fix.to_string();
            }
        } else {
            if m.len() >= MAX_PATTERN_ROWS && !m.contains_key(class) {
                // Evict the least-common row to keep the table bounded.
                if let Some(min_key) = m.iter().min_by_key(|(_, r)| r.count).map(|(k, _)| k.clone()) {
                    m.remove(&min_key);
                }
            }
            m.insert(
                class.to_string(),
                PatternRow {
                    class: class.to_string(),
                    count: 1,
                    root_cause: root_cause.to_string(),
                    structural_fix: fix.to_string(),
                    status: "open".to_string(),
                },
            );
        }
        let new = m.get(class).cloned().unwrap();
        let snapshot: Vec<PatternRow> = m.values().cloned().collect();
        drop(m); // release the lock before touching files
        self.persist(&snapshot).await?;
        self.record_history(old.as_ref(), &new).await
    }

    /// Append to the JSONL history (old row → new row), for traceability.
    async fn record_history(&self, old: Option<&PatternRow>, new: &PatternRow) -> anyhow::Result<()> {
        if let Some(parent) = self.history.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let rec = serde_json::json!({
            "ts": chrono::Utc::now().to_rfc3339(),
            "old": old,
            "new": new,
        });
        let mut line = serde_json::to_string(&rec)?;
        line.push('\n');
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.history)
            .await?;
        f.write_all(line.as_bytes()).await?;
        f.flush().await.ok();
        Ok(())
    }

    async fn persist(&self, rows: &[PatternRow]) -> anyhow::Result<()> {
        if let Some(p) = self.path.parent() {
            tokio::fs::create_dir_all(p).await.ok();
        }
        let mut buf = String::from("| Error class | Count | Root cause | Structural fix | Status |\n");
        buf.push_str("|---|---|---|---|---|\n");
        let mut sorted = rows.to_vec();
        sorted.sort_by(|a, b| b.count.cmp(&a.count));
        for r in sorted {
            if buf.len() > PATTERN_FILE_CAP {
                break;
            }
            buf.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                esc_cell(&r.class),
                r.count,
                esc_cell(&r.root_cause),
                esc_cell(&r.structural_fix),
                r.status
            ));
        }
        let mut f = tokio::fs::File::create(&self.path).await?;
        f.write_all(buf.as_bytes()).await?;
        f.flush().await.ok();
        Ok(())
    }
}

fn esc_cell(s: &str) -> String {
    s.replace('|', "\\|").chars().take(120).collect()
}

fn parse_patterns(raw: &str) -> Vec<PatternRow> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if !t.starts_with('|') || t.starts_with("| Error") || t.starts_with("|---") {
            continue;
        }
        let cells: Vec<&str> = t
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(|c| c.trim())
            .collect();
        if cells.len() >= 5 {
            out.push(PatternRow {
                class: cells[0].replace("\\|", "|"),
                count: cells[1].parse().unwrap_or(1),
                root_cause: cells[2].to_string(),
                structural_fix: cells[3].to_string(),
                status: cells[4].to_string(),
            });
        }
    }
    out
}

/// Append-only reflection journal.
pub struct ReflectionLog {
    path: PathBuf,
}

impl ReflectionLog {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            path: dir.as_ref().join("reflections.jsonl"),
        }
    }

    pub async fn append(&self, rec: &ReflectionRecord) -> anyhow::Result<()> {
        if let Some(p) = self.path.parent() {
            tokio::fs::create_dir_all(p).await.ok();
        }
        let mut line = serde_json::to_string(rec)?;
        line.push('\n');
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;
        f.write_all(line.as_bytes()).await?;
        f.flush().await.ok();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pattern_register_bumps_recurrence() {
        let tmp = tempfile::TempDir::new().unwrap();
        let reg = PatternRegister::new(tmp.path());
        reg.load().await.unwrap();
        reg.upsert("source_fetch_error", "", "").await.unwrap();
        reg.upsert("source_fetch_error", "", "").await.unwrap();
        assert_eq!(reg.rows().await.len(), 1);
        assert_eq!(reg.rows().await[0].count, 2);
    }

    #[tokio::test]
    async fn pattern_register_persists_and_reloads() {
        let tmp = tempfile::TempDir::new().unwrap();
        {
            let reg = PatternRegister::new(tmp.path());
            reg.load().await.unwrap();
            reg.upsert("mx_timeout", "network", "use DoH fallback").await.unwrap();
        }
        let reg = PatternRegister::new(tmp.path());
        reg.load().await.unwrap();
        let rows = reg.rows().await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].root_cause, "network");
        assert_eq!(rows[0].structural_fix, "use DoH fallback");
    }

    #[tokio::test]
    async fn bounded_at_max_rows() {
        let tmp = tempfile::TempDir::new().unwrap();
        let reg = PatternRegister::new(tmp.path());
        reg.load().await.unwrap();
        for i in 0..(MAX_PATTERN_ROWS + 5) {
            reg.upsert(&format!("class_{i}"), "", "").await.unwrap();
        }
        let rows = reg.rows().await;
        assert!(rows.len() <= MAX_PATTERN_ROWS);
    }

    #[tokio::test]
    async fn reflection_log_appends_unique_records() {
        let tmp = tempfile::TempDir::new().unwrap();
        let log = ReflectionLog::new(tmp.path());
        log.append(&ReflectionRecord {
            ts: "t".into(),
            session_id: "s".into(),
            task_summary: "q".into(),
            observations: vec!["obs".into()],
            pattern_upserts: vec![],
            backlog_candidates: vec![],
        })
        .await
        .unwrap();
        log.append(&ReflectionRecord {
            ts: "t2".into(),
            session_id: "s".into(),
            task_summary: "q".into(),
            observations: vec![],
            pattern_upserts: vec![],
            backlog_candidates: vec![],
        })
        .await
        .unwrap();
        let raw = tokio::fs::read_to_string(tmp.path().join("reflections.jsonl")).await.unwrap();
        assert_eq!(raw.lines().count(), 2);
    }
}
