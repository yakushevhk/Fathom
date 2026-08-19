//! Durable improvement backlog (ouroboros `improvement_backlog.py` inspired).
//!
//! Self-improvement that remembers: rather than shipping one-off fixes, the
//! agent records candidate improvements durably, keyed by a stable fingerprint,
//! re-triggering the same item bumps a recurrence count instead of writing a
//! second look-alike row, and the whole list survives restarts. An optional
//! LLM pass performs semantic de-dup *outside* the file lock so a reformulated
//! version of the same idea folds into the existing row rather than spawning a
//! duplicate.
//!
//! Storage is a plain Markdown file (`improvement-backlog.md`) so it stays
//! human-readable and diff-friendly, plus an exact-fingerprint index built on
//! load. Fields follow the canonical order from ouroboros:
//! `status priority kind created_at last_seen count source category task_id
//! requires_plan_review closed_at summary evidence context proposed_next_step`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;

/// Default backlog topic file under the ledger dir.
pub const BACKLOG_REL_PATH: &str = "improvement-backlog.md";
/// Cap on candidates offered to semantic de-dup (ouroboros `_DEDUP_CANDIDATE_CAP`).
pub const DEDUP_CANDIDATE_CAP: usize = 20;
/// Grooming cap (keep the file bounded).
pub const GROOM_CAP: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    fn from_str(s: &str) -> Self {
        match s {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Medium,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Priority::High => "high",
            Priority::Medium => "med",
            Priority::Low => "low",
        }
    }
}

/// One backlog item.
#[derive(Debug, Clone)]
pub struct BacklogEntry {
    /// Stable dedup fingerprint.
    pub fingerprint: String,
    pub summary: String,
    pub category: String,
    pub source: String,
    pub status: String, // open | done
    pub priority: Priority,
    pub kind: String, // bug | improvement | capability_idea
    pub count: u64,
    pub last_seen: String, // RFC3339
    pub created_at: String, // RFC3339
    pub closed_at: Option<String>,
    pub requires_plan_review: bool,
    pub context: Option<String>,
    pub proposed_next_step: Option<String>,
}

/// In-memory index: fingerprint → entry, plus raw line preservation.
#[derive(Debug, Default)]
struct State {
    by_fp: HashMap<String, BacklogEntry>,
}

/// The improvement backlog. Thread-safe; persists through a compact Markdown
/// file under the ledger directory.
pub struct ImprovementBacklog {
    path: PathBuf,
    inner: Arc<TokioMutex<State>>,
}

