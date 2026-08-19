# Crate `server` — detailed documentation

## Overview

The `pr-server` crate provides an HTTP JSON API for managing research sessions. Built on the **axum** framework using **tokio** for asynchronous execution. Includes API-key authentication, rate limiting, Prometheus-format metrics, and SSE (Server-Sent Events) for event streaming.

---

## File structure

| File | Purpose |
|------|---------|
| `lib.rs` | Routing, handlers, server startup, SSE |
| `auth.rs` | API-key authentication, rate limiter |
| `metrics.rs` | Atomic Counter/Gauge/Histogram, Prometheus render |

---

## 1. `lib.rs` — main server module

### 1.1 Route registration — `build_router(state)`

The function builds a `Router` from axum with two layers:

#### API routes (behind authentication and rate limiting):
```
POST   /api/v1/sessions             → create_session
GET    /api/v1/sessions             → list_sessions
GET    /api/v1/sessions/:id         → get_session_status
DELETE /api/v1/sessions/:id         → cancel_session
POST   /api/v1/sessions/:id/steer   → steer_session
GET    /api/v1/sessions/:id/results → get_session_results
GET    /api/v1/sessions/:id/events  → session_events (SSE)
GET    /api/v1/events               → global_events (SSE)
GET    /api/v1/agents               → list_agents
GET    /api/v1/agents/:id           → get_agent_status
```

#### Public routes (no authentication):
```
GET    /health   → health_check
GET    /metrics  → metrics_endpoint
```

#### Middleware order (outermost to innermost):
1. **CorsLayer** — permissive when auth is enabled, restrictive without it
2. **TraceLayer** — HTTP tracing
3. **metrics_middleware** — collect request duration metrics
4. **auth_middleware** — API-key validation
5. **rate_limit_middleware** — request rate limiting

Important note: CORS is permissive **only** when API keys are configured. Without authentication (local dev), CORS is blocked — protection against malicious web pages controlling agents.

