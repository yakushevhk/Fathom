# HTTP API

Fathom provides a self-hosted REST API built on **Axum** for controlling autonomous worker sessions. It exposes session lifecycle and agent inspection, SSE event streams, steering and approvals, optional memory and jobs, optional computer relay, credentials, coworkers/channels/schedules, governance, redacted replay, observability, notifications, and a read-only AG-UI compatibility bridge. API-key authentication and rate limiting apply when configured; loopback is the default bind surface.

Start:
```bash
fathom serve --port 8080
```

Base URL: `http://localhost:8080`

---

## Authentication

If `FATHOM_API_KEYS` (comma-separated list of keys) is set, `/api/v1/*` requests require one of those API keys. When it is unset, access is open for development; bind only to a trusted interface. Public endpoints (`/health`, `/metrics`, `/dashboard`) are not protected, while dashboard data requests use `/api/v1` routes.

```bash
# Bearer token
curl -H "Authorization: Bearer your-key" http://localhost:8080/api/v1/sessions

# Or X-Api-Key header
curl -H "X-Api-Key: your-key" http://localhost:8080/api/v1/sessions
```

The `Authorization: Bearer` header is checked first; if absent, the `X-Api-Key` header is used. If neither is present or the key is invalid, the server returns `401 Unauthorized`. If the variable is not set — access is open (for development).

**Rate limiting**: sliding-window request limit per client identity (the authenticated principal name, or the client IP when auth is disabled). The default limit is **120 requests per minute** per client. Override with the `FATHOM_RATE_LIMIT` environment variable (set to the desired requests-per-minute value). Each client's window is tracked independently: a burst of requests from one client does not affect another client's budget. On exceeding the limit, the server returns `429 Too Many Requests`. Public endpoints (`/health`, `/metrics`, `/dashboard`) are exempt from rate limiting.

---

## Endpoints

### `GET /health`

Health check — returns `200 OK` with the current server status and a count of active sessions.

**Response 200:**
```json
{
  "status": "ok",
  "active_sessions": 2
}
```

---

**Available metrics:**

| Metric | Type | Description |
|--------|------|-------------|
| `pr_sessions_total` | Counter | Total number of research sessions created |
| `pr_sessions_active` | Gauge | Number of research sessions currently running |
| `pr_agents_spawned_total` | Counter | Total number of research agents spawned |
| `pr_tokens_used_total` | Counter | Total number of LLM tokens consumed by completed agents |
| `pr_tool_calls_total` | Counter | Total number of tool invocations completed |
| `pr_http_requests_total` | Counter | Total number of HTTP requests served |
| `pr_request_duration_seconds` | Histogram | HTTP request duration in seconds (buckets: 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0) |

Metrics are recorded by middleware wrapping all API routes. Counters and gauges are atomic integers; the histogram uses fixed bucket counts. The `pr_sessions_active` gauge is decremented exactly once per session regardless of cancellation path, using an `AtomicBool` guard in the session's Drop handler.

---

### `GET /dashboard`

Built-in live dashboard (single HTML file, no build step): session and job tables, agent tree, memory panel, and a live event feed via `EventSource` (`GET /api/v1/events`). Data is polled every 5 seconds. If authorization is enabled (`FATHOM_API_KEYS`), enter the key in the header field — it is saved in localStorage and injected into `X-Api-Key`. The route lies outside `/api/v1`, the page itself is not authorized; all data is fetched only through protected endpoints.

The dashboard provides a local visual overview of worker sessions, agent trees, events, memory (when enabled), and durable jobs. It is a convenience surface for the self-hosted server, not a hosted control plane.

---

### `POST /api/v1/sessions`

Create a new autonomous task session. The server spawns a coordinator that manages the agent tree, persists progress to SQLite, and streams events to subscribers. The session runs asynchronously — the response returns immediately with the session ID and status.

