# Crate `server` — detailed documentation

## Overview

The `pr-server` crate provides an HTTP JSON API for managing research sessions, background jobs, memory, governance, computer-use relay, and operational observability. Built on the **axum** framework using **tokio** for asynchronous execution. Includes API-key authentication, per-client rate limiting, Prometheus-format metrics, SSE (Server-Sent Events) for live event streaming, an AG-UI compatibility bridge, and an embedded single-file dashboard.

---

## File structure

| File | Purpose |
|------|---------|
| `lib.rs` | Routing, handlers, server startup, SSE, AppState, middleware |
| `auth.rs` | API-key authentication, rate limiter |
| `metrics.rs` | Atomic Counter/Gauge/Histogram, Prometheus render |
| `computers_api.rs` | Computer-use relay routes (loopback HTTP + WebSocket) |
| `jobs_api.rs` | Durable background job CRUD |
| `memory_api.rs` | Long-term semantic memory CRUD and pipeline endpoints |
| `coworkers_api.rs` | Desktop-agent coworkers and channels REST API |
| `governance_api.rs` | Governance policy CRUD, decision endpoint, audit log |
| `replay_api.rs` | Replay timeline of recorded governed actions |
| `supervisor_api.rs` | Optional Docker computer supervisor endpoints |
| `schedules_api.rs` | Cron-based schedule CRUD and due-claim tick |
| `credentials_api.rs` | Encrypted credential vault CRUD |
| `notifications_api.rs` | Operator-triggered notification delivery test |
| `observability.rs` | Bounded operational summary (live metrics + audit counts) |
| `agui.rs` | AG-UI SSE compatibility bridge |

---

## 1. `lib.rs` — main server module

### 1.1 Application state — `AppState`

```rust
pub struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) db: Arc<Persistence>,
    pub(crate) llm: Arc<dyn LlmProvider>,
    pub(crate) tools: Arc<ToolRegistry>,
    pub(crate) metrics: Arc<Metrics>,
    pub(crate) auth: ApiKeyAuth,
    pub(crate) rate_limiter: Mutex<RateLimiter>,
    pub(crate) active_sessions: Mutex<HashMap<String, RunningSession>>,
    pub(crate) event_tx: broadcast::Sender<AgentEvent>,
    pub(crate) jobs: Arc<JobsDb>,
    pub(crate) jobs_root: PathBuf,
    pub(crate) job_spawner: JobSpawner,
    pub(crate) memory: Option<Arc<pr_memory::Memory>>,
    pub(crate) pending_controls: Arc<Mutex<HashMap<String, PendingControl>>>,
    pub(crate) governance: Arc<tokio::sync::RwLock<Governance>>,
    pub(crate) governance_enabled: bool,
    pub(crate) supervisor: Option<Arc<pr_supervisor::ComputerSupervisor>>,
}
```

| Field | Type | Purpose |
|-------|------|---------|
| `config` | `AppConfig` | Full application configuration |
| `db` | `Arc<Persistence>` | SQLite persistence layer |
| `llm` | `Arc<dyn LlmProvider>` | LLM provider (DeepSeek, OpenAI-compatible, etc.) |
| `tools` | `Arc<ToolRegistry>` | Built-in tool registry |
| `metrics` | `Arc<Metrics>` | Prometheus-style atomic metrics |
| `auth` | `ApiKeyAuth` | API key registry (from `FATHOM_API_KEYS` env) |
| `rate_limiter` | `Mutex<RateLimiter>` | Per-client sliding-window rate limiter |
| `active_sessions` | `Mutex<HashMap<String, RunningSession>>` | Live session handles |
| `event_tx` | `broadcast::Sender<AgentEvent>` | SSE event bus (capacity 1024) |
| `jobs` | `Arc<JobsDb>` | Durable background job registry (SQLite or in-memory) |
| `jobs_root` | `PathBuf` | Root directory for job output |
| `job_spawner` | `JobSpawner` | Injectable function `(job_id, log_path) -> pid` |
| `memory` | `Option<Arc<pr_memory::Memory>>` | Long-term semantic memory (None when disabled/failed) |
| `pending_controls` | `Arc<Mutex<HashMap<String, PendingControl>>>` | Operator round-trips (questions/approvals) |
| `governance` | `Arc<tokio::sync::RwLock<Governance>>` | Runtime-shared policy engine |
| `governance_enabled` | `bool` | Whether governance enforcement is active |
| `supervisor` | `Option<Arc<ComputerSupervisor>>` | Docker computer supervisor |