### 1.2 Application state — `AppState`

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
}
```

#### `AppState::new(config)` — constructor from config
1. Creates the output directory (`create_dir_all`)
2. Opens the SQLite database: `Persistence::open(&output_dir.join(".research.db"))`
3. Delegates to `with_db`

#### `AppState::with_db(config, db)` — constructor with pre-existing DB
1. Builds the LLM provider: `pr_llm::build_provider(&config.llm)`, fallback to `DeepSeekProvider`
2. Initializes `ToolRegistry::with_builtins()`
3. Creates `Metrics::new()`
4. Loads `ApiKeyAuth::from_env()`
5. Creates `RateLimiter` with limit from env `PARALLEL_RESEARCH_RATE_LIMIT` (default 120 req/min)
6. Creates a broadcast channel for events (capacity 1024)

#### `is_active(session_id) -> bool`
Checks whether session_id exists in the `active_sessions` HashMap.

### 1.3 Server startup — `run_server(host, port)`

1. Loads configuration `AppConfig::load()`
2. Warns if LLM API key is not set (sessions will return 503)
3. **Security check**: if `host` is not loopback and API keys are not configured — refuses to start (`bail!`). This prevents exposing agents to the network without authentication.
4. Creates `AppState::new(config)`
5. Builds the router via `build_router`
6. Binds to `SocketAddr::from((ip, port))`
7. Starts `axum::serve` with `into_make_service_with_connect_info::<SocketAddr>()`

### 1.4 Middleware — `metrics_middleware`

```rust
pub async fn metrics_middleware(State(state), request, next) -> Response {
    let start = Instant::now();
    let response = next.run(request).await;
    state.metrics.request_duration.observe(start.elapsed().as_secs_f64());
    state.metrics.requests_total.inc();
    response
}
```
Measures the duration of each request and increments the counter.

### 1.5 Helper functions

#### `json(status, value) -> Response`
Creates a JSON response with the specified HTTP status.

#### `error(status, message) -> Response`
Creates a JSON response `{"error": "message"}`.

#### `SessionResponse`
```rust
pub struct SessionResponse {
    pub id: String,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
}
```
`active` — a flag indicating that the session is currently running (present in `active_sessions`).

### 1.6 Handlers

#### `POST /api/v1/sessions` — `create_session`

Algorithm:
1. Validates `query` (non-empty, trimmed)
2. Checks for LLM API key (503 if missing)
3. Generates `SessionId::new()`
4. **`output_dir` validation** (path traversal protection):
   - Not empty
   - Does not start with `/`
   - Does not contain `\`
   - Does not contain `..`
   - Not an absolute path
   - Exactly 1 component (checked via `Path::components().count() == 1`)
   - If violated — 400 Bad Request
5. Creates the session directory: `base_dir / session_id`
6. Saves the session to the database
7. Writes `output_dir` to the database via `set_session_output_dir`
8. Increments metrics `sessions_total` and `sessions_active`
9. Creates `RunningSession`:
   - `finished: Arc<AtomicBool>` — RAII flag for gauge
   - `steer_tx / steer_rx` — unbounded channel for mid-run steering
   - `cancel: CancellationToken` — cancellation token for the entire agent tree
10. Spawns `spawn_session(...)` — returns `JoinHandle`
11. Saves `RunningSession` into `active_sessions`
12. Returns 202 Accepted with id, status, query, output_dir

#### `spawn_session()` — background task with RAII cleanup

Algorithm:
1. Clones dependencies (llm, tools, db, config, event_tx)
2. Spawns `tokio::spawn` with two parallel tasks via `tokio::join!`:

**metrics_loop** — subscribes to the broadcast event channel:
- For each event: filters by session_id (counts only its own session)
- `AgentSpawned` → `agents_spawned.inc()`
- `ToolCallCompleted` → `tool_calls.inc()`
- `AgentCompleted { tokens_used }` → `tokens_used.inc_by(tokens_used)`
- `SessionCompleted / SessionFailed` → break (ends the loop)

**run** — main logic:
1. Opens contact store (best effort)
2. Creates CrmSync from config
3. Creates `Coordinator::new(...)` with full dependencies
4. Connects steer_rx, cancel token, contact_db, crm
5. Calls `coordinator.execute().await`
6. On error: `db.fail_session` + sends `SessionFailed` event

**RAII Cleanup Guard** — `SessionCleanup` struct with `Drop`:
```rust
struct SessionCleanup {
    state: Arc<AppState>,
    session_id: SessionId,
    finished: Arc<AtomicBool>,
}
impl Drop for SessionCleanup {
    fn drop(&mut self) {
        if !self.finished.swap(true, Ordering::SeqCst) {
            self.state.metrics.sessions_active.dec();
        }
        if let Ok(mut map) = self.state.active_sessions.lock() {
            map.remove(&self.session_id.0);
        }
    }
}
```
Guarantees:
- `sessions_active` gauge is decremented exactly once (atomic swap)
- Record is removed from `active_sessions`
- Executes even on task panic

#### `GET /api/v1/sessions` — `list_sessions`

1. Calls `db.list_sessions()`
2. For each row checks `state.is_active(&row.id)`
3. Returns JSON `{"sessions": [...], "count": N}`

#### `GET /api/v1/sessions/:id` — `get_session_status`

1. Calls `db.get_session(&SessionId(id))`
2. If found — returns `SessionResponse` with the active flag
3. If not found — 404

#### `DELETE /api/v1/sessions/:id` — `cancel_session`

Algorithm:
1. Removes the record from `active_sessions` (atomically via `map.remove`)
2. If record found:
   - Calls `session.cancel.cancel()` — cancels the CancellationToken (entire agent tree)
   - Calls `session.handle.abort()` — aborts the background task
   - Decrements the gauge (with `finished` check)
   - Marks the session as cancelled in the database
   - Returns `{"id": id, "status": "cancelled"}`
3. If not found in active:
   - Checks the database: if exists — 409 Conflict (not running)
   - If does not exist — 404

#### `POST /api/v1/sessions/:id/steer` — `steer_session`

Mid-run steering (fleet E1):
1. Validates message (non-empty)
2. Retrieves `steer_tx` from `active_sessions`
3. If channel found — sends `tx.send(message)`
   - Success — 202 Accepted
   - Channel closed — 409 Conflict
4. If channel not found — checks session existence in the database

#### `GET /api/v1/sessions/:id/results` — `get_session_results`

1. Loads session from the database
2. Checks `status == "completed"` (otherwise 409)
3. Reads `summary.md` from output_dir
4. Scans `findings/` directory, sorts .md files
5. Returns JSON with summary, findings (file + content), metadata

#### `GET /api/v1/agents` — `list_agents`

Calls `db.list_agents()`, returns `{"agents": [...], "count": N}`.

#### `GET /api/v1/agents/:id` — `get_agent_status`

Calls `db.get_agent(&id)`. `AgentDetailRow` is serialized directly to JSON (derives `serde::Serialize`).

#### `GET /health` — `health_check`

1. Checks database availability: `db.list_sessions().is_ok()`
2. Returns:
   - `200 {"status": "ok", "service": "parallel-research", "version": "...", "database": "ok", "active_sessions": N}`
   - `503 {"status": "degraded", "database": "error"}` when database is unavailable

#### `GET /metrics` — `metrics_endpoint`

1. Calls `state.metrics.render_metrics()`
2. Returns with Content-Type `text/plain; version=0.0.4; charset=utf-8`

### 1.7 SSE (Server-Sent Events)

#### `GET /api/v1/events` — `global_events`

Subscribes to the `event_tx` broadcast channel without filtering. Each event is serialized to JSON and transmitted as an SSE `data:` field.

#### `GET /api/v1/sessions/:id/events` — `session_events`

1. Checks session existence in the database (404 if not found)
2. Loads the initial set of session agents: `db.get_session_agent_rows`
3. Creates a stream with filter: `(session_id, agents_set, negative_cache)`

#### `event_stream(rx, filter, db)` — SSE stream builder

Uses `futures::stream::unfold` to create a `Pin<Box<dyn Stream>>`:
- Receives events from the broadcast channel
- If a filter is set — calls `event_belongs_to_session`
- Serializes the event to JSON
- Returns `Event::default().data(data)`
- On `Lagged(n)` — logs and continues
- On `Closed` — returns None (closes the stream)

KeepAlive sends comments by default to maintain the connection.

#### `event_belongs_to_session()` — filtering algorithm with caching

```
Input: event, session_id, agents (HashSet), negative (HashSet), db
Output: bool