**Request:**
```json
{
  "query": "Research the AI agent market",
  "output_dir": "./results"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | ✅ | Natural-language task (non-empty) |
| `output_dir` | string | ❌ | Single relative directory name override (sanitized: no path separators, no `..`, no absolute paths) |

The `output_dir` field is validated to prevent directory traversal: it must be a single relative directory name with no slashes, backslashes, or `..` components. The server creates `<output_dir>/<session_id>` as the working directory. If omitted, the configured default output directory is used.

**Response 202:**
```json
{
  "id": "019fd38a-9a7c-7322-a671-64427832f0eb",
  "status": "running",
  "query": "Research the AI agent market",
  "output_dir": "./research-output/019fd38a-..."
}
```

**Errors:**
- `400` — empty query or invalid `output_dir`
- `503` — LLM api_key is not configured
- `500` — filesystem error creating the output directory

---

### `GET /api/v1/sessions`

List all sessions. Results are read from the persistent SQLite database and include both active and completed sessions. The `active` field indicates whether the session is currently executing in-process.

**Response 200:**
```json
{
  "sessions": [
    {
      "id": "019fd38a-...",
      "query": "Research the AI agent market",
      "status": "completed",
      "output_dir": "./research-output",
      "total_tokens": 890042,
      "total_agents": 3,
      "created_at": "2026-08-05T14:55:00Z",
      "updated_at": "2026-08-05T14:58:00Z",
      "active": false
    }
  ],
  "count": 1
}
```

**Session statuses**: `running`, `completed`, `failed`, `cancelled`

---

### `GET /api/v1/sessions/:id`

Status of a specific session.

**Response 200:**
```json
{
  "id": "019fd38a-...",
  "query": "...",
  "status": "running",
  "output_dir": null,
  "total_tokens": 450000,
  "total_agents": 3,
  "created_at": "...",
  "updated_at": "...",
  "active": true
}
```

`active` — whether the session is currently executing (in-process and tracked in the server's active sessions map).

**Errors:**
- `404` — session not found

---

### `GET /api/v1/sessions/:id/results`

Results of a completed session. Reads the output directory on disk: the `summary.md` file and all individual finding markdown files from the `findings/` subdirectory. Findings are returned in alphabetical order by filename.

**Response 200:**
```json
{
  "session_id": "019fd38a-...",
  "status": "completed",
  "output_dir": "./research-output",
  "total_tokens": 890042,
  "total_agents": 3,
  "summary": "## TL;DR\n\nFull summary.md text...",
  "findings": [
    {
      "file": "finding-1.md",
      "content": "Finding content..."
    },
    {
      "file": "finding-2.md",
      "content": "..."
    }
  ]
}
```

**Errors:**
- `404` — session not found
- `409` — session not yet completed (status is `running`, `failed`, or `cancelled`)

---

### `DELETE /api/v1/sessions/:id`

Cancel a running session. Cancels the entire agent tree via a cancellation token (fan-out to all child agents and background spawns), aborts the coordinator background task, decrements the active sessions gauge, and marks the session as `cancelled` in the database. The cancellation is applied layer by layer: first the cancellation token cancels in-flight agent operations, then the outer task handle is aborted. This ensures clean shutdown without orphaned child processes.

**Response 200:**
```json
{
  "id": "019fd38a-...",
  "status": "cancelled"
}
```

Idempotent: re-cancelling a completed session is not an error. If the session is not running but exists (e.g., already completed), returns `409 Conflict` with the current status. If the session ID is unknown, returns `404`.

**Errors:**
- `404` — session not found
- `409` — session exists but is not currently running

---

### `GET /api/v1/agents`

List all agents across all sessions, including historical agents from already-completed sessions. Each agent record includes its role (e.g., `researcher`), depth in the agent tree, and token usage.

**Response 200:**
```json
{
  "agents": [
    {
      "id": "019fd38a-...",
      "session_id": "019fd38a-...",
      "role": "researcher",
      "task": "Research performance...",
      "status": "completed",
      "depth": 1,
      "tokens_used": 218567,
      "created_at": "...",
      "completed_at": "..."
    }
  ],
  "count": 3
}
```

---

### `GET /api/v1/agents/:id`

Status of a specific agent.

**Response 200:**
```json
{
  "id": "019fd38a-...",
  "session_id": "...",
  "role": "researcher",
  "task": "...",
  "status": "completed",
  "depth": 1,
  "tokens_used": 218567,
  "created_at": "...",
  "completed_at": "..."
}
```

**Errors:**
- `404` — agent not found

---

### `POST /api/v1/sessions/:id/steer`

Inject an instruction into a running session (mid-run steering). The instruction is sent through an unbounded mpsc channel to the coordinator, which delivers it to the active agent tree at the next turn boundary. Agents pick up the steering message on their next iteration — the session does not need to be restarted. This is useful for changing research direction, narrowing scope, or correcting course mid-execution.

**Request:**
```json
{ "message": "Focus only on companies from Kazan" }
```

**Response 202:**
```json
{ "id": "019fd38a-...", "steered": true, "message": "Focus only on companies from Kazan" }
```

**Errors:**
- `400` — empty message
- `404` — session not found
- `409` — session exists but is not running, or the steering channel is closed (session is shutting down)

---

### `POST /api/v1/sessions/:id/answer`

Answer an agent's question (`question` tool) waiting for an operator. When an agent invokes the `question` tool, it emits a `question_asked` event with a `request_id`. The agent pauses and waits for the operator's response. The HTTP client extracts the `request_id` from the SSE event stream and sends the answer through this endpoint. The answer is forwarded through a oneshot channel to the waiting agent, which resumes execution with the operator's input.

`request_id` is taken from the `question_asked` event.

**Request:**
```json
{ "request_id": "019fe7ad-...", "text": "Work on the EU region" }
```

**Response 200:**
```json
{ "answered": true, "request_id": "019fe7ad-..." }
```

**Errors:**
- `404` — no pending question with this `request_id` (may have already been answered or timed out)
- `400` — this `request_id` belongs to an approval request — use `/approve` instead
- `410` — the agent is no longer waiting for this answer (stale or cancelled)

---

### `POST /api/v1/sessions/:id/approve`

Approve/deny a side-effect tool call (`approval_tools`). When an agent wants to execute a tool that has side effects (e.g., writing files, sending emails, making API calls), it emits an `approval_requested` event with a `request_id`. The agent pauses and waits for the operator's decision. The HTTP client extracts the `request_id` from the SSE event stream and sends the approval decision through this endpoint.

`request_id` is taken from the `approval_requested` event.

**Request:**
```json
{ "request_id": "019fe7ad-...", "approved": true }
```

**Response 200:**
```json
{ "approved": true, "request_id": "019fe7ad-..." }
```

**Errors:**
- `404` — no pending approval with this `request_id`
- `400` — this `request_id` belongs to a question — use `/answer` instead
- `410` — the agent is no longer waiting for this approval

---

### `GET /api/v1/sessions/:id/events` / `GET /api/v1/events`

SSE (Server-Sent Events) stream of agent events. The first endpoint filters by session ID; the second is global (all sessions on the server). Both use a `broadcast::Receiver` subscribed to the server-wide event bus (channel capacity: 1024 events). Each event is a JSON object with a `type` discriminator field.

**Event types emitted on the stream:**

| Event type (`type` field) | Description |
|---------------------------|-------------|
| `session_started` | A new session began execution |
| `session_completed` | Session finished successfully |
| `session_failed` | Session terminated with an error |
| `session_forked` | A session spawned a child session |
| `agent_spawned` | A new agent was created (includes `parent`, `role`, `depth`) |
| `agent_state_changed` | Agent transitioned to a new state |
| `agent_completed` | Agent finished its work |
| `agent_failed` | Agent encountered an error |
| `tool_call_started` | Agent began executing a tool |
| `tool_call_completed` | Tool returned a result |
| `llm_stream_chunk` | Streaming text delta from the LLM |
| `question_asked` | Agent invoked the `question` tool — includes `request_id` |
| `approval_requested` | Agent wants to run a side-effect tool — includes `request_id` |
| `finding` | Agent discovered a finding |
| `file_change_undone` | A file operation was rolled back |
| `title_generated` | Session title was auto-generated |

The session-filtered variant snapshots the agent set from the database at connect time. Agents spawned after connection are discovered via a DB lookup on first sight, with a negative cache to avoid re-querying agents proven to belong to other sessions. The stream includes a keep-alive heartbeat to prevent proxy timeouts.

**SSE format (each event is a JSON line):**
```
event: message
data: {"type":"agent_spawned","id":"...","role":"researcher","depth":1}
```

---

## Memory API

Long-term semantic memory store (see [MEMORY-KB.md](MEMORY-KB.md)). All memory endpoints are available only when `[memory] enabled = true` in the server configuration. The memory system uses embeddings for semantic search, supports fact absorption through a pipeline, and provides GC for lifecycle management. Memories are versioned — soft deletes preserve history.

### `GET /api/v1/memories`

List or search memories. Without the `q` query parameter, returns a paginated list with optional filters (`scope`, `scope_key`, `status`, `limit`). With `q`, performs a hybrid search (semantic + keyword) returning the top `top_k` results ranked by relevance score.

**Query parameters:**
- `q` — search query (optional; when absent, returns a list instead of search results)
- `scope` — filter by scope (e.g., `agent`, `session`, `global`)
- `scope_key` — filter by scope-specific key
- `status` — filter by status (`active` by default; use `all` to include archived)
- `limit` — max results when listing (default: 20)
- `top_k` — max results when searching (default: 10)

**Response 200:**
```json
{ "memories": [
  { "id": "019fe796-...", "content": "...", "scope": "agent",
    "status": "active", "confidence": 0.9, "importance": 1.0,
    "tags": ["contact"], "created_at": "...", "score": 0.83 }
] }
```

### `POST /api/v1/memories/absorb`

Save facts via the absorb pipeline. The body is an `AbsorbRequest` containing `facts[]` (each fact has a `content` string, optional `confidence`, `importance`, and `tags`), `source` (a string identifying the origin of the facts), `scope` and `scope_key` for namespacing, and an optional `dry_run` flag. When `dry_run` is true, the pipeline simulates absorption and returns what would be stored without actually writing to the database.

### `GET /api/v1/memories/stats`

Returns store-wide statistics: counters by scope and status (active vs. archived), the embedding model name used for vector search, and the entity graph size (number of deduplicated entities across all memories).

### `POST /api/v1/memories/distill`

Distill run facts into persistent agent knowledge. Accepts query parameters `?session=<key>` (required — the session ID whose facts to distill) and `?dry_run=true|false` (optional). The distillation process promotes high-value, cross-session facts from a specific research run into the long-term memory store, making them available for future sessions.

### `POST /api/v1/memories/gc`

Storage garbage collection. Accepts `?ttl_days=<N>` (default: 30) and `?dry_run=true|false`. Archives:
- **Expired facts**: memories whose `expires_at` timestamp is in the past
- **Stale facts**: untouched run facts that have not been accessed or modified beyond the TTL threshold
- **Overgrown scope groups**: compacts groups with too many facts into a single consolidated entry (N→1)

Returns a detailed report:
```json
{ "expired_archived": 5, "stale_archived": 12,
  "groups_compacted": 2, "facts_compacted": 24,
  "errors": [], "dry_run": true }