#### `PendingControl` enum

```rust
pub(crate) enum PendingControl {
    Question {
        session_id: String,
        reply: tokio::sync::oneshot::Sender<String>,
    },
    Approval {
        session_id: String,
        reply: tokio::sync::oneshot::Sender<bool>,
    },
}
```

#### `JobSpawner` type alias

```rust
pub(crate) type JobSpawner =
    Arc<dyn Fn(&str, &std::path::Path) -> std::io::Result<u32> + Send + Sync>;
```

### 1.2 Constructors

#### `AppState::new(config)`

1. Creates the output directory (`create_dir_all`)
2. Opens the SQLite database: `Persistence::open(&output_dir.join(".research.db"))`
3. Opens the jobs database: `JobsDb::open(default_jobs_db_path())` — falls back to in-memory on failure
4. Delegates to `with_db_and_jobs`

#### `AppState::with_db(config, db)`

Builds state around an already-open database (used by tests). Jobs get a private in-memory registry. Delegates to `with_db_and_jobs`.

#### `AppState::with_db_and_jobs(config, db, jobs)`

1. Builds the LLM provider: `pr_llm::build_provider(&config.llm)`, fallback to `DeepSeekProvider`
2. Initializes `ToolRegistry::with_builtins()`
3. Creates `Metrics::new()`
4. Loads `ApiKeyAuth::from_env()`
5. Creates `RateLimiter` with limit from env `FATHOM_RATE_LIMIT` (default 120 req/min), 60-second window
6. Creates a broadcast channel for events (capacity 1024)
7. Opens long-term memory if `config.memory.enabled` (best-effort, logs warning on failure)
8. Loads governance from env: `FATHOM_GOVERNANCE_ENABLED` (bool, default false), `FATHOM_GOVERNANCE_POLICY` (JSON PolicyConfig)
9. Initializes the computer supervisor from env via `ComputerSupervisor::from_env()` (absent when `COMPUTER_TOKEN` not set)
10. Returns `Arc<AppState>`

### 1.3 Route registration — `build_router(state)`

Builds the axum `Router` with two layers:

#### API routes (behind auth + rate limiting, nested under `/api/v1`):

**Sessions (9 routes):**
```
POST   /api/v1/sessions             → create_session
GET    /api/v1/sessions             → list_sessions
GET    /api/v1/sessions/:id         → get_session_status
DELETE /api/v1/sessions/:id         → cancel_session
POST   /api/v1/sessions/:id/steer   → steer_session
POST   /api/v1/sessions/:id/answer  → answer_question
POST   /api/v1/sessions/:id/approve → approve_tool
GET    /api/v1/sessions/:id/results → get_session_results
GET    /api/v1/sessions/:id/events  → session_events (SSE)
```

**Agents (2 routes):**
```
GET    /api/v1/agents               → list_agents
GET    /api/v1/agents/:id           → get_agent_status
```

**Events (1 route):**
```
GET    /api/v1/events               → global_events (SSE)
```

**Memory (6 routes):**
```
GET    /api/v1/memories             → list_memories
POST   /api/v1/memories/absorb      → absorb_memories
GET    /api/v1/memories/stats       → memory_stats
POST   /api/v1/memories/distill     → distill_memories
POST   /api/v1/memories/gc          → gc_memories
GET    /api/v1/memories/:id         → get_memory
DELETE /api/v1/memories/:id         → archive_memory
```

**Jobs (6 routes):**
```
POST   /api/v1/jobs                 → create_job
GET    /api/v1/jobs                 → list_jobs
GET    /api/v1/jobs/:id             → get_job
GET    /api/v1/jobs/:id/log         → get_job_log
DELETE /api/v1/jobs/:id             → cancel_job
POST   /api/v1/jobs/:id/rerun       → rerun_job
```

**Credentials (3 routes):**
```
GET    /api/v1/credentials          → list
POST   /api/v1/credentials          → store
DELETE /api/v1/credentials/:id      → delete
```

**Coworkers (5 routes):**
```
GET    /api/v1/coworkers            → list_coworkers
POST   /api/v1/coworkers            → create_coworker
GET    /api/v1/coworkers/:id        → get_coworker
PUT    /api/v1/coworkers/:id        → update_coworker
PATCH  /api/v1/coworkers/:id        → update_coworker
DELETE /api/v1/coworkers/:id        → delete_coworker
```

