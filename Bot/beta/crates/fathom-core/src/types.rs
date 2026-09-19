//! Shared domain types for the Fathom bot platform — a local-first chat app
//! where every contact is a real agent. Pure-Rust port of the OpenMausBot
//! domain model (bots, threads, messages, approvals, engines, computers).

use serde::{Deserialize, Serialize};

/// Supported agent engine kinds. A bot's brain can be a local CLI harness
/// (`claude`, `codex`, `fathom`) or an OpenAI-compatible HTTP endpoint
/// (`grok`, `openai-compat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineKind {
    Claude,
    Codex,
    Fathom,
    Grok,
    OpenAiCompat,
}

impl EngineKind {
    pub const ALL: [EngineKind; 5] = [
        EngineKind::Claude,
        EngineKind::Codex,
        EngineKind::Fathom,
        EngineKind::Grok,
        EngineKind::OpenAiCompat,
    ];

    pub fn label(self) -> &'static str {
        match self {
            EngineKind::Claude => "Claude",
            EngineKind::Codex => "Codex",
            EngineKind::Fathom => "Fathom",
            EngineKind::Grok => "Grok",
            EngineKind::OpenAiCompat => "OpenAI-compat",
        }
    }

    /// Engine uses a local CLI binary that can be pointed at a custom path.
    pub fn is_cli(self) -> bool {
        matches!(
            self,
            EngineKind::Claude | EngineKind::Codex | EngineKind::Fathom
        )
    }
}

impl std::str::FromStr for EngineKind {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "claude" => EngineKind::Claude,
            "codex" => EngineKind::Codex,
            "fathom" => EngineKind::Fathom,
            "grok" => EngineKind::Grok,
            "openai-compat" => EngineKind::OpenAiCompat,
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for EngineKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            EngineKind::Claude => "claude",
            EngineKind::Codex => "codex",
            EngineKind::Fathom => "fathom",
            EngineKind::Grok => "grok",
            EngineKind::OpenAiCompat => "openai-compat",
        })
    }
}

/// Per-engine configuration: CLI path override and HTTP endpoint settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineConfig {
    /// Absolute path to the CLI binary (CLI engines) — PATH lookup when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// Base URL for HTTP engines (grok/openai-compat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Environment variable holding the API key for HTTP engines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Default model id for this engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Whether the engine may run tools/commands without asking (default ask).
    #[serde(default)]
    pub auto_approve: bool,
}

/// Runtime status of an engine as reported by `GET /api/engines`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub kind: EngineKind,
    pub label: String,
    /// Whether a usable installation was detected on this machine.
    pub available: bool,
    /// Resolved binary path or endpoint URL.
    pub resolved: Option<String>,
    /// Version string when probed.
    pub version: Option<String>,
    /// Why the engine is unavailable (dimmed in the picker with the reason).
    pub reason: Option<String>,
    /// Catalog of models this engine exposes.
    pub models: Vec<ModelInfo>,
    pub config: EngineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    /// True for the engine's default model.
    #[serde(default)]
    pub default: bool,
}

/// A bot contact in the sidebar roster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bot {
    pub id: String,
    pub name: String,
    pub engine: EngineKind,
    pub model: Option<String>,
    /// Persona/instructions — the bot's SOUL.md body.
    #[serde(default)]
    pub soul: String,
    /// Hue seed used to color the avatar circle.
    #[serde(default)]
    pub avatar_seed: u8,
    /// Working directory the engine runs in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Sandboxed auto-approval for this bot (engine permitting).
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub archived: bool,
    /// Attached cloud/local computer id, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Denormalized for roster display: last message snippet + activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<i64>,
    #[serde(default)]
    pub unread: i64,
    /// True while a turn is running for this bot's active thread.
    #[serde(default)]
    pub working: bool,
}

/// A chat thread. One primary thread per bot (`kind=direct`) plus rooms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// `direct` (bot DM) or `room` (group).
    pub kind: String,
    /// Owning bot for direct threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Bot,
    System,
    Tool,
}

/// One chat message. Rich payloads (approvals, tool calls, thinking) ride in
/// `card`/`segments` JSON so the wire format stays stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: Role,
    pub text: String,
    /// Engine that produced the message (bot messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Ordered content segments: text, thinking, tool calls, cards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<Segment>,
    /// Streaming in progress flag.
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: i64,
}

/// A piece of a message — rendered in order by clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Segment {
    Text {
        text: String,
    },
    Thinking {
        text: String,
    },
    ToolCall {
        name: String,
        input: String,
        output: Option<String>,
        status: ToolStatus,
    },
    Approval {
        approval: ApprovalRequest,
    },
    Error {
        text: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Running,
    Done,
    Failed,
}

/// A permission/decision request raised by an engine — shown as an inline card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub bot_id: String,
    pub thread_id: String,
    /// Engine-native request id echoed back on resolve.
    #[serde(default)]
    pub request_id: String,
    /// `tool_use` (allow/deny) or `question` (free-text answer).
    pub kind: String,
    pub title: String,
    /// Command/tool preview shown in the card.
    #[serde(default)]
    pub detail: String,
    /// Choice labels for multi-option questions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    pub status: ApprovalStatus,
    /// The answer/denial reason recorded on resolve.
    #[serde(default)]
    pub response: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    Pending,
    Allowed,
    Denied,
    Answered,
}

/// A cloud or local computer attached to a bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Computer {
    pub id: String,
    /// `cloud` | `local` | `host`
    pub kind: String,
    pub name: String,
    /// `off` | `starting` | `ready` | `error`
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Latest screenshot preview (base64 jpeg) when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub created_at: i64,
}

/// Event envelope broadcast on `GET /api/events` (SSE `data:` payloads).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A message was created or updated (streaming snapshots included).
    MessageUpsert { message: Message },
    /// Bot roster entry changed (name, model, working flag, unread…).
    BotUpsert { bot: Bot },
    /// Approval card created/resolved.
    ApprovalUpsert { approval: ApprovalRequest },
    /// Thread created/renamed.
    ThreadUpsert { thread: Thread },
    /// Engine status refresh (installed/uninstalled/version).
    Engines { engines: Vec<EngineStatus> },
    /// Computer status change.
    ComputerUpsert { computer: Computer },
    /// A turn finished with an error (also marks the pending message).
    TurnError {
        thread_id: String,
        bot_id: String,
        error: String,
    },
}

/// Request body for `POST /api/threads/:id/messages` — a user chat send.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessage {
    pub text: String,
    /// Override the bot's model for this turn.
    #[serde(default)]
    pub model: Option<String>,
}

/// Harness-wide settings persisted in the store's kv table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    /// Data directory holding the SQLite db, SOUL.md files and caches.
    pub data_dir: String,
    /// Engine configs keyed by engine kind ("claude" → EngineConfig…).
    #[serde(default)]
    pub engines: std::collections::BTreeMap<String, EngineConfig>,
    /// Strict opt-in telemetry. Defaults OFF — sovereignty invariant.
    #[serde(default)]
    pub analytics_enabled: bool,
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().simple())
}