```

Nothing is deleted — archived memories remain accessible via `?follow=full_history`.

### `GET /api/v1/memories/:id`

Single memory record. Accepts `?follow=latest|full_history` (default: `latest`). `latest` returns the current active version; `full_history` traverses the version chain and returns all past versions of the memory.

### `DELETE /api/v1/memories/:id`

Archive (soft delete). The memory is marked as archived and excluded from default queries, but its full history is preserved and accessible via `?follow=full_history`.

---

## Computer API — single computer mode

Relay routes to the configured computer-use service (a small loopback HTTP service, by default `http://127.0.0.1:8765`, overridable via `FATHOM_COMPUTER_SERVICE_URL`; the upstream token comes from `COMPUTER_TOKEN`). The server validates requests, forwards them to the computer service, and proxies the response back.

Requests are proxied verbatim to the upstream service and may contain secrets; responses can include `mimeType`-tagged base64 data (e.g. screenshots). Upstream calls and websocket writes have explicit timeouts so a disconnected or wedged computer service cannot retain a request task indefinitely.

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/v1/computers/session` | Start or refresh the browser session |
| `GET` | `/api/v1/computers/health` | Computer service health check |
| `GET` | `/api/v1/computers/snapshot` | Accessibility snapshot of the current page |
| `POST` | `/api/v1/computers/navigate` | Navigate to a URL |
| `POST` | `/api/v1/computers/click` | Click at coordinates / on an element |
| `POST` | `/api/v1/computers/type` | Type text into the page |
| `POST` | `/api/v1/computers/key` | Send a keyboard key chord |
| `POST` | `/api/v1/computers/secret` | Enter a secret without returning/logging its value |
| `GET` | `/api/v1/computers/screenshot` | Capture a screenshot (`{mimeType, data}` base64) |
| `GET` | `/api/v1/computers/tabs` | List open tabs |
| `POST` | `/api/v1/computers/tabs/open` | Open a new tab |
| `POST` | `/api/v1/computers/tabs/:tab_id/activate` | Activate a tab |
| `POST` | `/api/v1/computers/tabs/:tab_id/close` | Close a tab |
| `POST` | `/api/v1/computers/control/take` | Take operator control of the computer |
| `POST` | `/api/v1/computers/control/release` | Release operator control |
| `GET` | `/api/v1/computers/files` | List workspace files |
| `GET` | `/api/v1/computers/files/read?path=...` | Read a workspace file |
| `PUT` | `/api/v1/computers/files/write` | Write a workspace file (raw body) |
| `DELETE` | `/api/v1/computers/files?path=...` | Delete a workspace file |

**Notes:**
- Action bodies (`navigate`, `click`, `type`, `key`, `session`, `tabs/open`) are arbitrary JSON passed straight through to the computer service.
- `GET /api/v1/computers/screenshot` returns `{ "mimeType": "image/png", "data": "<base64>" }`.
- The files workspace body writes are capped (default 1.1 MB); screenshots are capped at 20 MB and other JSON bodies at 2 MB — oversized payloads are rejected.

### `POST /api/v1/computers/secret`

Enter a secret directly into the computer without returning or logging the value. The request body is the secret action JSON; the upstream service returns only the refreshed page metadata/snapshot. The value is never echoed back to the caller.

---

## Computer API — per-agent (Docker supervisor mode)

Agent-scoped routes resolve each agent's supervised computer container and proxy to it. Available in addition to the single-computer mode: `health`, `snapshot`, `screenshot`, `screen` (websocket), input actions (`navigate`, `click`, `type`, `key`), control hand-off, and file operations. Container lifecycle is managed by the supervisor endpoints below.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/computers/:agent_id/snapshot` | Snapshot of the agent's browser |
| `POST` | `/api/v1/computers/:agent_id/navigate` | Navigate the agent's browser |
| `POST` | `/api/v1/computers/:agent_id/click` | Click in the agent's browser |
| `POST` | `/api/v1/computers/:agent_id/type` | Type text in the agent's browser |
| `POST` | `/api/v1/computers/:agent_id/key` | Send a key chord to the agent's browser |
| `GET` | `/api/v1/computers/:agent_id/screenshot` | Screenshot of the agent's browser |
| `GET` | `/api/v1/computers/:agent_id/screen` | **WebSocket** — continuous screen stream (binary frames, ~500 ms polling interval) |
| `GET` | `/api/v1/computers/:agent_id/health` | Health of the agent's computer |
| `POST` | `/api/v1/computers/:agent_id/control/take` | Take operator control |
| `POST` | `/api/v1/computers/:agent_id/control/release` | Release operator control |
| `GET` | `/api/v1/computers/:agent_id/files` | List the agent's workspace files |
| `GET` | `/api/v1/computers/:agent_id/files/read?path=...` | Read a file from the agent's workspace |
| `PUT` | `/api/v1/computers/:agent_id/files/write` | Write to the agent's workspace (raw body) |
| `DELETE` | `/api/v1/computers/:agent_id/files?path=...` | Delete from the agent's workspace |