**Channels (4 routes):**
```
GET    /api/v1/channels             → list_channels
POST   /api/v1/channels             → create_channel
PUT    /api/v1/channels/:id         → update_channel
PATCH  /api/v1/channels/:id         → update_channel
DELETE /api/v1/channels/:id         → delete_channel
```

**Schedules (7 routes):**
```
POST   /api/v1/schedules            → create_schedule
GET    /api/v1/schedules            → list_schedules
GET    /api/v1/schedules/:id        → get_schedule
PUT    /api/v1/schedules/:id        → update_schedule
PATCH  /api/v1/schedules/:id        → update_schedule
DELETE /api/v1/schedules/:id        → delete_schedule
POST   /api/v1/schedules/claim      → claim_schedules
```

**Governance (4 routes):**
```
GET    /api/v1/governance/policy    → get_policy
PUT    /api/v1/governance/policy    → put_policy
POST   /api/v1/governance/decide    → decide
GET    /api/v1/governance/audit     → audit
```

**Replay (1 route):**
```
GET    /api/v1/replay               → list_replay
```

**Observability (1 route):**
```
GET    /api/v1/observability/summary → summary
```

**Notifications (1 route):**
```
POST   /api/v1/notifications/test   → test
```

**AG-UI (2 routes):**
```
GET    /api/v1/ag-ui/events         → agui::events (SSE)
GET    /api/v1/ag-ui/health         → agui::health
```

**Computer relay — unscoped (16 routes):**
```
POST   /api/v1/computers/session              → start_session
GET    /api/v1/computers/health               → health_default
GET    /api/v1/computers/screenshot           → screenshot_default
GET    /api/v1/computers/snapshot             → snapshot_default
GET    /api/v1/computers/tabs                 → tabs
POST   /api/v1/computers/tabs/open            → tabs_open
POST   /api/v1/computers/tabs/:tab_id/activate → tab_activate
POST   /api/v1/computers/tabs/:tab_id/close   → tab_close
POST   /api/v1/computers/control/take         → take_control
POST   /api/v1/computers/control/release      → release_control
POST   /api/v1/computers/navigate             → navigate
POST   /api/v1/computers/click                → click
POST   /api/v1/computers/type                 → type_text
POST   /api/v1/computers/secret               → secret
POST   /api/v1/computers/key                  → key
GET    /api/v1/computers/files                → files
DELETE /api/v1/computers/files                → files_delete
GET    /api/v1/computers/files/read           → files_read
PUT    /api/v1/computers/files/write          → files_write
```

**Computer relay — agent-scoped (14 routes):**
```
GET    /api/v1/computers/:agent_id/health           → health_for_agent
GET    /api/v1/computers/:agent_id/screenshot       → screenshot
GET    /api/v1/computers/:agent_id/snapshot         → snapshot_for_agent
GET    /api/v1/computers/:agent_id/screen           → screen (WebSocket)
POST   /api/v1/computers/:agent_id/control/take     → take_control_for_agent
POST   /api/v1/computers/:agent_id/control/release  → release_control_for_agent
POST   /api/v1/computers/:agent_id/navigate         → navigate_for_agent
POST   /api/v1/computers/:agent_id/click            → click_for_agent
POST   /api/v1/computers/:agent_id/type             → type_for_agent
POST   /api/v1/computers/:agent_id/key              → key_for_agent
GET    /api/v1/computers/:agent_id/files            → files_for_agent
DELETE /api/v1/computers/:agent_id/files            → files_delete_for_agent
GET    /api/v1/computers/:agent_id/files/read       → files_read_for_agent
PUT    /api/v1/computers/:agent_id/files/write      → files_write_for_agent
```

**Computer supervisor (4 routes):**
```
GET    /api/v1/computers                             → supervisor_api::list
POST   /api/v1/computers/:agent_id/ensure            → supervisor_api::ensure
POST   /api/v1/computers/:agent_id/stop              → supervisor_api::stop
POST   /api/v1/computers/:agent_id/reset             → supervisor_api::reset
```

#### Public routes (no authentication):
```
GET    /health      → health_check
GET    /metrics     → metrics_endpoint
GET    /dashboard   → dashboard_page (embedded single-file HTML)
```

#### Middleware order (outermost to innermost):

1. **CorsLayer** — loopback origins only; allows standard methods and auth headers
2. **TraceLayer** — HTTP request/response tracing
3. **metrics_middleware** — records request duration and total request count
4. **`/api/v1` only:**
   - **auth_middleware** — API-key validation (401 on invalid key)
   - **rate_limit_middleware** — per-client sliding-window rate limiting (429 on exhaustion)