impl ImprovementBacklog {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            inner: Arc::new(TokioMutex::new(State::default())),
        }
    }

    /// The default backlog path: `~/.fathom/ledger/improvement-backlog.md`.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
        Ok(home.join(".fathom").join("ledger").join(BACKLOG_REL_PATH))
    }

    /// Load the backlog from disk. Absent file → empty.
    pub async fn load(&self) -> anyhow::Result<()> {
        let raw = match tokio::fs::read_to_string(&self.path).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("read backlog {}: {e}", self.path.display())),
        };
        let mut state = self.inner.lock().await;
        for entry in parse_backlog(&raw) {
            if let Ok(fp) = fingerprint_of(&entry) {
                state.by_fp.insert(fp, entry);
            }
        }
        Ok(())
    }

    /// Number of entries (for tests / diagnostics).
    pub async fn len(&self) -> usize {
        self.inner.lock().await.by_fp.len()
    }

    /// Add (or bump) a backlog entry. Uses exact fingerprint as a fast path;
    /// if `semantic_llm` is supplied, an unmatched entry is first asked whether
    /// it semantically duplicates an existing row of the same category+source.
    /// `semantic_llm` is called *after* releasing the lock (fail-open: on any
    /// LLM error we fall back to creating a fresh row).
    pub async fn add(
        &self,
        summary: &str,
        category: &str,
        source: &str,
        kind: &str,
        priority: Priority,
        requires_plan_review: bool,
        context: Option<String>,
        proposed_next_step: Option<String>,
        semantic_llm: Option<
            &(dyn Fn(&str, &[(String, String, String)]) -> anyhow::Result<Option<String>>
                + Send
                + Sync),
        >,
    ) -> anyhow::Result<()> {
        // Fast path: exact fingerprint under lock.
        let now = now_rfc3339();
        {
            let mut state = self.inner.lock().await;
            if let Some(ex) = state.by_fp.get_mut(&stable_fingerprint(summary, category, source)) {
                ex.count += 1;
                ex.last_seen = now.clone();
                if ex.status == "done" {
                    ex.status = "open".to_string();
                    ex.closed_at = None;
                }
                self.persist(&state).await?;
                return Ok(());
            }
        }

        // Semantic de-dup outside the lock: only when we could not exact-match.
        if let Some(llm) = semantic_llm {
            let candidates: Vec<(String, String, String)> = {
                let state = self.inner.lock().await;
                state
                    .by_fp
                    .values()
                    .filter(|e| e.category == category && e.source == source)
                    .take(DEDUP_CANDIDATE_CAP)
                    .map(|e| (e.summary.clone(), e.kind.clone(), e.fingerprint.clone()))
                    .collect()
            };
            if !candidates.is_empty() {
                if let Ok(Some(dup_fp)) = llm(summary, &candidates) {
                    let mut state = self.inner.lock().await;
                    if let Some(ex) = state.by_fp.get_mut(&dup_fp) {
                        ex.count += 1;
                        ex.last_seen = now_rfc3339();
                        if ex.status == "done" {
                            ex.status = "open".to_string();
                            ex.closed_at = None;
                        }
                        self.persist(&state).await?;
                        return Ok(());
                    }
                }
            }
        }

        // Fresh row.
        let fp = stable_fingerprint(summary, category, source);
        let entry = BacklogEntry {
            fingerprint: fp.clone(),
            summary: summary.trim().to_string(),
            category: category.to_string(),
            source: source.to_string(),
            status: "open".to_string(),
            priority,
            kind: kind.to_string(),
            count: 1,
            last_seen: now.clone(),
            created_at: now,
            closed_at: None,
            requires_plan_review,
            context,
            proposed_next_step,
        };
        let mut state = self.inner.lock().await;
        state.by_fp.insert(fp, entry);
        self.persist(&state).await
    }

    /// Close a set of entries by fingerprint or summary match (e.g. on commit).
    pub async fn close(&self, task_id_or_fps: &[&str]) -> anyhow::Result<usize> {
        let mut state = self.inner.lock().await;
        let mut closed = 0usize;
        for value in task_id_or_fps {
            let mut hit = false;
            if let Some(ex) = state.by_fp.get_mut(*value) {
                if ex.status != "done" {
                    ex.status = "done".to_string();
                    ex.closed_at = Some(now_rfc3339());
                    closed += 1;
                }
                hit = true;
            }
            if !hit {
                for ex in state.by_fp.values_mut() {
                    if ex.summary == *value || ex.source == *value {
                        if ex.status != "done" {
                            ex.status = "done".to_string();
                            ex.closed_at = Some(now_rfc3339());
                            closed += 1;
                        }
                        break;
                    }
                }
            }
        }
        if closed > 0 {
            self.persist(&state).await?;
        }
        Ok(closed)
    }

    /// Format a short digest for injection into context (max `limit` items,
    /// sorted by priority then count desc).
    pub async fn digest(&self, limit: usize) -> String {
        let state = self.inner.lock().await;
        let mut items: Vec<&BacklogEntry> = state
            .by_fp
            .values()
            .filter(|e| e.status != "done")
            .collect();
        items.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then(b.count.cmp(&a.count))
                .then(b.last_seen.cmp(&a.last_seen))
        });
        let mut out = Vec::new();
        for e in items.into_iter().take(limit.max(1)) {
            out.push(format!(
                "- [{}] (x{} | {}) {}",
                e.priority.as_str(),
                e.count,
                e.source,
                e.summary
            ));
        }
        if out.is_empty() {
            "No open improvements in backlog.".to_string()
        } else {
            format!("Open improvements:\n{}", out.join("\n"))
        }
    }

    async fn persist(&self, state: &State) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut buf = String::from("# Improvement Backlog\n\n");
        let mut items: Vec<&BacklogEntry> = state.by_fp.values().collect();
        items.sort_by(|a, b| b.priority.cmp(&a.priority).then(b.last_seen.cmp(&a.last_seen)));
        for e in items {
            buf.push_str(&format!("### {}\n", e.fingerprint));
            buf.push_str(&format!("- status: {}\n", e.status));
            buf.push_str(&format!("- priority: {}\n", e.priority.as_str()));
            buf.push_str(&format!("- kind: {}\n", e.kind));
            buf.push_str(&format!("- count: {}\n", e.count));
            buf.push_str(&format!("- created_at: {}\n", e.created_at));
            buf.push_str(&format!("- last_seen: {}\n", e.last_seen));
            buf.push_str(&format!("- source: {}\n", e.source));
            buf.push_str(&format!("- category: {}\n", e.category));
            if let Some(c) = &e.closed_at {
                buf.push_str(&format!("- closed_at: {c}\n"));
            }
            buf.push_str(&format!(
                "- requires_plan_review: {}\n",
                if e.requires_plan_review { "true" } else { "false" }
            ));
            buf.push_str(&format!("- summary: {}\n", e.summary));
            if let Some(c) = &e.context {
                buf.push_str(&format!("- context: {}\n", sanitize_line(c)));
            }
            if let Some(s) = &e.proposed_next_step {
                buf.push_str(&format!("- proposed_next_step: {}\n", sanitize_line(s)));
            }
            buf.push('\n');
        }
        let mut f = tokio::fs::File::create(&self.path)
            .await
            .map_err(|e| anyhow::anyhow!("open backlog {}: {e}", self.path.display()))?;
        f.write_all(buf.as_bytes()).await?;
        f.flush().await.ok();
        Ok(())
    }
}