1. If event has session_id — compare directly
2. If event has agent_id:
   a. If agent_id in agents → true (positive cache hit)
   b. If agent_id in negative → false (negative cache hit)
   c. Otherwise — DB lookup:
      SELECT id, task, status, tokens_used, summary
      FROM agents WHERE session_id = ?1 ORDER BY id
      Check if agent_id is in the result
   d. If found → add to agents, return true
   e. If not found → add to negative, return false
3. Otherwise → false
```

The positive cache grows as session agents are discovered. The negative cache prevents repeated DB queries for agents from other sessions.

### 1.8 `RunningSession` structure

```rust
struct RunningSession {
    handle: JoinHandle<()>,
    finished: Arc<AtomicBool>,
    steer_tx: tokio::sync::mpsc::UnboundedSender<String>,
    cancel: tokio_util::sync::CancellationToken,
}
```

- `handle` — background task handle (for `abort()`)
- `finished` — atomic flag guaranteeing single gauge adjustment
- `steer_tx` — channel for sending mid-run instructions to agents
- `cancel` — cancellation token, cancels the entire agent tree on DELETE

### 1.9 Constants

```rust
pub const RATE_LIMIT_ENV: &str = "PARALLEL_RESEARCH_RATE_LIMIT";
pub const DEFAULT_RATE_LIMIT: usize = 120; // requests per minute
```

---

## 2. `auth.rs` — authentication and rate limiting

### 2.1 API keys — `ApiKeyAuth`

```rust
pub struct ApiKeyAuth {
    keys: HashMap<String, ApiKeyInfo>,
}