### 1.4 Server startup — `run_server(host, port)`

1. Loads configuration `AppConfig::load()`
2. Warns if LLM API key is not set (sessions will return 503)
3. **Security check**: if `host` is not loopback and API keys are not configured — refuses to start
4. Creates `AppState::new(config)`
5. Builds the router via `build_router`
6. Binds to `SocketAddr::from((ip, port))`
7. Starts `axum::serve` with `into_make_service_with_connect_info::<SocketAddr>()`

### 1.5 Middleware — `metrics_middleware`

Records request duration and increments `requests_total`.

### 1.6 Helper functions

- `json(status, value) -> Response` — JSON response with HTTP status
- `error(status, message) -> Response` — `{"error": "message"}`
- `dashboard_cors() -> CorsLayer` — loopback origins only
- `SessionResponse` — session serialization with `active` flag

### 1.7 Handlers

#### `POST /api/v1/sessions` — `create_session`

Validates query, checks LLM key (503 if missing), generates SessionId, validates output_dir (path traversal protection), creates directory, saves to DB, increments metrics, spawns background task, returns 202 Accepted.

#### `spawn_session()` — background task with RAII cleanup

Runs `Coordinator::execute()` and a metrics_loop in parallel via `tokio::join!`. RAII guard (`SessionCleanup`) ensures `sessions_active` gauge is decremented exactly once and record is removed from `active_sessions` even on panic.

#### Session lifecycle handlers:

- `GET /sessions` — list all with active flag
- `GET /sessions/:id` — status with active flag
- `DELETE /sessions/:id` — cancel (CancellationToken + abort + DB update)
- `POST /sessions/:id/steer` — mid-run instruction injection
- `POST /sessions/:id/answer` — answer pending question tool
- `POST /sessions/:id/approve` — allow/deny pending side-effect tool
- `GET /sessions/:id/results` — summary.md + findings/

#### Agent handlers:

- `GET /agents` — list all across sessions
- `GET /agents/:id` — single agent detail

#### Health and metrics:

- `GET /health` — DB check + active session count (200 ok / 503 degraded)
- `GET /metrics` — Prometheus text exposition
- `GET /dashboard` — embedded HTML dashboard

### 1.8 SSE (Server-Sent Events)

#### `GET /api/v1/events` — `global_events`

Subscribes to broadcast channel without filtering.

#### `GET /api/v1/sessions/:id/events` — `session_events`

Loads initial agent set from DB, creates filtered stream with positive/negative caches.

#### `event_belongs_to_session()` — caching algorithm

1. Session-scoped events match by id
2. Agent-scoped events check positive cache → negative cache → DB lookup
3. Negative cache prevents repeated DB queries for foreign agents

#### `serialize_sse_event(event)` — redaction

Preserves user-facing fields (question text, approval details). Recursively redacts secret-like keys and tool/LLM payloads.

### 1.9 `RunningSession` structure

```rust
struct RunningSession {
    handle: JoinHandle<()>,
    finished: Arc<AtomicBool>,
    steer_tx: tokio::sync::mpsc::UnboundedSender<String>,
    cancel: tokio_util::sync::CancellationToken,
}
```

### 1.10 Constants

```rust
pub const RATE_LIMIT_ENV: &str = "FATHOM_RATE_LIMIT";
pub const DEFAULT_RATE_LIMIT: usize = 120; // requests per minute
```

---

## 2. `auth.rs` — authentication and rate limiting

### 2.1 `ApiKeyAuth` and `ApiKeyInfo`

Registry of valid API keys. When no keys are registered, authentication is disabled (open access). Loads from `FATHOM_API_KEYS` env (comma-separated).

### 2.2 `extract_api_key(headers)`

Supports `Authorization: Bearer <key>` and `X-Api-Key: <key>`.

### 2.3 `AuthPrincipal(pub String)`

Inserted into request extensions after successful authentication. Used as rate-limiting key.

### 2.4 `auth_middleware`

Returns 401 when keys are configured and request lacks a valid key. Inserts `AuthPrincipal` on success.

### 2.5 `RateLimiter`

Sliding-window rate limiter keyed by client identity. Returns 429 when window is exhausted.

### 2.6 `rate_limit_middleware`

Keys off `AuthPrincipal` → client IP → `"anonymous"`. Runs after `auth_middleware`.

---

## 3. `metrics.rs` — Prometheus metrics

### 3.1 Primitives