### `GET /api/v1/computers/:agent_id/screen`

Upgrades to a websocket and emits binary image frames at a bounded polling interval (currently 500 ms). The client may close the websocket at any time; polling stops on close, read error, timeout, or a client that does not accept frames promptly.

---

### Docker supervisor endpoints

Available only when the Docker computer supervisor is configured (`COMPUTER_TOKEN` set). If not configured, returns `503 Service Unavailable`.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/computers` | List supervised containers |
| `POST` | `/api/v1/computers/:agent_id/ensure` | Ensure a container exists for the agent (create if missing) |
| `POST` | `/api/v1/computers/:agent_id/stop` | Stop the agent's container |
| `POST` | `/api/v1/computers/:agent_id/reset` | Reset the agent's container to a clean state |

**Responses:**
- `GET /api/v1/computers` → `200` with the container list
- `POST /api/v1/computers/:agent_id/stop` → `{ "agent_id": "...", "stopped": true }`
- `POST /api/v1/computers/:agent_id/reset` → `{ "agent_id": "...", "reset": true }`
- `503` — supervisor not configured; `502` — upstream supervisor error

---

## Credentials API

An encrypted vault of named secrets. Secrets are encrypted with **AES-256-GCM** at rest (key from `FATHOM_CREDENTIAL_KEY`, a 32-byte key in hex or base64). The vault stores only metadata in query results — the plaintext secret is never returned by the API.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/credentials` | List credential metadata (id, name, kind, timestamps) |
| `POST` | `/api/v1/credentials` | Store a credential (upsert by unique `name`) |
| `DELETE` | `/api/v1/credentials/:id` | Delete a credential |

