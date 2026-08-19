//! Verification-receipt ledger for contact verification (ouroboros-inspired).
//!
//! The problem we solve: an OSINT/lead-gen agent can produce many claims about
//! a contact (syntax OK, domain has MX, mailbox accepted over SMTP, social
//! profile resolves, hypothetical email pattern guessed…). Without a durable,
//! typed record of *which check actually ran and what it concluded*, downstream
//! persistence (`save_contacts`, `autosave`, the synthetic final report) has no
//! way to distinguish a *verified* fact from a *guess*.
//!
//! This module keeps an append-only JSONL ledger of check *receipts*. Each
//! receipt is keyed by a **typed identity** `(kind, value)` — e.g.
//! `("email_domain_mx", "example.com")` or `("email_smtp", "a@b.co")` — so a
//! green on one kind can never "pay off" (silence) a red on a different kind,
//! and vice-versa. One check writing PASS for the domain cannot hide that the
//! SMTP mailbox probe returned FAIL. The latest receipt per typed key wins.
//!
//! Consumers only ever ask "what is the settled verdict for kind `k` on `v`?",
//! which is resolved from the ledger (latest record), falling back to `Unknown`
//! when nothing was ever recorded. This is the single source of truth that
//! keeps "verified" out of the mouth of code that merely guessed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

/// A named verification category. Opaque to the ledger itself — consumer tools
/// own the set of valid kinds. Wrapped `String` so it serializes freely
/// (durably recorded in the JSONL ledger) while remaining a newtype that makes
/// mixing two kinds a compile error at key-construction sites.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptKind(String);

impl ReceiptKind {
    pub const EMAIL_SYNTAX: &'static str = "email_syntax";
    pub const EMAIL_DOMAIN_MX: &'static str = "email_domain_mx";
    pub const EMAIL_SMTP: &'static str = "email_smtp";
    pub const EMAIL_DISPOSABLE: &'static str = "email_disposable";
    pub const PHONE_NORMALIZE: &'static str = "phone_normalize";
    pub const SOCIAL_PROFILE: &'static str = "social_profile";
    pub const PERSON_NAME: &'static str = "person_name";

    /// Construct from a canonical static kind name.
    pub fn of(name: &'static str) -> Self {
        Self(name.to_string())
    }

    /// Construct from an arbitrary (e.g. deserialized) name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl PartialEq<&str> for ReceiptKind {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::fmt::Display for ReceiptKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Verdict a check produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The check concluded the fact holds (e.g. mailbox accepted).
    Pass,
    /// The check conclusively showed the fact does not hold (rejected / no MX).
    Fail,
    /// The check could not reach a conclusion (inconclusive, rate-limited,
    /// greylisting, network failure). Not a green, not a red — a non-answer.
    Inconclusive,
    /// Present in the ledger but flagged as possibly laundered — the tool ran
    /// under a pipeline that could mask the real exit code (`\| tail`,
    /// `|| true`), so a PASS here must not be trusted as authoritative.
    PossiblyLaundered,
}

impl Verdict {
    pub fn is_passing(self) -> bool {
        matches!(self, Verdict::Pass)
    }
}

/// One durable record in the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Typed identity of the check.
    pub kind: ReceiptKind,
    /// The exact canonical value that was checked (email, domain, phone,
    /// profile URL/username…). Canonicalization is the caller's job — two
    /// callers must use the same spelling for the same fact.
    pub value: String,
    pub verdict: Verdict,
    /// Free-form human detail (e.g. the SMTP reply line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Provenance: which tool / orchestration produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Unix epoch seconds. Monotonic within one process and stable across
    /// restarts read back from disk.
    pub ts: u64,
}

impl Receipt {
    fn key(kind: &ReceiptKind, value: &str) -> (String, String) {
        (kind.to_string(), canonical(value))
    }

    /// The in-memory index key for this receipt.
    fn index_key(&self) -> (String, String) {
        Self::key(&self.kind, &self.value)
    }
}

/// Normalize a value for ledger identity. Emails/phones/domains are
/// lower-cased and trimmed so `A@B.Co` and `a@b.co` map to the same row.
pub fn canonical(value: &str) -> String {
    value.trim().to_lowercase()
}

/// In-memory projection of the ledger: latest receipt per typed key, plus the
/// verdict resolution used by consumers.
#[derive(Debug, Default)]
struct LedgerState {
    /// Latest receipt per `(kind, value)`.
    latest: HashMap<(String, String), Receipt>,
}

/// The verification-receipt ledger. Thread-safe, append-only on disk, with an
/// in-memory latest-wins projection for fast lookup.
#[derive(Clone)]
pub struct ReceiptLedger {
    path: PathBuf,
    state: Arc<Mutex<LedgerState>>,
}

