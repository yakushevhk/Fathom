//! Engine drivers — the "brains" behind bots. Each driver normalizes a
//! provider (local CLI harness or OpenAI-compatible HTTP endpoint) into a
//! stream of `TurnEvent`s plus an approval channel.

use fathom_core::types::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, oneshot, Mutex};

/// Normalized stream from an engine turn — drives message segments + cards.
#[derive(Debug, Clone)]
pub enum TurnEvent {
    /// Assistant text delta (append to current text segment).
    Text(String),
    /// Reasoning/chain-of-thought delta.
    Thinking(String),
    /// A tool/command call began or updated.
    ToolUse {
        name: String,
        input: String,
    },
    ToolResult {
        name: String,
        output: String,
        ok: bool,
    },
    /// Engine asks permission — the driver stashed a oneshot in the shared
    /// `SharedDecisions` map keyed by `request_id`; the API resolves it.
    PermissionRequest {
        /// Id the engine expects back in its control channel.
        request_id: String,
        title: String,
        detail: String,
    },
    /// Turn finished (final text in `text` when present).
    Done {
        text: Option<String>,
    },
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ApprovalDecision {
    Allow,
    Deny(String),
}

/// What the turn needs to run: prompt text + optional SOUL.md system prompt.
#[derive(Debug, Clone)]
pub struct TurnInput {
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub cwd: Option<PathBuf>,
    /// Auto-approve risky actions (skip permission prompts) where supported.
    pub auto_approve: bool,
}

/// Pending permission decisions: request_id → resolver. Engines stash a
/// oneshot when emitting `TurnEvent::PermissionRequest`; `POST
/// /api/approvals/:request_id` fires it. Entries live at most one turn.
pub type SharedDecisions =
    std::sync::Arc<Mutex<std::collections::HashMap<String, oneshot::Sender<ApprovalDecision>>>>;

/// One live engine interaction. CLI engines may keep a process resident across
/// turns; HTTP engines are stateless per turn.
#[async_trait::async_trait]
pub trait EngineSession: Send {
    /// Send a user turn; events are broadcast until Done/Error.
    async fn send_turn(
        &mut self,
        input: TurnInput,
        events: broadcast::Sender<TurnEvent>,
        decisions: SharedDecisions,
    );
    /// Abort the in-flight turn, if any, and tear the process down.
    async fn abort(&mut self);
}

/// Driver factory — one per engine kind.
#[async_trait::async_trait]
pub trait EngineDriver: Send + Sync {
    fn kind(&self) -> EngineKind;
    fn model_catalog(&self) -> Vec<ModelInfo>;
    /// Probe availability + version (cached by the registry).
    async fn probe(&self, cfg: &EngineConfig) -> EngineStatus;
    async fn start_session(&self, cfg: &EngineConfig) -> anyhow::Result<Box<dyn EngineSession>>;
}

/// Which binary would a CLI engine spawn? Config override → PATH lookup.
pub fn resolve_cli(cfg: &EngineConfig, default_bin: &str) -> Option<PathBuf> {
    if let Some(cli) = cfg.cli.as_deref().filter(|c| !c.trim().is_empty()) {
        let p = PathBuf::from(cli);
        if p.is_absolute() {
            return p.exists().then_some(p);
        }
        return which(default_bin_from(cli));
    }
    which(default_bin)
}

fn default_bin_from(cli: &str) -> &str {
    std::path::Path::new(cli)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(cli)
}

/// Minimal `which` — checks PATH entries for an executable file.
pub fn which(bin: &str) -> Option<PathBuf> {
    if let Some(p) = PathBuf::from(bin)
        .canonicalize()
        .ok()
        .filter(|p| p.is_file())
    {
        if bin.contains('/') {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join(bin);
        if cand.is_file() && is_executable(&cand) {
            return Some(cand);
        }
    }
    // Common install spots PATH may miss under a desktop session.
    if let Some(home) = dirs::home_dir() {
        for extra in [
            ".local/bin",
            ".npm-global/bin",
            ".cargo/bin",
            ".claude/local",
            "bin",
        ] {
            let cand = home.join(extra).join(bin);
            if cand.is_file() && is_executable(&cand) {
                return Some(cand);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
#[cfg(not(unix))]
fn is_executable(p: &std::path::Path) -> bool {
    p.exists()
}

/// Probe `<cli> --version` with a short timeout.
pub async fn cli_version(cli: &std::path::Path) -> Option<String> {
    let out = tokio::process::Command::new(cli).arg("--version").output();
    match tokio::time::timeout(std::time::Duration::from_secs(8), out).await {
        Ok(Ok(o)) => {
            let s = String::from_utf8_lossy(&o.stdout);
            let re = regex_lite_version(&s);
            Some(re.unwrap_or_else(|| s.lines().next().unwrap_or("").trim().to_string()))
                .filter(|v| !v.is_empty())
        }
        _ => None,
    }
}

fn regex_lite_version(s: &str) -> Option<String> {
    let start = s.char_indices().find(|(_, c)| c.is_ascii_digit())?.0;
    let end = s[start..]
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map(|(i, _)| start + i)
        .unwrap_or(s.len());
    Some(s[start..end].to_string())
}

/// Shared mutable session table: thread_id → live engine session.
pub type SessionMap = Arc<Mutex<BTreeMap<String, Arc<Mutex<Box<dyn EngineSession>>>>>>;

pub fn new_session_map() -> SessionMap {
    Arc::new(Mutex::new(BTreeMap::new()))
}