pub struct ApiKeyInfo {
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

#### `from_env()` — loading from environment variable

Reads `PARALLEL_RESEARCH_API_KEYS` (comma-separated). Each key gets the name `key-{index}`. Empty entries are skipped.

#### `with_key(key, name)` — programmatic registration
Inserts a key into the HashMap with metadata.

#### `is_enabled() -> bool`
Returns `true` if at least one key is registered.

#### `validate(key) -> Option<&ApiKeyInfo>`
Simple HashMap lookup. Returns `None` for unknown keys.

### 2.2 Key extraction — `extract_api_key(headers)`

Supports two formats:
1. **Authorization: Bearer `<key>`** — preferred
2. **X-Api-Key: `<key>`** — fallback

Algorithm:
1. Reads the `Authorization` header
2. Attempts to strip the prefix `"Bearer "` or `"bearer "` (case-insensitive)
3. Trims the result
4. If non-empty — returns Some
5. Otherwise — reads `x-api-key`, trims, filters empty
6. If both are absent — None

### 2.3 `AuthPrincipal`

```rust
pub struct AuthPrincipal(pub String);
```

Inserted into request extensions after successful authentication. Used by the rate limiter as the client key.

### 2.4 `auth_middleware` — authentication middleware

```rust
pub async fn auth_middleware(State(state), headers, mut request, next) -> Result<Response, StatusCode>
```

Algorithm:
1. If `state.auth.is_enabled()`:
   - Extracts the key from headers
   - Validates via `state.auth.validate(key)`
   - If valid — uses `info.name` as principal
   - If invalid — returns `Err(StatusCode::UNAUTHORIZED)`
2. If authentication is disabled — principal = `"anonymous"`
3. Inserts `AuthPrincipal(principal)` into request extensions
4. Calls `next.run(request).await`

### 2.5 Rate Limiter — `RateLimiter`

```rust
pub struct RateLimiter {
    requests: HashMap<String, Vec<Instant>>,
    limit: usize,
    window: Duration,
}
```

#### Algorithm — sliding window:

```rust
pub fn check(&mut self, key: &str, now: Instant) -> bool {
    let entry = self.requests.entry(key.to_string()).or_default();
    // Removes requests older than the window
    entry.retain(|t| now.duration_since(*t) < window);
    if entry.len() < self.limit {
        entry.push(now);  // Records the new request
        true               // Allowed
    } else {
        false              // Rejected (429)
    }
}
```

- `limit` — maximum requests per window (minimum 1)
- `window` — window duration (default 60 seconds)
- Key — API key name or client IP address

### 2.6 `rate_limit_middleware`

```rust
pub async fn rate_limit_middleware(State(state), request, next) -> Result<Response, StatusCode>
```

Algorithm:
1. Extracts the key:
   - `AuthPrincipal` from extensions (preferred)
   - IP address from `ConnectInfo<SocketAddr>`
   - `"anonymous"` (fallback)
2. Locks `state.rate_limiter` (Mutex)
3. Calls `limiter.check(&key, Instant::now())`
4. If `false` — returns `Err(StatusCode::TOO_MANY_REQUESTS)`
5. If `true` — `next.run(request).await`

### 2.7 Constants

```rust
pub const API_KEYS_ENV: &str = "PARALLEL_RESEARCH_API_KEYS";
```

---

## 3. `metrics.rs` — Prometheus metrics

### 3.1 Primitives

#### `Counter` — monotonically increasing counter

```rust
pub struct Counter {
    value: AtomicU64,
}
```

- `inc()` — `fetch_add(1, Relaxed)`
- `inc_by(n)` — `fetch_add(n, Relaxed)`
- `get()` — `load(Relaxed)`

#### `Gauge` — value that can increase and decrease

```rust
pub struct Gauge {
    value: AtomicI64,
}
```

- `inc()` — `fetch_add(1, Relaxed)`
- `dec()` — `fetch_sub(1, Relaxed)`
- `set(v)` — `store(v, Relaxed)`
- `get()` — `load(Relaxed)`

#### `Histogram` — histogram with fixed buckets

```rust
pub struct Histogram {
    buckets: Vec<f64>,        // Bucket boundaries (sorted)
    counts: Vec<AtomicU64>,   // Cumulative counters (<= bound)
    count: AtomicU64,         // Total number of observations
    sum_bits: AtomicU64,      // Sum as f64::to_bits
}
```

##### `new(buckets: Vec<f64>)`
Sorts the buckets, creates an `AtomicU64` for each.

##### `observe(value: f64)`
```rust
for (i, bound) in self.buckets.iter().enumerate() {
    if value <= *bound {
        self.counts[i].fetch_add(1, Relaxed);
    }
}
self.count.fetch_add(1, Relaxed);
// Atomic sum update via fetch_update
self.sum_bits.fetch_update(AcqRel, Acquire, |bits| {
    Some((f64::from_bits(bits) + value).to_bits())
});
```

Cumulative recording: if the value is 0.05 with buckets [0.1, 1.0, 10.0], all three buckets are incremented (0.05 ≤ 0.1, ≤ 1.0, ≤ 10.0).

##### `count()` / `sum()`
Reads atomic values.

##### `render(name, help)` — Prometheus format render

```
# HELP name help text
# TYPE name histogram
name_bucket{le="0.005"} 0
name_bucket{le="0.01"} 1
...
name_bucket{le="+Inf"} N
name_sum 12.34
name_count N
```

### 3.2 `Metrics` — metrics registry

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

#### `new()`
Initializes all metrics. The request duration histogram uses default buckets:
```rust
const DEFAULT_DURATION_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];
```
This is 11 buckets from 5ms to 10s.

#### `render_metrics() -> String`

Generates the full text in Prometheus text exposition format:

```
# HELP pr_sessions_total Total number of research sessions created
# TYPE pr_sessions_total counter
pr_sessions_total N

# HELP pr_sessions_active Number of research sessions currently running
# TYPE pr_sessions_active gauge
pr_sessions_active N

# HELP pr_agents_spawned_total Total number of research agents spawned
# TYPE pr_agents_spawned_total counter
pr_agents_spawned_total N

# HELP pr_tokens_used_total Total number of LLM tokens used by completed agents
# TYPE pr_tokens_used_total counter
pr_tokens_used_total N

# HELP pr_tool_calls_total Total number of tool calls completed
# TYPE pr_tool_calls_total counter
pr_tool_calls_total N

# HELP pr_http_requests_total Total number of HTTP requests served
# TYPE pr_http_requests_total counter
pr_http_requests_total N

# HELP pr_request_duration_seconds HTTP request duration in seconds
# TYPE pr_request_duration_seconds histogram
pr_request_duration_seconds_bucket{le="0.005"} N
pr_request_duration_seconds_bucket{le="0.01"} N
...
pr_request_duration_seconds_bucket{le="+Inf"} N
pr_request_duration_seconds_sum N
pr_request_duration_seconds_count N
```

### 3.3 Render helper functions

#### `render_counter(out, name, help, counter)`
```
# HELP name help
# TYPE name counter
name value
```

#### `render_gauge(out, name, help, gauge)`
```
# HELP name help
# TYPE name gauge
name value
```

### 3.4 Properties of atomic metrics

- **Lock-free**: all operations use `AtomicU64`/`AtomicI64` with `Relaxed` ordering (sufficient for metrics)
- **Thread-safe**: `Counter`, `Gauge`, `Histogram` implement `Sync`
- **Cheap**: `fetch_add` is a single CPU instruction on most architectures
- **Histogram sum**: uses `fetch_update` with `AcqRel` for atomic read-modify-write on `f64` via bit representation