**Request (POST):**
```json
{ "name": "outreach-smtp", "kind": "password", "secret": "sup3rs3cret" }
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | ✅ | Unique credential name (upsert target, ≤128 bytes) |
| `kind` | string | ✅ | Credential type/label (≤64 bytes) |
| `secret` | string | ✅ | Plaintext secret to encrypt (≤64 KiB, never returned) |

**Response 200 (GET):** array of `{ "id", "name", "kind", "created_at", "updated_at" }`.
**Response 201 (POST):** the stored credential metadata (no secret).
**Errors:**
- `400` — invalid name/kind/secret, or encryption key not configured
- `404` — credential id not found (DELETE)
- `204` (DELETE) — successful deletion

---

## Coworkers API

Persistent coworker profiles for recurring or operator-managed workers. Coworkers have a name, role, prompt, and visibility; channels link a coworker to sessions. These are local records in the self-hosted persistence layer and use the same API-key and rate-limit behavior as the rest of `/api/v1`.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/coworkers` | List all configured coworkers |
| `POST` | `/api/v1/coworkers` | Create a coworker |
| `GET` | `/api/v1/coworkers/:id` | Fetch one coworker |
| `PUT` / `PATCH` | `/api/v1/coworkers/:id` | Replace an existing coworker |
| `DELETE` | `/api/v1/coworkers/:id` | Delete a coworker |

