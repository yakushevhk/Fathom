//! Task-tree blackboard for coordinating a swarm of sub-agents (ouroboros
//! `task_tree_ledger.py` inspiration).
//!
//! Unlike the per-agent transcript (private to one agent), the blackboard is a
//! **shared, durable journal for the whole task tree rooted at one session**.
//! Coordinator and every sub-agent read from / write to the same append-only
//! JSONL vector. Its two jobs:
//!
//! 1. **Coordination records** (`contract`, `decision`, `fact`, `note`) keep a
//!    shared picture of what the swarm decided and discovered, independent of
//!    any single agent's context.
//! 2. **Typed child→parent beacons** (`partial_finding`, `milestone`,
//!    `blocker`, `question`, `interface_contract`, `delegation_constraint`)
//!    carry high-signal, attention-worthy signals. A parent can ask "what did
//!    my children flag since time T?" and act on blockers/milestones without
//!    waiting for full completion (the "letters home" pattern).
//!
//! The ledger is append-only and size-bounded: text is capped per row and the
//! on-disk file stops accepting writes past a hard byte bound, so a chatty
//! children swarm cannot balloon disk. Consumers resolve "current" state by
//! ordering on `ts` (append order) and reading the tail.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// Coordination record kinds (shared, non-blocking).
pub const COORDINATION_KINDS: &[&str] = &["contract", "decision", "fact", "note"];
/// Beacon kinds (child→parent; `blocker`/`question`/`interface_contract`/
/// `delegation_constraint` need parent attention now).
pub const BEACON_KINDS: &[&str] = &[
    "milestone",
    "partial_finding",
    "blocker",
    "question",
    "interface_contract",
    "delegation_constraint",
];
/// Beacon kinds that must wake the parent out of a sliced wait immediately.
pub const ATTENTION_KINDS: &[&str] = &[
    "blocker",
    "question",
    "interface_contract",
    "delegation_constraint",
];

/// Delegation constraint directives the parent can act on.
pub const DELEGATION_CONSTRAINT_DIRECTIVES: &[&str] =
    &["halt_fanout", "cap_children", "require_lane", "block_surface"];

/// Hard cap on the text of any single row, mirroring ouroboros `_MAX_TEXT_CHARS`.
pub const MAX_TEXT_CHARS: usize = 4000;
/// Hard cap on the ledger file size before writes are refused. Bulk findings
/// belong in artifacts/report, not here.
pub const MAX_LEDGER_BYTES: u64 = 2 * 1024 * 1024;

/// One append-only row in the tree ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeRow {
    /// Unix epoch seconds of append.
    pub ts: u64,
    /// One of [`COORDINATION_KINDS`] or [`BEACON_KINDS`].
    pub kind: String,
    /// Human-readable content (capped at [`MAX_TEXT_CHARS`]).
    pub text: String,
    pub task_id: String,
    /// Role of the writer (`coordinator`, `researcher`, …).
    pub role: String,
    /// True for beacons that require the parent's attention now.
    #[serde(default)]
    pub needs_parent_attention: bool,
    /// Optional structured payload (e.g. a delegation constraint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl TreeRow {
    pub fn is_attention_kind(&self) -> bool {
        ATTENTION_KINDS.contains(&self.kind.as_str())
    }
}

/// In-memory view: all rows in append order, plus a size accumulator.
#[derive(Debug, Default)]
struct LedgerState {
    rows: Vec<TreeRow>,
    bytes: u64,
}

/// Shared, append-only tree ledger for one session root.
#[derive(Clone)]
pub struct TaskTreeLedger {
    path: PathBuf,
    state: Arc<Mutex<LedgerState>>,
}