- **Counter** — `AtomicU64`, monotonic increment
- **Gauge** — `AtomicI64`, can go up and down
- **Histogram** — fixed buckets with cumulative counts, atomic sum via f64 bit representation

### 3.2 `Metrics` registry

```rust
pub struct Metrics {
    pub sessions_total: Counter,
    pub sessions_active: Gauge,
    pub agents_spawned: Counter,
    pub tokens_used: Counter,
    pub tool_calls: Counter,
    pub requests_total: Counter,
    pub request_duration: Histogram,
}
```

Default duration buckets: `[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]` (5ms to 10s, 11 buckets).

`render_metrics()` generates full Prometheus text exposition format.

---

## 4. `computers_api.rs` — computer-use relay

Proxies requests to an upstream loopback HTTP computer service (Playwright-backed). Default URL: `http://127.0.0.1:8765` (`FATHOM_COMPUTER_SERVICE_URL`).

### Key features:

- **Screenshot**: base64 image data (20 MB max); WebSocket `/screen` emits binary frames at 500ms interval
- **Input relay**: click, type, key, secret (redacted from logs), navigate
- **Tab management**: list, open, activate, close
- **File operations**: list, read, write, delete (1.1 MB body max)
- **Browser control**: take/release control
- **Health and snapshot**: page metadata

### Agent-scoped routing:

Resolves supervisor port per agent via `agent_service_root()`. Otherwise uses default service root.

### Timeouts:

- HTTP upstream: 5s
- WebSocket send: 2s
- Screenshot polling: 500ms

---

## 5. `jobs_api.rs` — durable background jobs

Jobs execute in fully detached runner processes (`job-run`), surviving server restarts. Registry shared with CLI (`fathom jobs list`).

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/jobs` | Submit job (query, attempts default 3) |
| GET | `/api/v1/jobs` | List all jobs |
| GET | `/api/v1/jobs/:id` | Get status (full id or unique prefix) |
| GET | `/api/v1/jobs/:id/log?lines=N` | Tail log (default 100 lines) |
| DELETE | `/api/v1/jobs/:id` | Cancel active job |
| POST | `/api/v1/jobs/:id/rerun` | Re-run finished/stale job |

---

## 6. `memory_api.rs` — long-term semantic memory

Routes nested under `/api/v1` behind auth + rate limiting. Returns 503 when `[memory] enabled = false` or store failed to open.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/memories?q=...` | Search memories |
| GET | `/memories` | List (scope/status/limit filters) |
| POST | `/memories/absorb` | Absorb facts through full pipeline |
| GET | `/memories/stats` | Store statistics |
| POST | `/memories/distill?session=&dry_run=` | Promote run facts to agent knowledge |
| POST | `/memories/gc?ttl_days=&dry_run=` | Archive expired facts, compact groups |
| GET | `/memories/:id?follow=latest` | One memory with follow chain |
| DELETE | `/memories/:id` | Archive (soft delete) |

---

## 7. `coworkers_api.rs` — coworkers and channels

REST API for desktop-agent coworkers and their communication channels.

### Input validation:

- IDs: ASCII alphanumeric + `-_.`, max 128 chars
- Name: max 100 chars; Title: max 200 chars; Role: max 100 chars
- Prompt: max 20,000 chars; Visibility: max 32 chars (default `"private"`)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/coworkers` | List all coworkers |
| POST | `/coworkers` | Create coworker |
| GET | `/coworkers/:id` | Fetch one coworker |
| PUT/PATCH | `/coworkers/:id` | Update coworker |
| DELETE | `/coworkers/:id` | Delete coworker |
| GET | `/channels?coworker_id=` | List channels |
| POST | `/channels` | Create channel |
| PUT/PATCH | `/channels/:id` | Update channel |
| DELETE | `/channels/:id` | Delete channel |

---

## 8. `governance_api.rs` — governance policy and audit

| Method | Path | Description |
|--------|------|-------------|
| GET | `/governance/policy` | Get current policy (enabled flag + PolicyConfig) |
| PUT | `/governance/policy` | Replace policy (max 1000 rules) |
| POST | `/governance/decide` | Evaluate an ActionContext against the policy engine |
| GET | `/governance/audit?limit=&decision=&agent=&session=` | Query audit log (max 200 rows, default 50) |

Each decision is recorded as an `AuditEventRow` in the database.

---

## 9. `replay_api.rs` — replay timeline

| Method | Path | Description |
|--------|------|-------------|
| GET | `/replay?session=&agent=&limit=` | Newest recorded governed actions first |

Filter values max 256 bytes. Limit capped at `MAX_REPLAY_LIMIT`.

---

## 10. `supervisor_api.rs` — Docker computer supervisor

Optional endpoints; return 503 when supervisor is not configured.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/computers` | List all containers |
| POST | `/computers/:agent_id/ensure` | Start/ensure container for agent |
| POST | `/computers/:agent_id/stop` | Stop agent container |
| POST | `/computers/:agent_id/reset` | Reset agent container |