fn sanitize_line(s: &str) -> String {
    s.replace('\n', " ").replace('\r', " ").chars().take(300).collect()
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Stable SHA-256 fingerprint from the normalized (summary, category, source)
/// triple — the same idea expressed with a different casing/whitespace maps to
/// the same fingerprint.
pub fn stable_fingerprint(summary: &str, category: &str, source: &str) -> String {
    let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let joined = format!("{} | {} | {}", norm(summary), norm(category), norm(source));
    fingerprint_of_str(&joined)
}

fn fingerprint_of_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).take(12).collect()
}

fn fingerprint_of(e: &BacklogEntry) -> anyhow::Result<String> {
    Ok(stable_fingerprint(&e.summary, &e.category, &e.source))
}

/// Parse the Markdown backlog back into entries.
fn parse_backlog(raw: &str) -> Vec<BacklogEntry> {
    let mut out = Vec::new();
    let mut current: Option<BacklogEntry> = None;
    let mut pending_fp = None;

    for line in raw.lines() {
        if let Some(fp) = line.strip_prefix("### ") {
            if let Some(c) = current.take() {
                out.push(c);
            }
            pending_fp = Some(fp.trim().to_string());
            continue;
        }
        if let Some(c) = &mut current {
            let Some(kv) = line.strip_prefix("- ") else { continue };
            let Some((k, v)) = kv.split_once(": ") else { continue };
            match k {
                "status" => c.status = v.trim().to_string(),
                "kind" => c.kind = v.trim().to_string(),
                "count" => c.count = v.trim().parse().unwrap_or(1),
                "created_at" => c.created_at = v.trim().to_string(),
                "last_seen" => c.last_seen = v.trim().to_string(),
                "source" => c.source = v.trim().to_string(),
                "category" => c.category = v.trim().to_string(),
                "closed_at" => c.closed_at = Some(v.trim().to_string()),
                "requires_plan_review" => c.requires_plan_review = v.trim() == "true",
                "summary" => c.summary = v.trim().to_string(),
                "context" => c.context = Some(v.trim().to_string()),
                "proposed_next_step" => c.proposed_next_step = Some(v.trim().to_string()),
                "priority" => c.priority = Priority::from_str(v.trim()),
                _ => {}
            }
        } else if let Some(fp) = pending_fp.take() {
            // First field after the header is normally `status`.
            current = Some(BacklogEntry {
                fingerprint: fp.clone(),
                summary: String::new(),
                category: String::new(),
                source: String::new(),
                status: String::new(),
                priority: Priority::Medium,
                kind: String::new(),
                count: 1,
                last_seen: String::new(),
                created_at: String::new(),
                closed_at: None,
                requires_plan_review: false,
                context: None,
                proposed_next_step: None,
            });
        }
    }
    if let Some(c) = current.take() {
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn backlog(tmp: &tempfile::TempDir) -> ImprovementBacklog {
        let b = ImprovementBacklog::new(tmp.path().join(BACKLOG_REL_PATH));
        b.load().await.unwrap();
        b
    }

    #[tokio::test]
    async fn add_bumps_recurrence_on_fingerprint_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        let b = backlog(&tmp).await;
        b.add("Fix MX fallback", "tool", "verify_email", "bug", Priority::High, false, None, None, None)
            .await
            .unwrap();
        b.add("fix mx fallback ", "tool", "verify_email", "bug", Priority::High, false, None, None, None)
            .await
            .unwrap();
        assert_eq!(b.len().await, 1);
        let state = b.inner.lock().await;
        let e = state.by_fp.values().next().unwrap();
        assert_eq!(e.count, 2);
    }

    #[tokio::test]
    async fn reload_from_disk_roundtrips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(BACKLOG_REL_PATH);
        {
            let b = ImprovementBacklog::new(&path);
            b.load().await.unwrap();
            b.add("Add tool", "capability", "synthesize", "capability_idea", Priority::Low, false, Some("desc".into()), None, None)
                .await
                .unwrap();
        }
        let b = ImprovementBacklog::new(&path);
        b.load().await.unwrap();
        assert_eq!(b.len().await, 1);
        let state = b.inner.lock().await;
        let e = state.by_fp.values().next().unwrap();
        assert_eq!(e.kind, "capability_idea");
        assert_eq!(e.context.as_deref(), Some("desc"));
    }

    #[tokio::test]
    async fn close_reopens_on_repeat() {
        let tmp = tempfile::TempDir::new().unwrap();
        let b = backlog(&tmp).await;
        b.add("Fix X", "tool", "src", "bug", Priority::High, false, None, None, None)
            .await
            .unwrap();
        let fp = stable_fingerprint("Fix X", "tool", "src");
        let closed = b.close(&[&fp]).await.unwrap();
        assert_eq!(closed, 1);
        // Re-adding the same item re-opens it.
        b.add("Fix X", "tool", "src", "bug", Priority::High, false, None, None, None)
            .await
            .unwrap();
        let state = b.inner.lock().await;
        let e = state.by_fp.get(&fp).unwrap();
        assert_eq!(e.status, "open");
        assert!(e.closed_at.is_none());
    }

    #[tokio::test]
    async fn digest_omits_done_and_sorts() {
        let tmp = tempfile::TempDir::new().unwrap();
        let b = backlog(&tmp).await;
        b.add("Low idea", "cap", "s", "capability_idea", Priority::Low, false, None, None, None)
            .await
            .unwrap();
        b.add("High bug", "tool", "s", "bug", Priority::High, false, None, None, None)
            .await
            .unwrap();
        let fp = stable_fingerprint("Low idea", "cap", "s");
        b.close(&[&fp]).await.unwrap();
        let d = b.digest(10).await;
        assert!(d.contains("High bug"));
        assert!(!d.contains("Low idea"));
    }
}
