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

/// A lightweight organizational label within one bot (WireBot.projects).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotProject {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
}

/// Public, package-authored playbook installed for a bot — process guidance
/// only, never executable code (WireBot.playbooks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlaybook {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub instructions: String,
}

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
    #[serde(default)]
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

    // --- parity fields with the OpenMausBot WireBot (persisted in extra) ---
    /// Task groupings (labels only, never directories).
    #[serde(default)]
    pub projects: Vec<BotProject>,
    /// Model variant (provider-specific sub-mode), part of modelSelection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_variant: Option<String>,
    /// sha256 of the SOUL.md mirror — set at read time, never persisted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soul_hash: Option<String>,
    /// The SOUL.md mirror differed from `soul` at the last check.
    #[serde(default)]
    pub soul_drift: bool,
    /// Mascot skin ids — expressive avatar overlay (WireBot.mascot*).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mascot_expression: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mascot_body: Option<String>,
    /// Crop applied to the avatar image ({x,y,w,h}).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_crop: Option<String>,
    /// Where the bot works: cloud | vm | local | browser | off (unset = auto).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    /// Which cloud backend backs computer=cloud ("box" | "vps").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud_backend: Option<String>,
    /// Auto mode may prepare/start this bot's managed VPS container.
    #[serde(default)]
    pub auto_start_vps: bool,
    /// Speak this bot's replies aloud as they settle (TTS opt-in).
    #[serde(default)]
    pub speak_replies: bool,
    /// This bot's voice id, so a room of bots doesn't sound like one person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    /// true after an edit/branch-switch rewound the visible conversation.
    #[serde(default)]
    pub rewound: bool,
    /// Coordinator for this bot's sidebar section (chief-of-staff).
    #[serde(default)]
    pub chief_of_staff: bool,
    /// Owner-selected additional sections this chief may coordinate.
    #[serde(default)]
    pub managed_sections: Vec<String>,
    /// Pause for human approval before this bot talks to a peer bot.
    #[serde(default)]
    pub approve_peer_comms: bool,
    /// Bot ids this bot is allowed to contact (empty = all).
    #[serde(default)]
    pub peers: Vec<String>,
    /// Whether this bot may use the workspace's connected apps (Composio).
    #[serde(default)]
    pub composio: bool,
    /// Whether this bot gets the built-in browser engine.
    #[serde(default)]
    pub browser: bool,
    /// App-wide MCP server names this bot mounts.
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// Named browser profile id; absent = the bot's own private session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_profile: Option<String>,
    /// Public, package-authored playbooks installed for this bot.
    #[serde(default)]
    pub playbooks: Vec<InstalledPlaybook>,
    /// The one message pinned to the top of this bot's active thread —
    /// computed mirror, set at read time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_message_id: Option<String>,
}

fn default_approval_mode() -> ApprovalMode {
    ApprovalMode::Ask
}
fn default_true() -> bool {
    true
}

// ---- threads ---------------------------------------------------------------

/// A chat thread. Bots own `direct` threads (one primary + extra tasks);
/// rooms own `room` threads (WireTask / GroupTask).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: String,
    /// `direct` (bot DM) or `room` (group channel task).
    pub kind: String,
    /// Owning bot for direct threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    /// Owning room for room threads (GroupTask linkage).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    pub title: Option<String>,
    /// The one message pinned to the top of this thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_message_id: Option<String>,
    /// The first message already drove a title attempt — later sends do not
    /// rename a thread the person may have retitled.
    #[serde(default)]
    pub title_from_first_message: bool,
    /// When the person archived this thread; absent = unarchived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<i64>,
    /// Folder this task's turns run in, pinned on its first turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// true after an edit/branch-switch rewound the visible conversation.
    #[serde(default)]
    pub rewound: bool,
    /// Epoch ms when the current busy stretch began (elapsed readout anchor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_started_at: Option<i64>,
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

/// Comm chip — "Messaged @X" linking to a bot-bot channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommChip {
    pub group_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub with_bot_id: String,
    pub with_name: String,
    #[serde(default)]
    pub with_seed: u8,
}

/// Thread chip — "Opened thread #Title on @X".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRef {
    pub bot_id: String,
    pub thread_id: String,
    pub title: String,
}