impl ReceiptLedger {
    /// Open (or lazily create on first write) the ledger at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: Arc::new(Mutex::new(LedgerState::default())),
        }
    }

    /// The default user-scoped ledger path: `~/.fathom/ledger/verify_receipts.jsonl`.
    pub fn default_path() -> anyhow::Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
        Ok(home
            .join(".fathom")
            .join("ledger")
            .join("verify_receipts.jsonl"))
    }

    fn now_ts() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Load the ledger from disk, merging all records so the latest per typed
    /// key wins. Does not create the file if absent.
    pub async fn load(&self) -> anyhow::Result<()> {
        let raw = match tokio::fs::read(&self.path).await {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("read ledger {}: {e}", self.path.display())),
        };

        let mut state = self.state.lock().await;
        for (idx, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Receipt>(line) {
                Ok(rec) => {
                    state.latest.insert(rec.index_key(), rec);
                }
                Err(e) => {
                    // A torn tail (crashed append) is quarantined by skipping —
                    // never fail the whole ledger because of one bad line.
                    tracing::warn!(
                        "skipping malformed ledger line {} in {}: {}",
                        idx + 1,
                        self.path.display(),
                        e
                    );
                }
            }
        }
        Ok(())
    }

    /// Record a verdict and persist it. Idempotent on retry; a re-record with a
    /// newer ts supersedes the old. Returns the just-persisted receipt.
    pub async fn record(
        &self,
        kind: ReceiptKind,
        value: &str,
        verdict: Verdict,
        detail: Option<String>,
        source: Option<String>,
    ) -> anyhow::Result<Receipt> {
        let receipt = Receipt {
            kind: kind.clone(),
            value: canonical(value),
            verdict,
            detail,
            source,
            ts: Self::now_ts(),
        };

        // Persist first (append), then update the in-memory projection. If a
        // second process appends between our read and here, the file is
        // append-only so `OpenOptions::append` is atomic enough for our
        // latest-wins semantics.
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }
        let mut line = serde_json::to_string(&receipt)?;
        line.push('\n');
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
            .map_err(|e| anyhow::anyhow!("open ledger {}: {e}", self.path.display()))?;
        f.write_all(line.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("append ledger {}: {e}", self.path.display()))?;
        f.flush().await.ok();

        let mut state = self.state.lock().await;
        state.latest.insert(receipt.index_key(), receipt.clone());
        Ok(receipt)
    }

    /// Resolve the settled verdict for a typed check. `None` when nothing was
    /// ever recorded for this typed key.
    pub async fn verdict(&self, kind: ReceiptKind, value: &str) -> Option<Verdict> {
        let key = Receipt::key(&kind, value);
        let state = self.state.lock().await;
        state.latest.get(&key).map(|r| r.verdict)
    }

    /// Resolve a verdict, treating "no record" and "not passing" uniformly.
    pub async fn is_passing(&self, kind: ReceiptKind, value: &str) -> bool {
        self.verdict(kind, value)
            .await
            .map(|v| v.is_passing())
            .unwrap_or(false)
    }

    /// If a record exists, get its PASS/FAIL/inconclusive plus any detail.
    pub async fn get(&self, kind: ReceiptKind, value: &str) -> Option<Receipt> {
        let key = Receipt::key(&kind, value);
        let state = self.state.lock().await;
        state.latest.get(&key).cloned()
    }
}

/// Convenience: a default ledger opened and loaded at tool setup. Most callers
/// want this rather than constructing one by hand.
pub async fn open_default_ledger() -> anyhow::Result<ReceiptLedger> {
    let path = ReceiptLedger::default_path()?;
    let ledger = ReceiptLedger::new(&path);
    ledger.load().await?;
    Ok(ledger)
}

/// Ensure the parent directory of a path exists (helper for tests / callers).
pub async fn ensure_parent(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn latest_wins_per_typed_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ledger = ReceiptLedger::new(tmp.path().join("r.jsonl"));

        ledger
            .record(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co", Verdict::Fail, None, Some("test".to_string()))
            .await
            .unwrap();
        // A later PASS for the same typed key wins.
        ledger
            .record(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co", Verdict::Pass, None, Some("test".to_string()))
            .await
            .unwrap();
        assert!(ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co").await);

        // A DIFFERENT kind on the same value is NOT silenced by the SMTP pass.
        // The domain-MX Fact is recorded independently.
        ledger
            .record(ReceiptKind::of(ReceiptKind::EMAIL_DOMAIN_MX), "b.co", Verdict::Fail, None, Some("test".to_string()))
            .await
            .unwrap();
        assert!(!ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_DOMAIN_MX), "b.co").await);
        assert!(ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co").await);
    }

    #[tokio::test]
    async fn canonical_value_lowercases() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ledger = ReceiptLedger::new(tmp.path().join("r.jsonl"));
        ledger
            .record(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "A@B.Co", Verdict::Pass, None, None)
            .await
            .unwrap();
        assert!(ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co").await);
        assert!(ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "A@B.CO").await);
    }

    #[tokio::test]
    async fn unknown_kind_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ledger = ReceiptLedger::new(tmp.path().join("r.jsonl"));
        assert_eq!(
            ledger.verdict(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "x@y.z").await,
            None
        );
        assert!(!ledger.is_passing(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "x@y.z").await);
    }

    #[tokio::test]
    async fn reload_from_disk_roundtrips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("r.jsonl");
        {
            let ledger = ReceiptLedger::new(&path);
            ledger
                .record(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co", Verdict::Pass, Some("250 OK".to_string()), Some("t".to_string()))
                .await
                .unwrap();
        }
        // Fresh handle loads from disk.
        let reloaded = ReceiptLedger::new(&path);
        reloaded.load().await.unwrap();
        let rec = reloaded.get(ReceiptKind::of(ReceiptKind::EMAIL_SMTP), "a@b.co").await.unwrap();
        assert_eq!(rec.verdict, Verdict::Pass);
        assert_eq!(rec.detail.as_deref(), Some("250 OK"));
        assert_eq!(rec.kind, ReceiptKind::of(ReceiptKind::EMAIL_SMTP));
    }

    #[tokio::test]
    async fn torn_tail_is_skipped_on_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("r.jsonl");
        tokio::fs::write(
            &path,
            "{}\n{\"broken\": true\n",
        )
        .await
        .unwrap();
        let ledger = ReceiptLedger::new(&path);
        ledger.load().await.unwrap(); // must not error
    }
}
