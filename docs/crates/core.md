# `pr-core` — Complete Crate Documentation

> The `pr-core` crate is the fundamental core of the Fathom Agent system.
> It contains all domain types, configuration, error management, memory, skills,
> result export, notifications, contacts, and CRM integration.
> All public types are re-exported via `pub use` from lib.rs.

---

## Table of Contents

1. [lib.rs — Entry Point and HTTP Client](#1-librs)
2. [ids.rs — Identifier Types](#2-idsrs)
3. [message.rs — LLM Message Format](#3-messagers)
4. [agent.rs — Agents and Their States](#4-agentrs)
5. [event.rs — Event System](#5-eventrs)
6. [finding.rs — Research Findings](#6-findingrs)
7. [tool.rs — Tools and Their Output](#7-toolrs)
8. [config.rs — Application Configuration](#8-configrs)
9. [error.rs — Error System](#9-errorrs)
10. [token.rs — Token Counting](#10-tokenrs)
11. [memory.rs — File-Based Memory Storage](#11-memoryrs)
12. [skill.rs — Skill System](#12-skillrs)
13. [session.rs — Session Results](#13-sessionrs)
14. [export.rs — Result Export](#14-exportrs)
15. [notify.rs — Notification System](#15-notifyrs)
16. [contact.rs — Contacts](#16-contactrs)
17. [crm.rs — CRM Integration](#17-crmrs)
18. [irc.rs — Process-Global Message Bus](#18-ircrs)
19. [steer.rs — Steering Channel Registry](#19-steerrs)
20. [async_job.rs — Async Job Manager](#20-async_jobrs)
21. [daemon.rs — Daemon Registry](#21-daemonrs)
22. [protected.rs — Protected Surfaces](#22-protectedrs)
23. [profile.rs — Profile System](#23-profilers)
24. [capability.rs — Capability Evidence & Window Profile](#24-capabilityrs)
11. [memory.rs — File-Based Memory Storage](#11-memoryrs)
12. [skill.rs — Skill System](#12-skillrs)
13. [session.rs — Session Results](#13-sessionrs)
14. [export.rs — Result Export](#14-exportrs)
15. [notify.rs — Notification System](#15-notifyrs)
16. [contact.rs — Contacts](#16-contactrs)
17. [crm.rs — CRM Integration](#17-crmrs)
18. [irc.rs — Process-Global Message Bus](#18-ircrs)
19. [steer.rs — Steering Channel Registry](#19-steerrs)
20. [async_job.rs — Async Job Manager](#20-async_jobrs)
21. [daemon.rs — Daemon Registry](#21-daemonrs)
22. [protected.rs — Protected Surfaces](#22-protectedrs)
23. [profile.rs — Profile System](#23-profilers)
24. [capability.rs — Capability Evidence & Window Profile](#24-capabilityrs)

---

## 1. lib.rs

The crate declares 23 public modules and re-exports all their public symbols via glob re-export (`pub use module::*`):

```
ids, message, agent, event, finding, tool, config, error,
token, memory, skill, session, export, notify, contact, crm,
irc, steer, async_job, daemon, protected, profile, capability
```

Additional modules:

| Module | Key types and purpose |
|--------|----------------------|
| `irc` | `IrcBus` (process-global message bus), `AgentRegistry` (agent discovery), `IrcReviver` (parked-agent revival hook), `PeerStatus`, `DeliveryReceipt` |
| `steer` | `SteerRegistry` — global steering channel registry for mid-run instructions |
| `async_job` | `AsyncJobManager` — in-process background job tracking with delivery sinks |
| `daemon` | `DaemonRegistry` — long-running process registry |
| `protected` | `ProtectedSurfaces` — security surfaces protection |
| `profile` | Profile system — TOML persona presets |
| `capability` | `CapabilityEvidence`, `WindowProfile` — context window negotiation |

This means crate consumers can write `use pr_core::AgentId` directly, without specifying the full path `pr_core::ids::AgentId`.

### `http_client()`

```rust
pub fn http_client() -> reqwest::Client
```

**Signature**: `() -> reqwest::Client`

**Algorithm**:
1. Creates `reqwest::Client::builder()`
2. Sets a general timeout (`timeout`) — **30 seconds**
3. Sets a connection timeout (`connect_timeout`) — **10 seconds**
4. Calls `.build()` to finalize the client
5. If `build()` fails, returns a default `reqwest::Client::new()` via `unwrap_or_else`

**Edge cases**: If `build()` fails, the default client has no timeouts — this is an emergency fallback.

**Interaction**: Used by `CrmSync`, `Notifier`, as well as search and LLM provider crates. Intended to replace `reqwest::Client::new()` throughout the codebase.

---

## 2. ids.rs

The module contains three type-safe newtype wrappers for identifiers. All three follow the same structure.

### `SessionId`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);
```

Research session identifier.

#### `SessionId::new()`

```rust
pub fn new() -> Self
```

Generates UUIDv7 via `Uuid::now_v7()`, converts to string. UUIDv7 embeds a millisecond-precision timestamp in the high bits, providing sortability (monotonic ordering).

#### `impl Display`

Outputs the inner string `self.0`.

### `AgentId`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);
```

Agent identifier within a session.

#### `AgentId::new()`

Same as `SessionId::new()` — UUIDv7.

### `FindingId`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingId(pub String);
```

Finding identifier.

#### `FindingId::new()`

UUIDv7.

**Interaction**: All three types implement `Hash`, `Eq`, `Clone`, and `Serialize`/`Deserialize`, allowing their use in HashMap, database, and serialization.

---

## 3. message.rs

### `Message`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "user")]
    User { content: String },
    #[serde(rename = "assistant")]
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
    },
}
```

Enum representing a message format compatible with the OpenAI Chat Completions API. Uses `#[serde(tag = "role")]` for discriminant serialization.

| Variant | Fields | Description |
|---------|--------|-------------|
| `System` | `content: String` | System prompt |
| `User` | `content: String` | User message |
| `Assistant` | `content: Option<String>`, `tool_calls: Vec<ToolCall>` | Assistant response |
| `Tool` | `tool_call_id: String`, `content: String` | Tool execution result |

#### `Message::system(content) -> Self`

```rust
pub fn system(content: impl Into<String>) -> Self
```

Converts `content` via `.into()` and wraps it in `Message::System`.

#### `Message::user(content) -> Self`

```rust
pub fn user(content: impl Into<String>) -> Self
```

Wraps in `Message::User`.

#### `Message::assistant(content) -> Self`

```rust
pub fn assistant(content: impl Into<String>) -> Self
```

Creates `Message::Assistant` with `content = Some(content.into())` and empty `tool_calls`.

#### `Message::assistant_with_tools(content, tool_calls) -> Self`

```rust
pub fn assistant_with_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self
```

Arbitrary content + list of tool calls. `content` may be `None`.

#### `Message::tool(call_id, content) -> Self`

```rust
pub fn tool(call_id: impl Into<String>, content: impl Into<String>) -> Self
```

Creates `Message::Tool`, binding the result to `tool_call_id`.

**Interaction**: Used by the LLM module when building the dialog history and by the agent module for passing results.

### `ToolCall`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}
```

Tool call in OpenAI format.

#### `ToolCall::new(id, name, arguments) -> Self`

```rust
pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: serde_json::Value) -> Self
```

**Algorithm**:
1. Converts `id` and `name` to `String`
2. Sets `call_type = "function"`
3. Serializes `arguments` to a JSON string via `serde_json::to_string`; on error — empty string

#### `ToolCall::name() -> &str`

Returns `&self.function.name`.

#### `ToolCall::arguments() -> serde_json::Value`

Deserializes the JSON string `self.function.arguments` into `Value`. On parse error — `json!({})`.

### `ToolCallFunction`

```rust
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}
```

Helper structure — function name and arguments as a JSON string.

---

## 4. agent.rs

### `AgentRole`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentRole {
    Coordinator,
    Researcher,
    Analyst,
    Verifier,
    Writer,
}
```

| Role | Purpose | Can spawn children? |
|------|---------|---------------------|
| `Coordinator` | Manages session, plans, synthesizes | ✅ |
| `Researcher` | Performs search and information extraction | ✅ |
| `Analyst` | Analyzes and cross-references data | ✅ |
| `Verifier` | Validates findings | ❌ |
| `Writer` | Produces the final report | ❌ |

#### `AgentRole::can_spawn_children() -> bool`

Returns `true` for `Coordinator`, `Researcher`, `Analyst`; `false` for `Verifier`, `Writer`.

#### `impl Display`

Returns the lowercase string of the variant.

### `AgentState`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum AgentState {
    Idle,
    Planning { query: String },
    Researching { sub_tasks: Vec<String> },
    Analyzing,
    Synthesizing,
    Writing,
    Complete,
    Error { message: String },
}
```

Current agent state. Uses discriminant serialization with the `"state"` field.

| State | Fields | Description |
|-------|--------|-------------|
| `Idle` | — | Waiting for a task |
| `Planning` | `query: String` | Forming a plan |
| `Researching` | `sub_tasks: Vec<String>` | Executing sub-tasks |
| `Analyzing` | — | Analyzing data |
| `Synthesizing` | — | Synthesizing results |
| `Writing` | — | Writing the report |
| `Complete` | — | Finished |
| `Error` | `message: String` | Error |

**Interaction**: Published via `AgentEvent::AgentStateChanged`, used by UI/logging.

### `AgentStatus`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Spawned,
    Running,
    Completed,
    Failed,
    Cancelled,
}
```

Agent lifecycle (unlike `AgentState`, which describes the current work phase).

### `AgentRecord`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub id: AgentId,
    pub session_id: String,
    pub parent_id: Option<AgentId>,
    pub role: AgentRole,
    pub task: String,
    pub status: AgentStatus,
    pub depth: u32,
    pub tokens_used: u64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

Full agent record stored in the database.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `AgentId` | Unique identifier |
| `session_id` | `String` | Session identifier |
| `parent_id` | `Option<AgentId>` | Parent agent (None for root) |
| `role` | `AgentRole` | Role |
| `task` | `String` | Task text |
| `status` | `AgentStatus` | Current status |
| `depth` | `u32` | Depth in the tree (0 = root) |
| `tokens_used` | `u64` | Tokens consumed |
| `created_at` | `DateTime<Utc>` | Creation time |
| `completed_at` | `Option<DateTime<Utc>>` | Completion time |

**Interaction**: Saved to SQLite/PostgreSQL via `pr-persistence`.

---

## 5. event.rs

### `AgentEvent`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent { ... }
```

Union type for all system events. Each variant has a unique `"type"` tag value.

| Variant | Tag | Fields |
|---------|-----|--------|
| `SessionStarted` | `session_started` | `id: SessionId`, `query: String` |
| `AgentSpawned` | `agent_spawned` | `id`, `parent`, `role`, `task`, `depth` |
| `AgentStateChanged` | `agent_state_changed` | `id: AgentId`, `state: AgentState` |
| `Finding` | `finding` | `agent_id`, `finding: Finding` |
| `ToolCallStarted` | `tool_call_started` | `agent_id`, `tool`, `args` |
| `ToolCallCompleted` | `tool_call_completed` | `agent_id`, `tool`, `result_preview`, `duration_ms` |
| `LlmStreamChunk` | `llm_stream_chunk` | `agent_id`, `chunk: String` |
| `AgentCompleted` | `agent_completed` | `id`, `summary`, `tokens_used` |
| `AgentFailed` | `agent_failed` | `id`, `error` |
| `SessionCompleted` | `session_completed` | `id`, `output_dir`, `total_tokens`, `total_agents` |
| `SessionFailed` | `session_failed` | `id`, `error` |

#### `AgentEvent::agent_id() -> Option<&AgentId>`

Pattern matching: returns `Some` for agent-oriented events (`AgentSpawned`, `AgentStateChanged`, `AgentCompleted`, `AgentFailed`, `Finding`, `ToolCallStarted`, `ToolCallCompleted`, `LlmStreamChunk`). `None` for session events.

#### `AgentEvent::session_id() -> Option<&SessionId>`

Returns `Some` for `SessionStarted`, `SessionCompleted`, `SessionFailed`.

---

## 6. finding.rs

### `Source`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub excerpt: String,
}
```

Source (reference) for a finding. The `excerpt` field defaults to empty.

### `Finding`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub agent_id: AgentId,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

| Field | Default Value | Description |
|-------|--------------|-------------|
| `sources` | `[]` | Sources |
| `confidence` | `0.5` | Confidence (0.0–1.0) |

**Interaction**: Published via `AgentEvent::Finding`, saved to `findings/*.md`.

---

## 7. tool.rs

### `ToolSchema`

```rust
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}
```

Tool schema in OpenAI `tools` API format. `parameters` is arbitrary JSON Schema.

### `ToolOutput`

```rust
pub struct ToolOutput {
    pub success: bool,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub error_code: Option<String>,
}
```

Tool execution result. Error codes: `rate_limited`, `timeout`, `blocked`, `not_found`, `network`, `parse`, `other`.

#### `ToolOutput::ok(content) -> Self`

Creates a successful result without metadata.

#### `ToolOutput::ok_with_meta(content, metadata) -> Self`

Successful result with metadata.

#### `ToolOutput::err(content) -> Self`

Error without code.

#### `ToolOutput::err_code(content, code) -> Self`

Error with a machine-readable code for retry logic.

**Interaction**: All tools return `ToolOutput`. Agent retry logic uses `error_code`: `rate_limited` → retry with delay, `timeout` → retry, `blocked` → skip.

---

## 8. config.rs

### `AppConfig`

```rust
pub struct AppConfig {
    pub llm: LlmConfig,
    pub agent: AgentConfig,
    pub search: SearchConfig,
    pub output: OutputConfig,
    pub mcp: McpConfig,
    pub context: ContextConfig,
    pub export: ExportConfig,
    pub notifications: NotificationsConfig,
    pub contacts: ContactsConfig,
    pub crm: CrmConfig,
    pub hooks: Vec<HookConfig>,
}
```

Root configuration. Loaded from `~/.fathom/config.toml`.

#### `AppConfig::load() -> anyhow::Result<Self>`

**Algorithm**:
1. Gets the path via `Self::config_path()`
2. If the file exists — reads and parses TOML
3. If not — returns `Self::default()`

**Edge cases**: Invalid TOML → error. Missing file → defaults.

#### `AppConfig::config_path() -> anyhow::Result<PathBuf>`

Returns `~/.fathom/config.toml`. Error if no home directory.

#### `AppConfig::save(&self) -> anyhow::Result<()>`

Serializes to TOML, writes to file. Creates directory if needed.

### `LlmConfig`

```rust
pub struct LlmConfig {
    pub provider: String,     // "deepseek"
    pub base_url: String,     // "https://api.deepseek.com"
    pub api_key: String,      // ""
    pub model: String,        // "deepseek-chat"
    pub max_tokens: u32,      // 8192
    pub temperature: f32,     // 0.7
}
```

### `AgentConfig`

```rust
pub struct AgentConfig {
    pub max_depth: u32,                  // 2
    pub max_agents: u32,                 // 20
    pub max_iterations: u32,             // 50
    pub timeout_seconds: u64,            // 600
    pub use_multiprocess: bool,          // false
    pub max_concurrent_children: u32,    // 4
    pub stall_warn_seconds: u64,         // 450
    pub stall_kill_seconds: u64,         // 1200
    pub deny_tools: HashMap<String, Vec<String>>,
    pub role_models: HashMap<String, String>,
    pub session_token_limit: u64,        // 0
}
```

- `deny_tools` — tools denied by role (example: `verifier = ["shell"]`)
- `role_models` — model override by role (example: `researcher = "cheap-model"`)
- `session_token_limit` — session token limit (0 = disabled)

### `SearchConfig`

```rust
pub struct SearchConfig {
    pub backend: String,  // "hybrid"
    pub linkup: Option<LinkupConfig>,
    pub parallel: Option<ParallelConfig>,
    pub exa: Option<ExaConfig>,
    pub tavily: Option<TavilyConfig>,
    pub serper: Option<SerperConfig>,
    pub brave: Option<BraveConfig>,
}
```

Backends: `linkup`, `exa`, `tavily`, `serper`, `brave`, `parallel`, `duckduckgo`, `hybrid`, `smart`.

### `ContextConfig`

```rust
pub struct ContextConfig {
    pub context_window: u32,         // 128_000
    pub compact_threshold: f32,      // 0.50
    pub tool_output_max_bytes: u32,  // 50_000
    pub tool_output_max_lines: u32,  // 2_000
    pub turn_budget_bytes: u32,      // 200_000
}
```

### `ExportConfig`

```rust
pub struct ExportConfig {
    pub format: String, // "html"
}
```

#### `ExportConfig::parsed_format() -> ExportFormat`

Parses the string into `ExportFormat`. Unknown format → `Html`.

### `NotificationsConfig`

```rust
pub struct NotificationsConfig {
    pub webhook_url: String,
    pub email_to: String,
    pub email_from: String,
    pub smtp_host: String,
    pub smtp_port: u16,           // 587
    pub smtp_username: String,
    pub smtp_password: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
}
```

### `ContactsConfig`

```rust
pub struct ContactsConfig {
    pub db_path: String,  // "./contacts.db"
    pub pg_url: String,   // ""
}
```

### `CrmConfig`

```rust
pub struct CrmConfig {
    pub provider: String,
    pub domain: String,
    pub api_key: String,
}
```

#### `CrmConfig::is_configured() -> bool`

Returns `true` if `provider.trim()` is not empty.

### `HookConfig`

```rust
pub struct HookConfig {
    pub event: String,     // "PreToolUse", "PostToolUse", "Stop"
    pub command: String,
    pub args: Vec<String>,
    pub tool: String,
    pub timeout_ms: u64,   // 5000
}
```

Lifecycle hook — an external process receiving JSON on stdin.

### `McpConfig` / `McpServerConfig`

```rust
pub struct McpConfig {
    pub servers: Vec<McpServerConfig>,
}

pub struct McpServerConfig {
    pub name: String,
    pub transport: String,  // "stdio" | "http"
    pub command: Option<String>,
    pub args: Vec<String>,
    pub url: Option<String>,
}
```

### `set_config_value(key, value)`

```rust
pub fn set_config_value(key: &str, value: &str) -> anyhow::Result<()>
```

Sets a configuration value by dot-separated path (e.g., `llm.api_key`).

**Algorithm (step-by-step)**:
1. **Path**: `AppConfig::config_path()` → `~/.fathom/config.toml`
2. **Load**: If the file exists — parses into `toml::Value`; otherwise — empty table
3. **Type check**: `lookup_value(&root, key)` — if the existing value is already a string, the new value will also be a string. This prevents the problem: `"42"` for `telegram_chat_id` won't turn into a number
4. **Value parsing**: Order: `bool` → `i64` → `f64` → `String`. If the existing value is a string — force string
5. **Insertion**: `set_nested_value(&mut root, key, parsed)` — creates intermediate tables if needed
6. **Validation**: Serializes back to TOML, tries to deserialize into `AppConfig`. Error → file is not written
7. **Atomic write**: Writes to `config.toml.tmp`, then `rename` to `config.toml`

**Edge cases**:
- Empty key / `..` → error
- Intermediate value is not a table → error
- Unknown key → validation error at step 6
- `"42"` for a string field → saved as a string

### `lookup_value(root, key)` (private)

Descends the dot-separated path in the TOML tree. Returns `None` if a segment is not found.

### `set_nested_value(root, key, value)` (private)

Splits `key` by `.`, for each segment (except the last) creates intermediate tables. The last segment inserts the value.

---

## 9. error.rs

### `PrError`

```rust
#[derive(Debug, Error)]
pub enum PrError {
    Llm(String),
    Tool(String),
    Agent(String),
    Persistence(String),
    Config(String),
    Timeout(u64),
    MaxDepthReached(u32),
    MaxAgentsReached(u32),
    MaxIterationsReached(u32),
    Cancelled,
    Http { status: u16, message: String, retry_after: Option<u64> },
    ResponseTooLarge(String),
}
```

| Variant | Output Format | Retryable? |
|---------|--------------|------------|
| `Llm(msg)` | `"LLM error: {msg}"` | ✅ |
| `Tool(msg)` | `"Tool error: {msg}"` | ❌ |
| `Agent(msg)` | `"Agent error: {msg}"` | ❌ |
| `Persistence(msg)` | `"Persistence error: {msg}"` | ❌ |
| `Config(msg)` | `"Config error: {msg}"` | ❌ |
| `Timeout(secs)` | `"Timeout after {secs}s"` | ✅ |
| `MaxDepthReached(d)` | `"Max depth reached ({d})"` | ❌ |
| `MaxAgentsReached(n)` | `"Max agents reached ({n})"` | ❌ |
| `MaxIterationsReached(n)` | `"Max iterations reached ({n})"` | ❌ |
| `Cancelled` | `"Cancelled"` | ❌ |
| `Http { status, message, retry_after }` | `"API error {status}: {message}"` | 408/429/5xx → ✅ |
| `ResponseTooLarge(msg)` | `"response too large, streaming required: {msg}"` | ❌ |

#### `PrError::is_retryable(&self) -> bool`

- `Http` with status 408, 429, ≥ 500 → `true`
- `Llm(_)` → `true` (transport errors)
- `Timeout(_)` → `true`
- Others → `false`

#### `PrError::retry_after_secs(&self) -> Option<u64>`

For `Http { retry_after: Some(n), .. }` → `Some(n)`. Others → `None`.

### `PrResult<T>`

```rust
pub type PrResult<T> = Result<T, PrError>;
```

---

## 10. token.rs

### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `IMAGE_TOKEN_COST` | `1500` | Image cost |
| `MESSAGE_OVERHEAD` | `4` | Overhead tokens per message |
| `TOOL_CALL_OVERHEAD` | `10` | Overhead tokens per tool call |

### `estimate_tokens(text) -> u32`

Estimates the number of tokens without a real tokenizer.

**Algorithm** (`estimate_tokens_structured`):
1. Counts ASCII characters (code ≤ 0x7F) separately
2. For non-ASCII characters calls `char_cost(ch)`
3. ASCII: `(count + 3) / 4` (4 chars ≈ 1 token, rounded up)
4. Returns `ascii_tokens + other_tokens`

### `char_cost(ch) -> u32` (private)

| Range | Script | Cost |
|-------|--------|------|
| `0x00..0x7F` | ASCII | 0 (bulk) |
| `0x4E00..0x9FFF` | CJK Unified | 1 |
| `0x3400..0x4DBF` | CJK Extension A | 1 |
| `0xF900..0xFAFF` | CJK Compatibility | 1 |
| `0x20000..0x2A6DF` | CJK Extension B | 1 |
| `0xAC00..0xD7AF` | Hangul | 1 |
| `0x1100..0x11FF` | Hangul Jamo | 1 |
| `0x3040..0x309F` | Hiragana | 1 |
| `0x30A0..0x30FF` | Katakana | 1 |
| `0x31F0..0x31FF` | Katakana Phonetic | 1 |
| `0x2E80..0x2FDF` | CJK/Kangxi Radicals | 1 |
| `0x3000..0x303F` | CJK Symbols | 1 |
| rest | Cyrillic, emoji, etc. | 1 |

### `estimate_message_tokens(msg) -> u32`

**Algorithm**:
1. Starts with `MESSAGE_OVERHEAD` (4)
2. For `System`/`User` — text cost
3. For `Assistant` — text (if present) + each `ToolCall`
4. For `Tool` — cost of `tool_call_id` + `content`

### `estimate_tool_call_tokens(tc) -> u32` (private)

`TOOL_CALL_OVERHEAD` (10) + cost of `id` + `name` + `arguments` (JSON string).

### `estimate_messages_tokens(messages) -> u32`

Sum of `estimate_message_tokens` for each message.

### `estimate_schemas_tokens(schemas) -> u32`

For each schema: serialize to JSON → estimate tokens. Sums them up.

---

## 11. memory.rs

### Constants

| Constant | Value |
|----------|-------|
| `MEMORY_FILENAME` | `"MEMORY.md"` |
| `USER_FILENAME` | `"USER.md"` |
| `DEFAULT_MAX_MEMORY_CHARS` | `2200` |
| `DEFAULT_MAX_USER_CHARS` | `1375` |
| `ENTRY_DELIMITER` | `'§'` (U+00A7) |

### `MemoryType`

```rust
pub enum MemoryType { User, Feedback, Project, Reference }
```

Typed memory categories.

#### `impl FromStr`

Case-insensitive parsing. Unknown string → error.

### `Frontmatter`

```rust
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub memory_type: MemoryType,
}
```

#### `Frontmatter::to_frontmatter_string() -> String`

Generates a YAML block `---\nname: ...\ndescription: ...\ntype: ...\n---`.

### `parse_frontmatter(input) -> Option<(Frontmatter, &str)>`

**Algorithm**:
1. Checks for `---` at the beginning
2. Looks for the second `---`
3. Parses `name:`, `description:`, `type:` keys from the block
4. If `name` is empty → `None`
5. Returns `(Frontmatter, body)`

### `escape_yaml_value(s) -> String` (private)

Wraps in quotes if it contains `:`, `#`, `"`, `'` or starts/ends with whitespace.

### `MemoryEntry`

```rust
pub struct MemoryEntry {
    pub content: String,
    pub created_at: DateTime<Utc>,
}
```

#### `MemoryEntry::new(content) -> Self`

Sets `content` and `created_at = Utc::now()`.

### `TypedMemoryEntry`

```rust
pub struct TypedMemoryEntry {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub created_at: DateTime<Utc>,
}
```

#### `TypedMemoryEntry::to_entry_string() -> String`

`frontmatter + "\n" + body`.

### `MemoryOp`, `MemoryAction`, `MemoryTarget`

```rust
pub struct MemoryOp {
    pub action: MemoryAction,
    pub target: MemoryTarget,
    pub content: Option<String>,
    pub old_text: Option<String>,
}

pub enum MemoryAction { Add, Replace, Remove }
pub enum MemoryTarget { Memory, User }
```

### `MemoryStore`

```rust
pub struct MemoryStore {
    memory_path: PathBuf,
    user_path: PathBuf,
    pub max_memory_chars: usize,
    pub max_user_chars: usize,
}
```

File-based memory storage. Entries are separated by `§` in files. A character budget limits injection into the prompt.

#### `MemoryStore::new(home_dir) -> Self`

Path: `home_dir/.fathom/memory/{MEMORY,USER}.md`. Budgets: 2200 / 1375.

#### `MemoryStore::with_budgets(home_dir, max_memory_chars, max_user_chars) -> Self`

With custom budgets (for testing).

#### `MemoryStore::memory_path() -> &Path` / `user_path() -> &Path`

Getters.

#### `MemoryStore::load_memory() -> Vec<MemoryEntry>`

Reads `MEMORY.md`, splits by `§`, returns entries. File does not exist → empty `Vec`.

#### `MemoryStore::load_user() -> Vec<MemoryEntry>`

Same for `USER.md`.

#### `MemoryStore::load_entries_from_path(path) -> Vec<MemoryEntry>` (static, public)

Load from an arbitrary path.

#### `MemoryStore::add_memory(content) -> Result<()>`

**Algorithm** (`add_to_file`):
1. Trim; empty → error
2. Load existing entries
3. Exact duplicate → `Ok(())` (idempotence)
4. Add new entry
5. Serialize via `§\n`
6. Apply budget (`enforce_budget`) — remove old ones if exceeded
7. Atomic write (`atomic_write`)

#### `MemoryStore::add_user(content) -> Result<()>`

Same for `USER.md`.

#### `MemoryStore::replace_memory(old_substr, new_content) -> Result<()>`

**Algorithm**:
1. Load entries
2. Find the first one containing `old_substr`
3. Not found → error
4. Replace, apply budget, atomic write

#### `MemoryStore::remove_memory(substr) -> Result<()>`

Removes all entries containing `substr`. Not found → error.

#### `MemoryStore::batch_operations(ops) -> Result<()>`

**Algorithm**:
1. Load both files into memory
2. Apply each operation:
   - **Add**: check for duplicate, add
   - **Replace**: search by `old_text`, replace
   - **Remove**: delete by `old_text`; not found → entire batch fails
3. If error → **entire batch is aborted**, files are not written
4. Serialize both files, apply budgets
5. Atomic write both files

**Atomicity guarantee**: If an error occurs mid-way, changes are not written. The two files are written sequentially (not truly atomic between them).

#### `MemoryStore::to_system_prompt_block() -> String`

Generates a block for the system prompt:
```
## Memory

### Persistent Memory
- entry1
- entry2

### User Context
- entry1
```

Empty lists do not generate a section.

#### `serialize_entries(entries) -> String` (private, static)

Joins content via `§\n`.

#### `enforce_budget(content, max_chars) -> String` (private, static)

**Algorithm**:
1. If `len <= max_chars` → return as-is
2. Split by `§`
3. Iterate **from the end** (newest): collect entries while budget is not exhausted
4. Reverse the result (since iteration went from the end)

Newest entries are **prioritized** — old ones are removed first.

#### `atomic_write(path, content) -> Result<()>` (private, static)

1. Create parent directories
2. Write to a temporary file `*.md.tmp`
3. `rename` temporary → target (atomic filesystem operation)

---

## 12. skill.rs

### `Skill`

```rust
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub file_path: PathBuf,
    pub created_at: DateTime<Utc>,
}
```

#### `Skill::from_file(path) -> Result<Self>`

**Algorithm**:
1. Read the file
2. `parse_skill_header` extracts the name (`# Name`) and description (first non-empty line after the header)
3. If the name is empty → fallback to the directory name → `"unnamed"`
4. `content` = full file content

### `parse_skill_header(content) -> (String, String)` (private)

Looks for a `# ` header, then the first non-empty non-heading line as the description.

### `SkillRegistry`

```rust
pub struct SkillRegistry {
    skills_dir: PathBuf,
    skills: Vec<Skill>,
}
```

#### `SkillRegistry::new(home_dir) -> Self`

`skills_dir = home_dir/.fathom/skills/`.

#### `SkillRegistry::with_dir(skills_dir) -> Self`

With an arbitrary directory (for testing).

#### `SkillRegistry::skills_dir() -> &Path`

Getter.

#### `SkillRegistry::discover() -> Result<()>`

**Algorithm**:
1. Clear the skill list
2. If the directory does not exist → Ok
3. Recursively scan the directory
4. For each `SKILL.md` file → `Skill::from_file(&path)`
5. Load error → log warning, continue

**Edge cases**: Non-existent directory → empty registry. Corrupted file → log warning.

#### `SkillRegistry::create_from_experience(task, approach) -> Result<()>`

**Algorithm**:
1. Generate a slug from `task` via `slugify()`
2. Create directory `skills_dir/<slug>/`
3. Write `SKILL.md` with the format:
   ```
   # <task>

   Skill learned from: <task>

   ## Approach

   <approach>
   ```
4. Add the skill to the registry

**Interaction**: Called by the Writer/Coordinator agent when it discovers a reusable work pattern.

#### `SkillRegistry::get_skill(name) -> Option<&Skill>`

Search by name, **case-insensitive**.

#### `SkillRegistry::all_skills() -> &[Skill]`

All discovered skills.

#### `SkillRegistry::to_system_prompt_block() -> String`

**Algorithm**:
1. If no skills → empty string
2. Builds:
   ```
   ## Available Skills

   Load a skill's full instructions with the `skill` tool before following its workflow.

   ### <name>
   <description>
   <location>/path/to/SKILL.md</location>
   ```

### `slugify(input) -> String` (private)

**Algorithm**:
1. Each character: alphanumeric / `-` / `_` → lowercase; space / `/` → `-`; rest → `-`
2. Split by `-`, filter empty, join via `-`

Examples: `"Hello World"` → `"hello-world"`, `"a/b/c"` → `"a-b-c"`, `"special!@#chars"` → `"special-chars"`.

---

## 13. session.rs

### `SessionOutput`

```rust
pub struct SessionOutput {
    pub session_id: SessionId,
    pub output_dir: PathBuf,
    pub synthesis: String,
    pub total_tokens: u64,
    pub total_agents: u32,
}
```

Final result of a completed research session. Created by the coordinator after all agents finish.

#### `SessionOutput::summary_line() -> String`

**Algorithm**: Formats: `"Research session {id} completed: {agents} agent(s), {tokens} tokens. Output: {dir}"`.

#### `SessionOutput::synthesis_preview(max_chars) -> String`

**Algorithm**:
1. Takes the first `max_chars` characters via `.chars().take(max_chars)`
2. If the text is longer → appends `…` (Unicode ellipsis)

**Edge cases**: Empty synthesis → empty string. `max_chars = 0` → `"…"`.

**Interaction**: Used by notifications to form email/Telegram/webhook text.

---

## 14. export.rs

### `ExportFormat`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat { Pdf, Html, Json, Docx }
```

#### `ExportFormat::parse(s) -> Option<Self>`

Case-insensitive parsing. `Some` for `"pdf"`, `"html"`, `"json"`, `"docx"`. Otherwise `None`.

#### `ExportFormat::as_str() -> &'static str` / `extension() -> &'static str`

Returns the lowercase string of the format. `extension()` = `as_str()`.

#### `ExportFormat::all() -> [ExportFormat; 4]`

All variants.

#### `impl FromStr`

Returns an error with a description of valid values.

### `ContactExportFormat`

```rust
pub enum ContactExportFormat { Csv, VCard, Json, Excel }
```

#### `ContactExportFormat::parse(s) -> Option<Self>`

`"csv"`, `"vcard"`, `"vcf"`, `"json"`, `"excel"`, `"xlsx"` → `Some`. Otherwise `None`.

#### `ContactExportFormat::extension() -> &'static str`

`Csv` → `"csv"`, `VCard` → `"vcf"`, `Json` → `"json"`, `Excel` → `"xlsx"`.

### `Exporter`

```rust
pub struct Exporter {
    output_dir: PathBuf,
}
```

#### `Exporter::new(output_dir) -> Self`

#### `Exporter::output_dir() -> &Path`

#### `Exporter::export(session, format) -> Result<PathBuf>`

**Algorithm**:
1. Create `output_dir` if it does not exist
2. Delegate to `export_html`, `export_json`, `export_pdf`, `export_docx`
3. Return the path to the created file

#### `Exporter::target_path(format) -> PathBuf`

`output_dir/report.<ext>`.

#### `Exporter::contacts_target_path(format) -> PathBuf`

`output_dir/contacts.<ext>`.

#### `Exporter::export_contacts(contacts, format) -> Result<PathBuf>`

**Algorithm**: Create directory, delegate to format-specific method.

### HTML Export (`export_html`)

**Algorithm**:
1. Call `build_report_markdown(session)` to generate Markdown
2. Call `render_html_document(session, &markdown)` to render into HTML
3. Write to `output_dir/report.html`

### `render_html_document(session, markdown) -> String`

**Algorithm**:
1. Create `pulldown_cmark::Parser` with extensions: tables, footnotes, strikethrough, tasklists
2. Render Markdown to HTML via `html::push_html`
3. Embed into a full HTML template with CSS styles:
   - System fonts (Apple, Segoe UI, Roboto)
   - Max width 860px, centered
   - Styles for `h1`, `h2`, `h3`, `code`, `pre`, `blockquote`, `table`, `a`, `hr`
   - CSS `color-scheme: light dark` for dark theme support
   - Meta viewport tags
4. Add footer: `"Generated by Fathom Agent · session {id} · {timestamp}"`
5. Escape `session_id` via `html_escape` (replace `&`, `<`, `>`, `"`)

### `html_escape(s) -> String` (private)

Replaces `&` → `&amp;`, `<` → `&lt;`, `>` → `&gt;`, `"` → `&quot;`.

### JSON Export (`export_json`)

**Algorithm**:
1. Collect findings via `collect_findings`
2. For each finding: `{file, content, sources}` (URLs extracted via `extract_urls`)
3. Build JSON:
   ```json
   {
     "session_id": "...",
     "generated_at": "RFC3339",
     "output_dir": "...",
     "total_tokens": N,
     "total_agents": N,
     "synthesis": { "markdown": "...", "sources": [...] },
     "findings": [{ "file": "...", "content": "...", "sources": [...] }]
   }
   ```
4. Write to `report.json`

### PDF Export (`export_pdf`)

**Algorithm**:
1. Write Markdown to a temporary `report.md`
2. **Priority 1**: If `pandoc` is available → run `pandoc report.md -o report.pdf --standalone`
3. If pandoc fails:
   - **Priority 2**: If `wkhtmltopdf` is available → export HTML, then `wkhtmltopdf report.html report.pdf`
4. If neither tool is found → error: "PDF export requires 'pandoc' or 'wkhtmltopdf'"

**Edge cases**: pandoc not installed → fallback to wkhtmltopdf. wkhtmltopdf also not installed → descriptive error.

### DOCX Export (`export_docx`)

**Algorithm**:
1. Check for pandoc; if not present → error
2. Write Markdown
3. Run `pandoc report.md -o report.docx`

### `build_report_markdown(session) -> String`

**Algorithm**:
1. Title: `# Research Report`
2. Metadata: Session, Generated, Agents, Tokens
3. Horizontal rule
4. Body: `session.synthesis`
5. Appendix: collect findings via `collect_findings`, build `## Appendix: Individual Findings` with `### filename` for each

### `collect_findings(output_dir) -> Vec<(String, String)>`

**Algorithm**:
1. Read the directory `output_dir/findings/`
2. Filter files with `.md` extension
3. Sort by name
4. Return `(filename, content)` for each

**Edge cases**: Directory does not exist → empty `Vec`. Files without `.md` are ignored.

### `extract_urls(text) -> Vec<String>`

**Algorithm**:
1. Find occurrences of `https://` and `http://`
2. For each: find the end of URL (space, `<`, `>`, `"`, `'`, `)`, `]`, `` ` ``)
3. Remove trailing `.` `,` `;` `:`
4. Deduplicate (do not add already existing URL)

### CSV Export

#### `CONTACT_EXPORT_COLUMNS`

```
["id", "name", "title", "company", "email", "phone", "tags", "social_profiles", "source", "created_at"]
```

#### `contacts_to_csv(contacts) -> String`

**Algorithm**:
1. Header: columns, wrapped via `csv_field`
2. For each contact:
   - `social_profiles` → `platform:url` or `platform:username`, joined via `"; "`
   - `tags` → joined via `"; "`
   - `created_at` → RFC 3339
3. Each value goes through `csv_field`
4. Lines separated by `\r\n` (RFC 4180)

#### `csv_field(value) -> String`

**RFC 4180 quoting**:
- If it contains `,`, `"`, `\n`, `\r` → wrap in `"..."`, all `"` inside replaced with `""`
- Otherwise → return as-is

### vCard Export

#### `contacts_to_vcard(contacts) -> String`

**Algorithm**: For each contact:
1. `BEGIN:VCARD\r\nVERSION:3.0\r\n`
2. `N:{family};{given};;;` (name split: first word = given, rest = family)
3. `FN:{name}`
4. `ORG:{company}` (if present)
5. `TITLE:{title}` (if present)
6. `EMAIL;TYPE=INTERNET:{email}` (if present)
7. `TEL:{phone}` (if present)
8. `X-SOCIALPROFILE;TYPE={platform}:{url}` for each social profile
9. `NOTE:{notes}` (notes joined with `\n`)
10. `CATEGORIES:{tags}` (comma-separated)
11. `END:VCARD\r\n`

All values go through `vcard_escape`.

#### `vcard_escape(value) -> String`

**RFC 2426 escaping**:
- `\` → `\\`
- `;` → `\;`
- `,` → `\,`
- `\r\n` → `\n`
- `\n` → `\n`

### XLSX Export

#### `contacts_to_xlsx(contacts) -> Result<Vec<u8>>`

**Algorithm**:
1. Create `rust_xlsxwriter::Workbook`
2. Add worksheet `"Contacts"`
3. Write headers (row 0)
4. For each contact (row N+1):
   - `id` → `write_number` (as f64)
   - Other fields → `write_string`
   - `social_profiles` → `platform:url/username`, via `"; "`
   - `tags` → via `"; "`
5. `workbook.save_to_buffer()` → return XLSX bytes

**Edge cases**: Empty list → file with headers. `id = None` → skipped.

### `pandoc_available() -> bool`

Checks for pandoc in PATH via `which pandoc` (Linux/macOS) or `where pandoc` (Windows).

---

## 15. notify.rs

### `NotificationChannel`

```rust
pub enum NotificationChannel {
    Webhook { url: String },
    Email {
        smtp_host: String,
        smtp_port: u16,
        from: String,
        to: String,
        username: String,
        password: String,
    },
    Telegram { bot_token: String, chat_id: String },
}
```

#### `impl Display`

- `Webhook` → `"webhook({url})"`
- `Email` → `"email({to}@{smtp_host})"`
- `Telegram` → `"telegram(chat {chat_id})"`

### `Notifier`

```rust
pub struct Notifier {
    channels: Vec<NotificationChannel>,
    http: reqwest::Client,
}
```

#### `Notifier::new(channels) -> Self`

Creates with a given list of channels. HTTP client via `http_client()`.

#### `Notifier::from_config(config) -> Self`

**Algorithm**:
1. If `webhook_url` is not empty → add `Webhook` channel
2. If `email_to` is not empty → add `Email` channel. `smtp_host` defaults to `"localhost"`, `email_from` defaults to `"fathom@localhost"`
3. If both `telegram_bot_token` and `telegram_chat_id` are not empty → add `Telegram` channel
4. Empty values → not added

**Edge cases**: Only `telegram_bot_token` without `chat_id` → Telegram is not added.

#### `Notifier::channels() -> &[NotificationChannel]`

Getter.

#### `Notifier::is_empty() -> bool`

No channels.

#### `Notifier::notify_completion(session) -> Result<()>`

**Algorithm**:
1. If no channels → `Ok(())`
2. For each channel:
   - `Webhook` → `send_webhook`
   - `Email` → `send_email`
   - `Telegram` → `send_telegram`
3. Errors are **collected**, they do not interrupt other channels
4. If there are errors → returns `anyhow::Error` with a list of failures

**Guarantee**: An error in one channel does not prevent delivery through others.

### `send_webhook(url, session)` (private)

**Algorithm**:
1. Build JSON payload:
   ```json
   {
     "event": "session.completed",
     "session_id": "...",
     "output_dir": "...",
     "total_tokens": N,
     "total_agents": N,
     "summary": "...",
     "synthesis_preview": "..."
   }
   ```
2. POST to URL
3. Check HTTP status → error if not success

### `send_email(smtp_host, smtp_port, from, to, username, password, session)` (private)

**Algorithm**:
1. Build email via `lettre::Message::builder`:
   - `From`, `To` parsed as `Mailbox`
   - Subject: `"Research session completed: {session_id}"`
   - Content-Type: `text/plain`
   - Body: `email_body(session)`
2. Configure transport:
   - If `username` is not empty → SMTP relay with credentials
   - If port is 25 or 1025 → `builder_dangerous` (plaintext, for local relays/MailHog)
   - Otherwise → SMTP relay without credentials (STARTTLS by default)
3. Send

### `send_telegram(bot_token, chat_id, session)` (private)

**Algorithm**:
1. URL: `https://api.telegram.org/bot{token}/sendMessage`
2. JSON: `{"chat_id": "...", "text": "..."}`
3. Check HTTP status and the `"ok": true` field in the response
4. Error → description from the `"description"` field of the Telegram response

### `email_body(session) -> String`

`summary_line() + "\n\nSynthesis preview:\n" + synthesis_preview(2000)`

### `telegram_text(session) -> String`

`"✅ " + summary_line() + "\n\n" + synthesis_preview(800) + "\n\nFull report: " + output_dir`

---

## 16. contact.rs

### `SocialProfile`

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialProfile {
    pub id: Option<i64>,
    pub platform: String,
    pub url: String,
    pub username: String,
}
```

Social network profile.

#### `SocialProfile::new(platform, url, username) -> Self`

Creates with `id = None`.

### `Company`

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Company {
    pub id: Option<i64>,
    pub name: String,
    pub website: Option<String>,
    pub industry: Option<String>,
    pub size: Option<String>,
    pub location: Option<String>,
    pub description: Option<String>,
}
```

Company record.

### `Contact`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub id: Option<i64>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub company: Option<String>,
    pub social_profiles: Vec<SocialProfile>,
    pub tags: Vec<String>,
    pub notes: Vec<String>,
    pub source: String,
    pub crm_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

Contact gathered during research.

| Field | Description |
|-------|-------------|
| `id` | DB ID (None before saving) |
| `crm_id` | CRM ID after push (for deduplication) |
| `source` | Source (search backend, tool) |

#### `Contact::new() -> Self`

Creates an empty contact: all `Option` = `None`, empty `Vec`, `source = "unknown"`, `created_at` and `updated_at = Utc::now()`.

#### `Contact::with_source(source) -> Self`

Builder-style method. Sets `source`.

#### `Contact::display_label() -> String`

**Fallback chain** (priority from highest to lowest):
1. `name` (if not empty after trim)
2. `email` (if not empty)
3. `phone` (if not empty)
4. `id` → `"contact #{id}"`
5. `"unnamed contact"`

**Interaction**: Used for displaying the contact in UI, logs, notifications.

#### `Contact::normalized_email() -> Option<String>`

Calls `normalize_email` on `self.email`. If the result is empty → `None`.

#### `Contact::normalized_phone() -> Option<String>`

Calls `normalize_phone` on `self.phone`. If the result is empty → `None`.

### `normalize_email(email) -> String`

```rust
pub fn normalize_email(email: &str) -> String
```

**Algorithm**: `email.trim().to_lowercase()`

**Interaction**: Used for contact deduplication: two contacts with the same normalized email are considered duplicates.

### `normalize_phone(phone) -> String`

```rust
pub fn normalize_phone(phone: &str) -> String
```

**Algorithm**: Keeps only ASCII digits (`0-9`). All other characters (spaces, `+`, `-`, `(`, `)`) are removed.

**Example**: `"+1 (555) 010-0100"` → `"15550100100"`, `"+7 999 123-45-67"` → `"79991234567"`, `"no digits"` → `""`.

**Interaction**: Used for deduplication: two contacts with the same digit set in the phone number are considered duplicates.

---

## 17. crm.rs

### Constants

```rust
pub const HUBSPOT_CONTACTS_URL: &str = "https://api.hubapi.com/crm/v3/objects/contacts";
```

### `CrmProvider`

```rust
pub enum CrmProvider {
    AmoCrm { domain: String, api_key: String },
    Bitrix24 { domain: String, api_key: String },
    HubSpot { api_key: String },
}
```

#### `CrmProvider::name() -> &'static str`

`AmoCrm` → `"amocrm"`, `Bitrix24` → `"bitrix24"`, `HubSpot` → `"hubspot"`.

#### `CrmProvider::parse(provider, domain, api_key) -> Option<CrmProvider>`

**Algorithm**:
1. Converts `provider` to lowercase
2. `"amocrm"` / `"amo"` → `AmoCrm` (requires `domain` and `api_key`)
3. `"bitrix24"` / `"bitrix"` → `Bitrix24` (requires `domain` and `api_key`)
4. `"hubspot"` → `HubSpot` (requires `api_key`)
5. Unknown provider or empty required fields → `None`

#### `impl Display`

Returns `name()`.

### `CrmSync`

```rust
pub struct CrmSync {
    provider: CrmProvider,
    http: reqwest::Client,
    endpoint_override: Option<String>,
}
```

#### `CrmSync::new(provider) -> Self`

HTTP client via `http_client()`.

#### `CrmSync::from_config(config) -> Option<Self>`

Creates from `CrmConfig`. `None` if the provider is not configured.

#### `CrmSync::with_endpoint(endpoint) -> Self`

URL override (for testing with a mock server).

#### `CrmSync::provider() -> &CrmProvider`

Getter.

#### `CrmSync::push_contact(contact) -> Result<String>`

**Algorithm**: Delegates to `push_amocrm`, `push_bitrix24`, or `push_hubspot` depending on the provider. Returns the **remote contact id**.

### amoCRM API

#### `push_amocrm(contact)` (private)

**Algorithm**:
1. URL: `https://{subdomain}.amocrm.ru/api/v4/contacts`
2. Authorization: `Bearer {api_key}` (header)
3. Payload: JSON array with one contact object (via `amocrm_payload`)
4. POST request
5. Extract ID from response: `/_embedded/contacts/0/id` or `id`
6. Error → `api_error("amocrm", status, body)`

#### `amocrm_contacts_url(domain) -> String`

**Algorithm**: `strip_scheme(domain)`, `trim_end_matches('/')`, `strip_suffix(".amocrm.ru")` → builds `https://{subdomain}.amocrm.ru/api/v4/contacts`.

#### `amocrm_payload(contact) -> serde_json::Value`

**Algorithm**:
1. Build an array with one object
2. `name`: if `contact.name` exists → use it; otherwise `email`; otherwise `phone`; otherwise `"Unknown contact"`
3. `custom_fields_values`: array with:
   - `PHONE` (field_code) if phone exists
   - `EMAIL` (field_code) if email exists
   - Each value: `{ "value": "..." }`

### Bitrix24 API

#### `push_bitrix24(contact)` (private)

**Algorithm**:
1. URL: `https://{host}/rest/crm.contact.add.json`
2. Authorization: query parameter `?auth={api_key}`
3. Payload: `{ "fields": ... }` via `bitrix24_fields`
4. Check HTTP status and presence of `"error"` in response
5. Extract ID from `"result"`

#### `bitrix24_contact_add_url(domain) -> String`

**Algorithm**: If `domain` contains a dot → use as-is (full host). Otherwise → `{domain}.bitrix24.ru`.

#### `bitrix24_fields(contact) -> serde_json::Value`

**Field format**:

| Contact Field | Bitrix24 Field | Format |
|--------------|---------------|--------|
| `name` | `NAME` | string |
| `title` | `POST` | string |
| `company` | `COMPANY_TITLE` | string |
| `email` | `EMAIL` | `[{ "VALUE": "...", "VALUE_TYPE": "WORK" }]` |
| `phone` | `PHONE` | `[{ "VALUE": "..." }]` |
| `notes` | `COMMENTS` | string (notes joined with `\n`) |

### HubSpot API

#### `push_hubspot(contact)` (private)

**Algorithm**:
1. URL: `https://api.hubapi.com/crm/v3/objects/contacts` (constant `HUBSPOT_CONTACTS_URL`)
2. Authorization: `Bearer {api_key}`
3. Payload: `{ "properties": ... }` via `hubspot_properties`
4. Extract ID from `"id"`

#### `hubspot_properties(contact) -> serde_json::Value`

**Property format**:

| Contact Field | HubSpot Property |
|--------------|-----------------|
| `email` | `email` |
| `phone` | `phone` |
| `name` | `firstname` + `lastname` (split: first word = firstname, rest = lastname) |
| `title` | `jobtitle` |
| `company` | `company` |

**Edge cases**: Single-word name → only `firstname`, `lastname` is absent.

### Helper Functions

#### `strip_scheme(s) -> &str` (private)

Removes `https://` or `http://` from the beginning of the string.

#### `split_name(name) -> (String, Option<String>)` (private)

First word → first name. Rest → last name (optional).

#### `id_to_string(v) -> String` (private)

Converts `serde_json::Value` (Number/String) to string.

#### `api_error(provider, status, body) -> anyhow::Error` (private)

**Algorithm**:
1. Attempts to extract an error message via `extract_api_error_message`
2. If unsuccessful → serializes body to string (truncated to 300 characters at char boundary)
3. Builds: `"{provider} API error (HTTP {status}): {detail}"`

#### `extract_api_error_message(body) -> Option<String>` (private)

**Algorithm**: Checks fields in priority order: `"detail"` → `"message"` → `"error_description"` → `"title"` → `"error"`. First non-empty string value.

**Deduplication via `crm_id`**: The `Contact::crm_id` field stores the contact ID in the CRM after the first push. Before a repeated push, the system checks whether `crm_id` is populated — if so, the contact is updated instead of creating a new one. This prevents duplicate contacts in the CRM during repeated research sessions.

#### Tests

The module contains integration tests with a mock server (`wiremock`), covering:
- Successful contact push to each of the three providers (amoCRM, Bitrix24, HubSpot)
- API errors with verification of informative error messages
- Correct URL and payload formation for each provider
- Handling of empty contact fields
- Validation of `CrmProvider::parse` with various parameter combinations

---

## Cross-Dependencies

Below describes how modules within `pr-core` interact with each other and with external crates.

### Internal Dependencies

| Module | Uses |
|--------|------|
| `agent.rs` | `ids.rs` (`AgentId`) |
| `event.rs` | `ids.rs` (`SessionId`, `AgentId`), `agent.rs` (`AgentRole`, `AgentState`), `finding.rs` (`Finding`) |
| `finding.rs` | `ids.rs` (`FindingId`, `AgentId`) |
| `session.rs` | `ids.rs` (`SessionId`) |
| `export.rs` | `session.rs` (`SessionOutput`), `config.rs` (`ExportFormat`) |
| `notify.rs` | `session.rs` (`SessionOutput`), `config.rs` (`NotificationsConfig`), `lib.rs` (`http_client`) |
| `crm.rs` | `contact.rs` (`Contact`), `config.rs` (`CrmConfig`), `lib.rs` (`http_client`) |
| `memory.rs` | standalone (only `serde`, `chrono`) |
| `token.rs` | `message.rs` (`Message`, `ToolCall`), `tool.rs` (`ToolSchema`) |
| `skill.rs` | standalone |
| `config.rs` | `export.rs` (`ExportFormat`) |

### External Crate Consumers

| Crate | Uses from `pr-core` |
|-------|---------------------|
| `pr-llm` | `Message`, `ToolCall`, `ToolSchema`, `PrError`, `estimate_*` |
| `pr-agent` | `AgentRole`, `AgentState`, `AgentRecord`, `AgentEvent`, `Finding`, `ToolOutput`, `Message` |
| `pr-persistence` | `AgentRecord`, `SessionOutput`, `Contact`, `SocialProfile`, `Company` |
| `pr-search` | `ToolOutput`, `ToolSchema` |
| `pr-tools` | `Contact`, `SocialProfile`, `ToolOutput` |
| `pr-cli` | `AppConfig`, `set_config_value`, `ExportFormat`, `Exporter`, `Notifier`, `SkillRegistry`, `MemoryStore` |