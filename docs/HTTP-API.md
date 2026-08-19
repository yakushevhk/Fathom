# HTTP API

Fathom provides a REST API built on **Axum** for programmatic control of research. The API exposes session lifecycle management, agent inspection, live event streaming, operator-in-the-loop controls (steering, question/answer, tool-call approval), a long-term semantic memory store, and a durable background jobs subsystem. All endpoints under `/api/v1` are protected by authentication and rate limiting when configured.

Start:
```bash
fathom serve --port 8080
```

Base URL: `http://localhost:8080`

---

## Authentication

If `FATHOM_API_KEYS` (comma-separated list of keys) is set, all `/api/v1/*` requests require an API key. When multiple keys are configured, each key is registered with a human-readable name derived from a hash — the auth middleware validates the key, extracts the principal name, and attaches it to the request for rate-limiting and logging. Public endpoints (`/health`, `/metrics`, `/dashboard`) are not protected, but the dashboard's data fetches all go through the protected `/api/v1` endpoints.

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

### `GET /metrics`

Prometheus metrics in text exposition format. Exposes counters and histograms for session lifecycle, agent spawning, token usage, tool calls, and HTTP request timing. This endpoint is outside the `/api/v1` namespace and is not rate-limited, making it suitable for Prometheus scrapers.

**Response 200** (text/plain, Prometheus format):
```
pr_sessions_total 42
pr_sessions_active 2
pr_agents_spawned_total 128
pr_tokens_used_total 15234567
pr_tool_calls_total 890
pr_http_requests_total 234
pr_request_duration_seconds_bucket{le="0.1"} 100
pr_request_duration_seconds_bucket{le="1"} 200
...
```

---

### `GET /dashboard`

Built-in live dashboard (single HTML file, no build step): session and job tables, agent tree, memory panel, and a live event feed via `EventSource` (`GET /api/v1/events`). Data is polled every 5 seconds. If authorization is enabled (`FATHOM_API_KEYS`), enter the key in the header field — it is saved in localStorage and injected into `X-Api-Key`. The route lies outside `/api/v1`, the page itself is not authorized; all data is fetched only through protected endpoints.

The dashboard provides a one-stop visual overview of the entire research cluster: which sessions are running/completed/failed, the agent tree for each session (depth, role, status, tokens consumed), a live event log that streams in real time, memory store contents, and the durable jobs queue with status and logs.

---

### `POST /api/v1/sessions`

Create a new research session. The server spawns a coordinator that manages the agent tree, persists progress to SQLite, and streams events to subscribers. The session runs asynchronously — the response returns immediately with the session ID and status.

**Request:**
```json
{
  "query": "Research the AI agent market",
  "api_key": "sk-optional-key"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `query` | string | ✅ | Research query (non-empty) |
| `api_key` | string | ❌ | Override LLM key for this session |
| `output_dir` | string | ❌ | Single relative directory name override (sanitized: no path separators, no `..`, no absolute paths) |

The `output_dir` field is validated to prevent directory traversal: it must be a single relative directory name with no slashes, backslashes, or `..` components. The server creates `<output_dir>/<session_id>` as the working directory. If omitted, the configured default output directory is used.

**Response 201:**
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
- `500` — LLM api_key not configured, or filesystem error creating the output directory

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

Available metrics:
| Metric | Type | Description |
|--------|------|-------------|
| `pr_sessions_total` | Counter | Total sessions created |
| `pr_sessions_active` | Gauge | Currently active sessions |
| `pr_agents_spawned_total` | Counter | Total agents spawned |
| `pr_tokens_used_total` | Counter | Total tokens used |
| `pr_tool_calls_total` | Counter | Total tool calls |
| `pr_http_requests_total` | Counter | HTTP requests to the API |
| `pr_request_duration_seconds` | Histogram | Request duration in buckets (0.1, 1, 10, +Inf) |

The metrics are recorded by a middleware that wraps all API routes. The `sessions_active` gauge is decremented exactly once per session regardless of cancellation path (normal completion, failure, or cancellation), using an `AtomicBool` guard in the session's Drop handler. Metrics are thread-safe lock-free counters and gauges implemented with `AtomicU64` and a histogram backed by `AtomicU64` buckets.

---

## CORS

The API includes CORS middleware (all origins allowed by default). For production, configure allowed origins in code (`crates/server/src/lib.rs`).