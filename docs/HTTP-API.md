# HTTP API

Parallel Research provides a REST API built on **Axum** for programmatic control of research.

Start:
```bash
parallel-research serve --port 8080
```

Base URL: `http://localhost:8080`

---

## Authentication

If `PARALLEL_RESEARCH_API_KEYS` (comma-separated list of keys) is set, all `/api/v1/*` requests require an API key:

```bash
# Bearer token
curl -H "Authorization: Bearer your-key" http://localhost:8080/api/v1/sessions

# Or X-Api-Key header
curl -H "X-Api-Key: your-key" http://localhost:8080/api/v1/sessions
```

If the variable is not set — access is open (for development).

**Rate limiting**: sliding-window request limit per client. On exceeding — `429 Too Many Requests`.

---

## Endpoints

### `GET /health`

Health check.

**Response 200:**
```json
{
  "status": "ok",
  "active_sessions": 2
}
```

---

### `GET /metrics`

Prometheus metrics.

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

Built-in live dashboard (single HTML file, no build step): session and job
tables, agent tree, memory panel, and a live event feed via
`EventSource` (`GET /api/v1/events`). Data is polled every 5 seconds.
If authorization is enabled (`PARALLEL_RESEARCH_API_KEYS`), enter the key
in the header field — it is saved in localStorage and injected into
`X-Api-Key`. The route lies outside `/api/v1`, the page itself is not
authorized; all data is fetched only through protected endpoints.

---

### `POST /api/v1/sessions`

Create a new research session.

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

**Response 201:**
```json
{
  "id": "019fd38a-9a7c-7322-a671-64427832f0eb",
  "status": "running",
  "query": "Research the AI agent market"
}
```

**Errors:**
- `400` — empty query
- `500` — LLM api_key not configured

---

### `GET /api/v1/sessions`

List all sessions.

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

`active` — whether the session is currently executing (in-process).

**Errors:**
- `404` — session not found

---

### `GET /api/v1/sessions/:id/results`

Results of a completed session.

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
- `409` — session not yet completed

---

### `DELETE /api/v1/sessions/:id`

Cancel a running session.

**Response 200:**
```json
{
  "id": "019fd38a-...",
  "status": "cancelled"
}
```

Idempotent: re-cancelling a completed session is not an error.

**Errors:**
- `404` — session not found

---

### `GET /api/v1/agents`

List all agents (all sessions).

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

Inject an instruction into a running session (mid-run steering).

**Request:**
```json
{ "message": "Focus only on companies from Kazan" }
```

**Response 202:**
```json
{ "id": "019fd38a-...", "steered": true, "message": "..." }
```

---

### `POST /api/v1/sessions/:id/answer`

Answer an agent's question (`question` tool) waiting for an operator.
`request_id` is taken from the `question_asked` event.

**Request:**
```json
{ "request_id": "019fe7ad-...", "text": "Work on the EU region" }
```

**Response 200:**
```json
{ "answered": true, "request_id": "019fe7ad-..." }
```

**Errors:** `404` — no such request, `400` — this is an approval (use `/approve`), `410` — agent is no longer waiting.

---

### `POST /api/v1/sessions/:id/approve`

Approve/deny a side-effect tool call (`approval_tools`).
`request_id` is taken from the `approval_requested` event.

**Request:**
```json
{ "request_id": "019fe7ad-...", "approved": true }
```

**Response 200:**
```json
{ "approved": true, "request_id": "019fe7ad-..." }
```

---

### `GET /api/v1/sessions/:id/events` / `GET /api/v1/events`

SSE stream of agent events (first — filtered by session, second — global).

---

## Memory API

Long-term semantic memory (see [MEMORY-KB.md](MEMORY-KB.md)).
Available when `[memory] enabled = true`.

### `GET /api/v1/memories`

List or search. Without `q` — list (filters `scope`, `scope_key`, `status`, `limit`),
with `q` — hybrid search (`top_k`).

**Response 200:**
```json
{ "memories": [
  { "id": "019fe796-...", "content": "...", "scope": "agent",
    "status": "active", "confidence": 0.9, "importance": 1.0,
    "tags": ["contact"], "created_at": "...", "score": 0.83 }
] }
```

### `POST /api/v1/memories/absorb`

Save facts via the absorb pipeline. Body — `AbsorbRequest`
(`facts[]`, `source`, `scope`, `scope_key`, `dry_run`).

### `GET /api/v1/memories/stats`

Counters by scope/status, embedding model, entity graph size.

### `POST /api/v1/memories/distill`

Distill run facts into persistent knowledge (`?session=<key>&dry_run=true|false`).

### `POST /api/v1/memories/gc`

Storage GC (`?ttl_days=<N>&dry_run=true|false`): archives expired
(`expires_at`) and stale untouched run facts, compacts overgrown
scope groups N→1. Returns `{expired_archived, stale_archived,
groups_compacted, facts_compacted, errors, dry_run}`. Nothing is deleted —
archive is accessible via `?follow=full_history`.

### `GET /api/v1/memories/:id`

Single record; `?follow=latest|full_history` traverses the version chain.

### `DELETE /api/v1/memories/:id`

Archive (soft delete; history is preserved).

---

## Jobs API

Background durable jobs (`parallel-research jobs ...` in CLI).

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/v1/jobs` | Submit a job (`task`, `attempts`) |
| `GET` | `/api/v1/jobs` | List jobs |
| `GET` | `/api/v1/jobs/:id` | Job status |
| `GET` | `/api/v1/jobs/:id/log` | Log (stdout+stderr of attempts) |
| `DELETE` | `/api/v1/jobs/:id` | Cancel an active job |
| `POST` | `/api/v1/jobs/:id/rerun` | Restart a completed/stuck job |

---

## Full usage example

```bash
# 1. Start the server
parallel-research serve --port 8080 &

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
  - job_name: 'parallel-research'
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
| `pr_request_duration_seconds` | Histogram | Request duration |

---

## CORS

The API includes CORS middleware (all origins allowed by default). For production, configure allowed origins in code (`crates/server/src/lib.rs`).