**Request (POST / PUT):**
```json
{
  "name": "outreach",
  "title": "Outreach Specialist",
  "role": "researcher",
  "prompt": "You research prospects and draft outreach...",
  "visibility": "private",
  "active": true
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | ✅ | — | Display name (≤100 chars) |
| `title` | string | ❌ | `""` | Job title (≤200 chars) |
| `role` | string | ❌ | `""` | Agent role (≤100 chars) |
| `prompt` | string | ✅ | — | System prompt (≤20,000 chars) |
| `visibility` | string | ❌ | `"private"` | Visibility scope (≤32 chars) |
| `active` | bool | ❌ | `true` | Whether the coworker is active |

**Responses:** `GET /api/v1/coworkers` → `{ "coworkers": [ ... ] }`; create returns `201` with `{ "coworker": { "id", "name", "title", "role", "prompt", "visibility", "active", "created_at", "updated_at" } }`.

**Errors:** `400` — invalid id or field too long; `404` — coworker not found; `500` — database error.

---

## Channels API

Channels link a coworker to sessions, giving the coworker a conversation context.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/channels?coworker_id=...` | List channels for a coworker (`coworker_id` required) |
| `POST` | `/api/v1/channels` | Create a channel, optionally attached to a session |
| `PUT` / `PATCH` | `/api/v1/channels/:id` | Update a channel title/session mapping |
| `DELETE` | `/api/v1/channels/:id` | Delete a channel |

**Request (POST):**
```json
{ "coworker_id": "outreach", "title": "Q3 Leads", "session_id": "019fd38a-..." }
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `coworker_id` | string | ✅ | The coworker this channel belongs to |
| `title` | string | ✅ | Channel title (≤200 chars) |
| `session_id` | string | ❌ | Session to link (validated to exist) |

**Responses:** list → `{ "channels": [ ... ] }`; create → `201` `{ "channel": { "id", "coworker_id", "title", "session_id", "created_at", "updated_at" } }`.

**Errors:** `400` — missing/`coworker_id` or bad fields; `404` — coworker or session not found; `500` — database error.

---

## Schedules API

Cron-like scheduling for coworkers. Schedules are persisted and claimed by worker processes with `POST /api/v1/schedules/claim` (bounded scheduler tick; no jobs are spawned here).

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/schedules` | List all schedules |
| `POST` | `/api/v1/schedules` | Create a schedule |
| `GET` | `/api/v1/schedules/:id` | Fetch one schedule |
| `PUT` / `PATCH` | `/api/v1/schedules/:id` | Update a schedule |
| `DELETE` | `/api/v1/schedules/:id` | Delete a schedule |
| `POST` | `/api/v1/schedules/claim` | Claim due schedules (`{ "limit": N }`, default 25, max 100) |

**Request (POST / PUT):**
```json
{
  "coworker_id": "outreach",
  "cron": "0 9 * * 1-5",
  "timezone": "Europe/Moscow",
  "query": "Research new fintech startups",
  "enabled": true,
  "next_run": null
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `coworker_id` | string | ✅ | — | Coworker to schedule (validated id) |
| `cron` (alias `cron_expression`) | string | ✅ | — | Five-field cron expression (≤256 chars, validated) |
| `timezone` | string | ❌ | `"UTC"` | IANA timezone (≤128 chars) |
| `query` | string | ✅ | — | Session query to run (≤20,000 chars) |
| `enabled` | bool | ❌ | `true` | Whether the schedule is active |
| `next_run` | string | ❌ | `null` | Optional next-run override (≤64 chars) |

**Responses:** list → `{ "schedules": [...], "count": N }`; create → `201` `{ "schedule": { "id", "coworker_id", "cron_expression", "timezone", "query", "enabled", "next_run", "last_run", "created_at", "updated_at" } }`; claim → `{ "schedules": [...], "count": N }`.

**Errors:** `400` — invalid id, empty/oversized field, or invalid cron expression; `404` — schedule not found; `500` — database error.

---

## Governance API

Policy engine for governing agent tool actions. Governed decisions are recorded to an audit log. Governance is disabled by default unless `FATHOM_GOVERNANCE_ENABLED=true` (policy can also be seeded via `FATHOM_GOVERNANCE_POLICY`).

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/governance/policy` | Get the active policy |
| `PUT` | `/api/v1/governance/policy` | Replace the active policy (max 1000 rules) |
| `POST` | `/api/v1/governance/decide` | Evaluate an action against the policy |
| `GET` | `/api/v1/governance/audit` | List audit events (newest first) |

### `POST /api/v1/governance/decide`

Evaluate an action against the current policy. When governance is disabled, policy enforcement is bypassed. When enabled, deny rules and unmatched actions are handled according to the active policy; audit records remain redacted.

