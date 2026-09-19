//! Shared domain types for the Fathom bot platform — a local-first chat app
//! where every contact is a real agent. Pure-Rust port of the OpenMausBot
//! domain model (bots, threads, messages, approvals, engines, rooms,
//! attachments).

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
            "openai-compat" | "openai_compat" => EngineKind::OpenAiCompat,
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

// ---- approval modes --------------------------------------------------------

/// Permission posture a bot runs under — passed through to the provider's own
/// approval semantics. Mirrors OpenMausBot's approval-mode set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    /// Ask for every risky action (default, fail-closed).
    Ask,
    /// Auto-accept file edits, still ask for commands/network.
    Edits,
    /// Provider-side auto review where the engine implements one.
    Auto,
    /// Answer everything — the operator granted full access.
    Full,
    /// Engine-specific granular mode (Codex only upstream).
    Custom,
}

impl ApprovalMode {
    pub const ALL: [ApprovalMode; 5] = [
        ApprovalMode::Ask,
        ApprovalMode::Edits,
        ApprovalMode::Auto,
        ApprovalMode::Full,
        ApprovalMode::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ApprovalMode::Ask => "Ask",
            ApprovalMode::Edits => "Edits",
            ApprovalMode::Auto => "Auto",
            ApprovalMode::Full => "Full access",
            ApprovalMode::Custom => "Custom",
        }
    }

    /// Whether an engine can express this level natively (or via harness).
    pub fn supported(self, engine: EngineKind) -> bool {
        match self {
            ApprovalMode::Ask | ApprovalMode::Auto => true,
            // `edits` exists where the CLI has an accept-edits permission mode.
            ApprovalMode::Edits => matches!(engine, EngineKind::Claude),
            // OpenAI-compat engines have no provider-side reviewer — Full is
            // implemented in the harness, which answers its own tool gate.
            ApprovalMode::Full => true,
            ApprovalMode::Custom => matches!(engine, EngineKind::Codex),
        }
    }

    /// Next selectable mode for this engine, skipping unsupported ones.
    pub fn next_for(self, engine: EngineKind) -> Self {
        let mut idx = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        for _ in 0..Self::ALL.len() {
            idx = (idx + 1) % Self::ALL.len();
            let m = Self::ALL[idx];
            if m.supported(engine) {
                return m;
            }
        }
        self
    }
}

impl std::str::FromStr for ApprovalMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "ask" => ApprovalMode::Ask,
            "edits" => ApprovalMode::Edits,
            "auto" => ApprovalMode::Auto,
            "full" => ApprovalMode::Full,
            "custom" => ApprovalMode::Custom,
            _ => return Err(()),
        })
    }
}

impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ApprovalMode::Ask => "ask",
            ApprovalMode::Edits => "edits",
            ApprovalMode::Auto => "auto",
            ApprovalMode::Full => "full",
            ApprovalMode::Custom => "custom",
        })
    }
}

/// Reasoning-effort levels (ascending); each engine takes the subset its CLI
/// accepts — none means no flag, the engine keeps its own default.
pub const EFFORT_LEVELS: [&str; 6] = ["none", "low", "medium", "high", "xhigh", "max"];

// ---- engine config / status -------------------------------------------------

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
    /// Reasoning effort hint for engines that accept it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
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

// ---- bots -------------------------------------------------------------------

/// A bot contact in the sidebar roster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bot {
    pub id: String,
    pub name: String,
    /// Role line under the name ("Rust reviewer", "Ops").
    #[serde(default)]
    pub title: String,
    /// Longer "about" text shown in the profile dialog.
    #[serde(default)]
    pub description: String,
    pub engine: EngineKind,
    pub model: Option<String>,
    /// Reasoning effort hint passed to the engine when supported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    /// Persona/instructions — the bot's SOUL.md body.
    #[serde(default)]
    pub soul: String,
    /// Hue seed used to color the avatar circle.
    #[serde(default)]
    pub avatar_seed: u8,
    /// Custom avatar image — an attachment id under data_dir/attachments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    /// Working directory the engine runs in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Canonical approval level (ask/edits/auto/full/custom).
    #[serde(default = "default_approval_mode")]
    pub approval_mode: ApprovalMode,
    /// Tools this bot may always use without asking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub always_allow: Vec<String>,
    /// Desktop notifications for this bot's replies.
    #[serde(default = "default_true")]
    pub notifications: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub hidden: bool,
    /// Sidebar section divider label this bot sits under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// Queue direct-chat messages behind a running turn instead of steering.
    #[serde(default = "default_true")]
    pub park_dms: bool,
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
    /// True while this bot has a pending approval card (waiting on you).
    #[serde(default)]
    pub waiting_on_you: bool,
}

fn default_approval_mode() -> ApprovalMode {
    ApprovalMode::Ask
}
fn default_true() -> bool {
    true
}

// ---- threads ---------------------------------------------------------------

/// A chat thread. Bots own `direct` threads (one primary + extra tasks);
/// rooms own `room` threads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// `direct` (bot DM) or `room` (group).
    pub kind: String,
    /// Owning bot for direct threads; owning room id for room threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    pub title: Option<String>,
    /// The one message pinned to the top of this thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_message_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- messages ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Bot,
    System,
    Tool,
}

