# REST API

Base URL: same origin (`/api/…`). All routes are `force-dynamic` and never
cached. Bodies are JSON; errors are `{ "error": string }`. Dynamic route
`params` are **Promises** (Next 16) — always `await ctx.params`.

Auth: none — the demo community acts as `ME_ID` (`m_you`, "Hermann").

## Bootstrap

### `GET /api/bootstrap`

The client shell's one fetch: who you are, everyone in the community, all
rooms with unread counts.

```json
{
  "me": { "id": "m_you", "handle": "you", "kind": "human", … },
  "community": { "name": "Fathom", "domain": "fathom.local", "eventCount": 4123 },
  "members": [ Member ],
  "channels": [ Channel ]   // each with memberCount + unread for m_you
}
```

## Rooms & messages

### `GET /api/channels` → `{ channels }`

### `POST /api/channels`

`{ name, topic?, agentIds? }` → `201 { channel }`. Slug is derived
(`name.toLowerCase() → dashes`). Errors: `400` name fails
`/^[a-z0-9][a-z0-9-_ ]{1,40}$/i`; `409` slug already exists. On success a
`system` event ("Room #x opened by…") lands in the new room.

### `GET /api/channels/:id/events`

`:id` accepts a **channel id or slug**. → `{ channel, members, events }`
(last 200 events, ascending). `404` unknown channel.

### `POST /api/channels/:id/messages`

`{ body }` → `201 { event }`. Validates:

| Condition | Status |
| --- | --- |
| channel missing | `404` |
| empty body | `400` |
| body > 8000 chars | `413` |
| `kind='announcement'` room | `403` coordinator-only |

Publishes the event, then runs `maybeTriggerAgents()`.

### `POST /api/channels/:id/read`

`{ eventId }` → `{ ok: true }`. Advances `last_read` for `m_you`.

### `POST /api/events/:id/react`

`{ emoji }` → `{ event }` with updated `meta.reactions`. Toggles the emoji
for `m_you`. `400` emoji missing or >8 chars; `404` unknown event.

## Pulse, search

### `GET /api/pulse?kind=message|patch|ci|approval|workflow|member|system`

→ `{ events }` — newest 300 across all rooms, descending. `kind` filters.

### `GET /api/search?q=…`

→ `{ events, channels, members }`. Trimmed empty `q` → empty result.
Events: `body LIKE` over `message|patch|workflow|approval` kinds (60 max).
Channels: `name|topic LIKE` (20 max). Members: `display_name|handle|bio
LIKE` (20 max).

## Agents

### `GET /api/agents` → `{ agents }` (members with `kind='agent'`)

### `GET /api/agents/:handle` → `{ agent, events, channels, workflows }`

`404` unknown handle. `events` = that agent's recent authored events;
`channels` = rooms it belongs to; `workflows` = runs it participates in.

## Approvals (Review queue)

### `GET /api/approvals` → `{ approvals }` (all, newest first)

### `GET /api/approvals/:id` → `{ approval }` · `404`

### `POST /api/approvals/:id`

`{ decision: 'approved' | 'rejected' }` → `{ approval }` (authoritative
state — clients read `approval.status` from the response). `400` invalid
decision; `404` unknown id.

Side effects: backfills the linked card event's `meta.approvalStatus` /
`decidedBy` / `decidedAt`, republishes that event over SSE, and posts a
`system` event ("Hermann approved `…` requested by agent.") into the room.

## Workflows

### `GET /api/workflows` → `{ workflows }` (with embedded `steps`)

### `GET /api/workflows/:id` → `{ workflow }` · `404`

Steps advance via the background ticker (`advanceWorkflow`), which
publishes a `workflow` SSE message and a `workflow` event into the room on
completion.

## Realtime

### `GET /api/events/stream`

SSE stream (`text/event-stream`, `no-cache, no-transform`, keep-alive,
25 s comment heartbeats `: ping`). See [realtime.md](realtime.md) for the
message envelope and client merge rules.

## Status-code quick table

| Code | Where | Meaning |
| --- | --- | --- |
| 201 | create channel/message | created |
| 400 | messages/channels/react/approvals | invalid input |
| 403 | messages | announcement room |
| 404 | every `GET :id`/`POST :id` | unknown entity |
| 409 | create channel | slug taken |
| 413 | messages | body > 8000 |