**Request:** an `ActionContext`:
```json
{
  "agent": "researcher-123",
  "session": "019fd38a-...",
  "tool": "web_search",
  "args": { "query": "..." }
}
```

Optional context fields: `url`, `element`, `file`, `intent`, `mcp_metadata`.

**Response 200:** a decision object `{ "decision": "allow" | "deny" }` (plus policy metadata).

### `GET /api/v1/governance/audit`

**Query parameters:** `limit` (default 50, max 200), `decision` (`allow`/`deny`, case-insensitive), `agent`, `session`.

**Response 200:** array of audit rows, newest first:
```json
[{
  "id": "...",
  "timestamp": "2026-08-20T12:00:00Z",
  "agent": "researcher-123",
  "session": "...",
  "tool": "web_search",
  "args": "{\"query\":\"...\"}",
  "url": null, "element": null, "file": null, "intent": null, "mcp_metadata": null,
  "decision": "allow"
}]
```

Audit rows are **redacted** — secrets in `args`/`mcp_metadata` are never stored raw.

### `GET` / `PUT /api/v1/governance/policy`

Policy is a set of rules using simple, safe string matching (never code evaluation). A deny rule always takes precedence over an allow rule; unmatched actions are denied.

```json
{ "rules": [
  { "effect": "deny", "tool": "shell", "host": null, "path": null, "intent": null },
  { "effect": "allow", "tool": "web_search" }
] }
```

**Errors:** `400` — too many rules (>1000) on PUT; `500` — decide/audit failure.

---

## Replay API

Redacted action recording timeline. Replay rows describe the bounded execution timeline of governed actions (records are separate from audit decisions — audit explains *why* an action was allowed/denied, replay describes *when* it ran). Payloads are redacted before storage and again at the API boundary.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/replay` | List redacted recorded actions (newest first) |

**Query parameters:**
- `session` — filter by session id (≤256 bytes)
- `agent` — filter by agent id (≤256 bytes)
- `limit` — max results (default 50, max 200)

**Response 200:**
```json
{ "actions": [
  {
    "id": "...",
    "agent": "...",
    "session": "...",
    "tool": "computer_click",
    "args_redacted": "{...}",
    "decision": "allow",
    "started_at": "...",
    "completed_at": "...",
    "duration_ms": 1234,
    "result_redacted": "...",
    "screenshot_before": null,
    "screenshot_after": null,
    "policy_version": "..."
  }
] }
```

**Errors:** `400` — invalid filter value; `500` — database error.

---

## Observability API

Live operational counters exposing process metrics and bounded audit counts.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/observability/summary` | Live counters and audit totals |

**Response 200:**
```json
{
  "active_sessions": 2,
  "sessions_total": 42,
  "agents_spawned": 128,
  "tool_calls": 890,
  "tokens_used": 15234567,
  "audit_events": 500,
  "audit_denials": 3,
  "audit_counts_truncated": false
}
```

`audit_counts_truncated` is `true` when the bounded audit sample reached its hard limit (10,000 rows), so counts must be treated as lower bounds.

---

## Notifications API

Safe, operator-triggered notification delivery through channels configured in the server `[notifications]` section. The endpoint deliberately accepts only a symbolic channel name — never destination details, credentials, or message content.

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/v1/notifications/test` | Send a bounded test message on one configured channel |

**Request:**
```json
{ "channel": "telegram" }
```

`channel` must be one of `webhook`, `email`, `telegram` (case-insensitive).

**Response 200:** `{ "channel": "telegram", "status": "sent" }`

**Errors:**
- `400` — unknown channel name
- `409` — channel symbol is known but not configured server-side
- `502` — delivery failed (addresses/transport details are deliberately not echoed)
- `504` — delivery timed out (10 s)

---

## AG-UI compatibility bridge

A narrow transport adapter exposing the existing agent event bus as versioned AG-UI-like SSE envelopes. It does not implement the complete AG-UI command or state protocol; events are read-only and payloads are redacted at the boundary before leaving the server.

The bridge is a compatibility surface for clients you operate, not a hosted AG-UI service.

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/ag-ui/events` | AG-UI-like SSE event stream |
| `GET` | `/api/v1/ag-ui/health` | Bridge capabilities/protocol info |

### `GET /api/v1/ag-ui/events`

SSE stream of agent events wrapped in an AG-UI envelope. Supports resuming via the `Last-Event-ID` header or `?last_event_id=<seq>` query parameter; a keep-alive heartbeat is emitted every 15 seconds. A bounded ring (256 events) allows replay from a recent cursor after connection drops.