---

## 11. `schedules_api.rs` — cron schedules

### Input validation:

- IDs: ASCII alphanumeric + `-_.`, max 128 chars
- Cron expression: max 256 chars
- Timezone: max 128 chars (default `"UTC"`)
- Query: max 20,000 chars

| Method | Path | Description |
|--------|------|-------------|
| POST | `/schedules` | Create schedule |
| GET | `/schedules` | List all schedules |
| GET | `/schedules/:id` | Get one schedule |
| PUT/PATCH | `/schedules/:id` | Update schedule |
| DELETE | `/schedules/:id` | Delete schedule |
| POST | `/schedules/claim?limit=` | Claim due rows for scheduler tick (default 25, max 100) |

---

## 12. `credentials_api.rs` — credential vault

| Method | Path | Description |
|--------|------|-------------|
| GET | `/credentials` | List all credentials (id, name, kind, timestamps — no secrets) |
| POST | `/credentials` | Store a credential (name, kind, secret) |
| DELETE | `/credentials/:id` | Delete a credential (204 No Content) |

---

## 13. `notifications_api.rs` — notification delivery

| Method | Path | Description |
|--------|------|-------------|
| POST | `/notifications/test` | Send test notification through one configured channel |

Accepts `{"channel": "webhook" | "email" | "telegram"}`. Uses the already-configured settings from `[notifications]` in config. 10-second delivery timeout. Does not accept destination details or message content from the caller.

---

## 14. `observability.rs` — operational summary

| Method | Path | Description |
|--------|------|-------------|
| GET | `/observability/summary` | Live process metrics + bounded audit counts |

Returns `ObservabilitySummary`:

```rust
pub struct ObservabilitySummary {
    pub active_sessions: usize,
    pub sessions_total: u64,
    pub agents_spawned: u64,
    pub tool_calls: u64,
    pub tokens_used: u64,
    pub audit_events: usize,
    pub audit_denials: usize,
    pub audit_counts_truncated: bool,
}
```

Audit rows loaded with a hard limit (10,000) to prevent unbounded reads.

---

## 15. `agui.rs` — AG-UI compatibility bridge

Narrow transport adapter exposing the existing agent event bus as SSE envelopes. Does not implement the complete AG-UI command/state protocol. Event payloads are redacted at the boundary.

### `AgUiEvent` envelope:

```rust
pub struct AgUiEvent {
    pub protocol: &'static str,    // "fathom.ag-ui"
    pub version: &'static str,     // "1"
    pub event_type: &'static str,  // lifecycle name
    pub event_id: Option<String>,
    pub timestamp_ms: u128,
    pub data: Value,               // Fathom event shape, redacted
}
```

### Event type mapping:

| Fathom Event | AG-UI Event Type |
|--------------|-----------------|
| SessionStarted | RUN_STARTED |
| SessionCompleted | RUN_FINISHED |
| SessionFailed | RUN_ERROR |
| AgentSpawned | STEP_STARTED |
| AgentCompleted | STEP_FINISHED |
| AgentFailed | STEP_ERROR |
| LlmStreamChunk | TEXT_MESSAGE_CONTENT |
| ToolCallStarted | TOOL_CALL_START |
| ToolCallCompleted | TOOL_CALL_END |
| Finding, AgentStateChanged | STATE_DELTA |
| QuestionAsked, ApprovalRequested | INTERRUPT |
| SessionForked | RUN_STARTED |
| FileChangeUndone, TitleGenerated | STATE_DELTA |

### Routes:

| Method | Path | Description |
|--------|------|-------------|
| GET | `/ag-ui/events?cursor=` | SSE stream with ring buffer (256 events) |
| GET | `/ag-ui/health` | Bridge capabilities report |

### Security:

Recursive redaction of credential-bearing fields (`api_key`, `password`, `secret`, `token`, etc.) via `redact_value()`.

### Event store:

Static `EventStore` with a `VecDeque<RecordedEvent>` ring buffer (capacity 256). Supports cursor-based replay for late-connecting clients.