impl TaskTreeLedger {
    /// Open the ledger for a session at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: Arc::new(Mutex::new(LedgerState::default())),
        }
    }

    /// Derive a default ledger path under `dir` for `session_id`:
    /// `<dir>/task_tree/<session_id>.jsonl`. `dir` is e.g. `~/.fathom/ledger`.
    pub fn for_session(dir: impl AsRef<Path>, session_id: &str) -> PathBuf {
        // Sanitize the id so it cannot escape the directory.
        let safe: String = session_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        dir.as_ref().join("task_tree").join(format!("{safe}.jsonl"))
    }

    /// Monotonic-ish millisecond timestamp. Millisecond (not second) granularity
    /// so a burst of appends within one wall-clock second still orders correctly.
    fn now_ts() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Load existing rows from disk (append order). Absent file → empty.
    pub async fn load(&self) -> anyhow::Result<()> {
        let raw = match tokio::fs::read(&self.path).await {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("read tree ledger {}: {e}", self.path.display())),
        };
        let mut state = self.state.lock().await;
        state.bytes = raw.len() as u64;
        for (idx, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<TreeRow>(line) {
                Ok(row) => state.rows.push(row),
                Err(e) => {
                    tracing::warn!(
                        "skipping malformed tree-ledger line {} in {}: {}",
                        idx + 1,
                        self.path.display(),
                        e
                    );
                }
            }
        }
        Ok(())
    }

    fn valid_kind(kind: &str) -> bool {
        COORDINATION_KINDS.contains(&kind) || BEACON_KINDS.contains(&kind)
    }

    /// Append a row. Refused (returns `Err`) when the kind is unknown or the
    /// ledger exceeds [`MAX_LEDGER_BYTES`].
    pub async fn append(
        &self,
        kind: &str,
        text: &str,
        task_id: &str,
        role: &str,
        needs_parent_attention: bool,
        payload: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        if !Self::valid_kind(kind) {
            anyhow::bail!("unknown tree-ledger kind: {kind}");
        }
        if text.len() > MAX_TEXT_CHARS {
            anyhow::bail!("tree-ledger text too long ({} > {MAX_TEXT_CHARS})", text.len());
        }
        let row = TreeRow {
            ts: Self::now_ts(),
            kind: kind.to_string(),
            text: text.to_string(),
            task_id: task_id.to_string(),
            role: role.to_string(),
            needs_parent_attention,
            payload,
        };

        // Hard byte bound: refuse further writes once the file is full so a
        // chatty swarm cannot balloon disk.
        {
            let state = self.state.lock().await;
            if state.bytes >= MAX_LEDGER_BYTES {
                anyhow::bail!("tree-ledger size limit reached ({MAX_LEDGER_BYTES} bytes)");
            }
        }

        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut line = serde_json::to_string(&row)?;
        line.push('\n');
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| anyhow::anyhow!("open tree ledger {}: {e}", self.path.display()))?;
        f.write_all(line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("append tree ledger {}: {e}", self.path.display()))?;
        f.flush().await.ok();

        let mut state = self.state.lock().await;
        state.bytes += line.len() as u64;
        state.rows.push(row);
        Ok(())
    }

    /// All attention-worthy beacons appended strictly after `after_ts`, plus a
    /// count of how many earlier rows exist (so a caller can point at the tail
    /// with `…[N earlier entries via tree_read]`).
    pub async fn attention_after(&self, after_ts: u64) -> (Vec<TreeRow>, usize) {
        let state = self.state.lock().await;
        let earlier = state.rows.iter().filter(|r| r.ts <= after_ts).count();
        let beacons: Vec<TreeRow> = state
            .rows
            .iter()
            .filter(|r| r.ts > after_ts && r.needs_parent_attention)
            .cloned()
            .collect();
        (beacons, earlier)
    }

    /// Tail digest for injecting into context (latest `limit` rows).
    pub async fn tail(&self, limit: usize) -> Vec<TreeRow> {
        let state = self.state.lock().await;
        state.rows.iter().rev().take(limit).rev().cloned().collect()
    }

    /// Row count (for tests / diagnostics).
    pub async fn len(&self) -> usize {
        self.state.lock().await.rows.len()
    }

    /// Group row counts by kind (for tests / diagnostics).
    pub async fn counts_by_kind(&self) -> HashMap<String, usize> {
        let state = self.state.lock().await;
        let mut m: HashMap<String, usize> = HashMap::new();
        for r in &state.rows {
            *m.entry(r.kind.clone()).or_insert(0) += 1;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn append_and_load_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("tree.jsonl");
        {
            let l = TaskTreeLedger::new(&path);
            l.append("fact", "found acme.com", "a1", "researcher", false, None)
                .await
                .unwrap();
            l.append("blocker", "captcha on source", "a2", "researcher", true, None)
                .await
                .unwrap();
            assert_eq!(l.len().await, 2);
        }
        let l = TaskTreeLedger::new(&path);
        l.load().await.unwrap();
        assert_eq!(l.len().await, 2);
        let counts = l.counts_by_kind().await;
        assert_eq!(counts.get("fact"), Some(&1));
        assert_eq!(counts.get("blocker"), Some(&1));
    }

    #[tokio::test]
    async fn attention_after_filters_by_kind_and_ts() {
        let tmp = tempfile::TempDir::new().unwrap();
        let l = TaskTreeLedger::new(tmp.path().join("t.jsonl"));
        let t0 = TaskTreeLedger::now_ts();
        l.append("milestone", "lead complete", "a1", "researcher", false, None)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        l.append("blocker", "hit rate limit", "a2", "researcher", true, None)
            .await
            .unwrap();

        let (beacons, earlier) = l.attention_after(t0).await;
        assert_eq!(earlier, 1); // the milestone is before
        assert_eq!(beacons.len(), 1); // only the blocker needs attention
        assert_eq!(beacons[0].kind, "blocker");
    }

    #[tokio::test]
    async fn refuses_bad_kind_and_oversized_row() {
        let tmp = tempfile::TempDir::new().unwrap();
        let l = TaskTreeLedger::new(tmp.path().join("t.jsonl"));
        assert!(l
            .append("nope", "x", "a", "r", false, None)
            .await
            .is_err());
        let huge = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(l
            .append("fact", &huge, "a", "r", false, None)
            .await
            .is_err());
        assert_eq!(l.len().await, 0);
    }

    #[tokio::test]
    async fn tail_returns_latest_first_order() {
        let tmp = tempfile::TempDir::new().unwrap();
        let l = TaskTreeLedger::new(tmp.path().join("t.jsonl"));
        l.append("fact", "one", "a", "r", false, None).await.unwrap();
        l.append("fact", "two", "a", "r", false, None).await.unwrap();
        l.append("fact", "three", "a", "r", false, None).await.unwrap();
        let tail = l.tail(2).await;
        let texts: Vec<&str> = tail.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["two", "three"]);
    }
}