Each message is a JSON envelope:
```json
{
  "protocol": "fathom.ag-ui",
  "version": "1",
  "event_type": "RUN_STARTED",
  "event_id": "019fd38a-...",
  "timestamp_ms": 1780000000000,
  "data": { "type": "session_started", "id": "...", "..." : "..." }
}
```

**Event types (`event_type` field):**

| `event_type` | Fathom source event |
|--------------|---------------------|
| `RUN_STARTED` | `session_started`, `session_forked` |
| `RUN_FINISHED` | `session_completed` |
| `RUN_ERROR` | `session_failed` |
| `STEP_STARTED` | `agent_spawned` |
| `STEP_FINISHED` | `agent_completed` |
| `STEP_ERROR` | `agent_failed` |
| `TEXT_MESSAGE_CONTENT` | `llm_stream_chunk` |
| `TOOL_CALL_START` | `tool_call_started` |
| `TOOL_CALL_END` | `tool_call_completed` |
| `INTERRUPT` | `question_asked`, `approval_requested` |
| `STATE_DELTA` | `finding`, `agent_state_changed`, `file_change_undone`, `title_generated` |
| `ERROR` | bridge serialization error |

**Redaction:** values under credential-bearing keys (`api_key`, `password`, `secret`, `token`, `authorization`, `private_key`, `credential`, ...) are replaced with `"[REDACTED]"` recursively.

### `GET /api/v1/ag-ui/health`

**Response 200:**
```json
{
  "ok": true,
  "protocol": "fathom.ag-ui",
  "version": "1",
  "transport": "sse",
  "capabilities": { "events": true, "commands": false, "state_mutation": false }
}
```

---

## Jobs API

Background durable jobs — long-running research tasks executed by a fully detached runner process (`fathom job-run`). Jobs survive server restarts because the registry lives in a shared SQLite database. Jobs submitted via HTTP are visible to `fathom jobs list` in the CLI and vice versa. Each job can be retried automatically on failure (configurable attempts, default: 3).

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/v1/jobs` | Submit a job (`task` string, `attempts` optional) |
| `GET` | `/api/v1/jobs` | List all jobs |
| `GET` | `/api/v1/jobs/:id` | Get job status (full ID or unique prefix) |
| `GET` | `/api/v1/jobs/:id/log` | Tail the job log (stdout+stderr of attempts) |
| `DELETE` | `/api/v1/jobs/:id` | Cancel an active job |
| `POST` | `/api/v1/jobs/:id/rerun` | Restart a completed/stuck job |

**Job log:** the `GET /api/v1/jobs/:id/log` endpoint tails the combined stdout+stderr output of all job attempts. Accepts `?lines=<N>` to control how many tail lines to return (default: 100). The log file is stored on disk at the configured jobs root.

**Rerun:** `POST /api/v1/jobs/:id/rerun` resets the job status and spawns a new runner process. Useful for recovering from transient failures or re-executing a completed job with updated parameters.

---

## Full usage example

```bash
# 1. Start the server
fathom serve --port 8080 &

# 2. Create a session
SESSION=$(curl -s -X POST http://localhost:8080/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{"query": "Find contacts of fintech startup CEOs in Dubai"}' \
  | jq -r '.id')

echo "Session: $SESSION"

# 3. Polling status
while true; do
  STATUS=$(curl -s http://localhost:8080/api/v1/sessions/$SESSION | jq -r '.status')
  echo "Status: $STATUS"
  [ "$STATUS" != "running" ] && break
  sleep 5
done

# 4. Get results
curl -s http://localhost:8080/api/v1/sessions/$SESSION/results | jq .

# 5. Monitoring
curl -s http://localhost:8080/metrics | grep pr_tokens
```

---

## Prometheus integration

Add to `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'fathom'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
```

| `pr_request_duration_seconds` | Histogram | Request duration (buckets: 0.005–10 s) |

The metrics are recorded by a middleware that wraps all API routes. Counters and gauges are lock-free atomic integers; the histogram uses fixed cumulative buckets. The `pr_sessions_active` gauge is decremented exactly once per session regardless of cancellation path, using an `AtomicBool` guard in the session's Drop handler.
| `pr_request_duration_seconds` | Histogram | Request duration in buckets (0.1, 1, 10, +Inf) |

The metrics are recorded by a middleware that wraps all API routes. The `sessions_active` gauge is decremented exactly once per session regardless of cancellation path (normal completion, failure, or cancellation), using an `AtomicBool` guard in the session's Drop handler. Metrics are thread-safe lock-free counters and gauges implemented with `AtomicU64` and a histogram backed by `AtomicU64` buckets.

---

## CORS

The API uses restrictive CORS when API-key authentication is disabled. When API keys are configured, the server enables permissive CORS for authenticated clients. Review the middleware in `crates/server/src/lib.rs` before exposing the API cross-origin.