/// Peer-comms provenance: a bot pushed this line in / delivered it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub unattended: bool,
}

/// Durable delivery identity for room requests (roomRequest wire field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomRequest {
    pub id: String,
    /// `request` | `result`
    pub phase: String,
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

    // --- parity fields with the OpenMausBot WireMessage (extra column) ---
    /// The message this one follows in the reply tree; null = thread root.
    /// Flat reply (`reply_to`) stays the user-facing inline quote.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    /// Stable client id for at-most-once chat POST retries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_id: Option<String>,
    /// Per-send channel behavior — `chat` (default) or `goal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_mode: Option<String>,
    /// The last assistant text item from a settled provider turn.
    #[serde(default)]
    pub turn_terminal: bool,
    /// Steer-queue entry id this drained user line came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue_id: Option<String>,
    /// screen messages: a frame of the bot's computer (base64 image).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png: Option<String>,
    /// Mime of an inline screen/frame payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
    /// Comm chip linking to a bot-bot channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comm: Option<CommChip>,
    /// Thread chip linking to a task thread on another bot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_ref: Option<ThreadRef>,
    /// Set on a room message a bot pushed in with post_to_room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_post: Option<PeerRef>,
    /// Set on a user-role line another bot delivered into this conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_ask: Option<PeerRef>,
    /// Durable delivery identity for room requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room_request: Option<RoomRequest>,
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
            parent_id: None,
            send_id: None,
            channel_mode: None,
            turn_terminal: false,
            queue_id: None,
            png: None,
            mime: None,
            comm: None,
            thread_ref: None,
            peer_post: None,
            peer_ask: None,
            room_request: None,
        }
    }
}

/// A piece of a message — rendered in order by clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
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
    /// Catalog key for `held` when it is one of the fixed notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held_code: Option<String>,
    /// The provider can remember an allow for the rest of its session.
    #[serde(default)]
    pub allow_session: bool,
    /// Dismissed without answering (card goes quiet, engine sees deny).
    #[serde(default)]
    pub dismissed: bool,
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
#[derive(Default)]
pub enum Responder {
    /// One named member answers.
    Member { bot_id: String },
    /// Every member takes a turn, in roster order.
    Everyone,
    /// Only @-mentioned members answer (default when unset).
    #[default]
    Mentions,
}

/// A group channel: member bots share one or more task threads
/// (WireGroup). `thread_id` is the *active* task; `kind='room'` threads
/// carrying `room_id` are this room's task list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub member_ids: Vec<String>,
    /// The active task's thread.
    pub thread_id: String,
    #[serde(default)]
    pub responder: Responder,
    /// The room's shared instructions, prepended to every member's persona.
    #[serde(default)]
    pub bulletin: String,
    /// true for auto-created bot-bot channels.
    #[serde(default)]
    pub dm: bool,
    /// Sidebar section heading this room is filed under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// The room's shared desk — overrides each member's default cwd.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Aggregate transient flag: a member turn is running.
    #[serde(default)]
    pub working: bool,
    /// Transient: the member currently running a turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub busy_bot_id: Option<String>,
    /// Transient: when the busy member's turn started (elapsed readout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_started_at: Option<i64>,
    /// New user-created rooms start with setup pending (null = pending).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_completed_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_skipped_at: Option<i64>,
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
    /// Bot removed from the roster.
    BotDeleted { bot_id: String },
    /// Room removed.
    RoomDeleted { room_id: String },
    /// Steer-queue snapshot for a thread (bot.queued frame).
    QueuedMessages {
        thread_id: String,
        items: Vec<QueuedItem>,
    },
    /// A transient notification for the UI (toast).
    Notify { title: String, body: String },
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

/// One pending steer-queue entry, as `queued_messages` frames carry them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedItem {
    pub queue_id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

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
    /// Stable client id — retrying the same send_id is idempotent.
    #[serde(default)]
    pub send_id: Option<String>,
    /// Per-send channel behavior: absent/"chat" = quick chat, "goal" =
    /// a bounded multi-bot channel goal (reserved).
    #[serde(default)]
    pub channel_mode: Option<String>,
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