/// A file/image attached to a message or used as an avatar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    /// Original filename.
    pub name: String,
    pub mime: String,
    /// `image` renders inline; anything else is a file chip.
    #[serde(default = "default_attachment_kind")]
    pub kind: String,
    /// Size in bytes.
    #[serde(default)]
    pub size: i64,
    pub created_at: i64,
    /// Server-side path (not serialized to clients).
    #[serde(skip)]
    pub path: String,
}

fn default_attachment_kind() -> String {
    "file".into()
}

/// An emoji reaction on a message. `by` is "user" or a member bot id.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    #[serde(default = "default_reactor")]
    pub by: String,
}

fn default_reactor() -> String {
    "user".into()
}

/// Attribution for a bot message inside a room thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FromBot {
    pub bot_id: String,
    pub name: String,
    #[serde(default)]
    pub avatar_seed: u8,
}

/// One chat message. Rich payloads (approvals, tool calls, thinking) ride in
/// `segments` JSON so the wire format stays stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: Role,
    /// `text` (normal), `activity` (status line), `options` (card).
    #[serde(default = "default_msg_kind")]
    pub kind: String,
    pub text: String,
    /// Engine that produced the message (bot messages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Ordered content segments: text, thinking, tool calls, cards.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<Segment>,
    /// Attachments carried by this message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    /// Emoji reactions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<Reaction>,
    /// Message this one flat-replies to (inline quote).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Display name of the person who sent this user message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    /// Room attribution: which member bot said this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_bot: Option<FromBot>,
    /// A user message that arrived through the HTTP API (not the app).
    #[serde(default)]
    pub via_api: bool,
    /// User message sent INTO a running turn (steer).
    #[serde(default)]
    pub steered: bool,
    /// User message waiting in the queue while the bot is mid-turn.
    #[serde(default)]
    pub queued: bool,
    /// Provider turn id that produced this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Streaming in progress flag.
    #[serde(default)]
    pub pending: bool,
    #[serde(default)]
    pub error: Option<String>,
    pub created_at: i64,
}

fn default_msg_kind() -> String {
    "text".into()
}

impl Message {
    /// Blank message skeleton — callers set id/role/text/thread.
    pub fn blank(thread_id: &str, role: Role, text: String) -> Self {
        Message {
            id: new_id("msg"),
            thread_id: thread_id.into(),
            role,
            kind: default_msg_kind(),
            text,
            engine: None,
            model: None,
            segments: vec![],
            attachments: vec![],
            reactions: vec![],
            reply_to: None,
            sender: None,
            from_bot: None,
            via_api: false,
            steered: false,
            queued: false,
            turn_id: None,
            pending: false,
            error: None,
            created_at: now_ms(),
        }
    }
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
    /// The tool name — the "always allow" grant remembers this key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Choice labels for multi-option questions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// Why this card is waiting, when held by guard/mode/sandbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
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

// ---- rooms --------------------------------------------------------------------

/// Who answers an unattended room message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Responder {
    /// One named member answers.
    Member { bot_id: String },
    /// Every member takes a turn, in roster order.
    Everyone,
    /// Only @-mentioned members answer (default when unset).
    Mentions,
}

impl Default for Responder {
    fn default() -> Self {
        Responder::Mentions
    }
}

/// A group channel: one thread shared by several member bots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub member_ids: Vec<String>,
    /// The room's chat thread.
    pub thread_id: String,
    #[serde(default)]
    pub responder: Responder,
    /// Aggregate transient flag: a member turn is running.
    #[serde(default)]
    pub working: bool,
    #[serde(default)]
    pub unread: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

// ---- computers --------------------------------------------------------------

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

// ---- events ---------------------------------------------------------------

/// Event envelope broadcast on `GET /api/events` (SSE `data:` payloads).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A message was created or updated (streaming snapshots included).
    MessageUpsert { message: Message },
    /// A message was deleted.
    MessageDeleted { id: String, thread_id: String },
    /// Bot roster entry changed (name, model, working flag, unread…).
    BotUpsert { bot: Bot },
    /// Room entry changed.
    RoomUpsert { room: Room },
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

// ---- request payloads -------------------------------------------------------

/// Request body for `POST /api/threads/:id/messages` — a user chat send.
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessage {
    pub text: String,
    /// Override the bot's model for this turn.
    #[serde(default)]
    pub model: Option<String>,
    /// Flat reply reference.
    #[serde(default)]
    pub reply_to: Option<String>,
    /// Attachment ids uploaded via POST /api/attachments.
    #[serde(default)]
    pub attachments: Vec<String>,
    /// Sender display name (shared/multi-user).
    #[serde(default)]
    pub sender: Option<String>,
}

/// Harness-wide settings persisted in the store's kv table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    /// Data directory holding the SQLite db, SOUL.md files and attachments.
    pub data_dir: String,
    /// The operator's display name (sender attribution on user messages).
    #[serde(default)]
    pub profile_name: String,
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
