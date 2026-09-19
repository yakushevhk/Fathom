# Fathom (Bot/beta)

A full pure-Rust + [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui)
port of [OpenMausBot](https://github.com/milind-soni/OpenMausBot) — a local-first
chat client where every sidebar contact is a real agent process.

Each bot is backed by a real CLI harness (Claude Code, Codex, Fathom, or any
OpenAI-compatible endpoint such as Grok). The embedded harness owns the agent
processes, streams turn events, enforces the permission loop, and persists
everything to a local SQLite store. Transcripts live in `~/.fathombot`
(override with `FATHOM_BOT_DATA`).

## Layout

```
crates/
  fathom-core      types, SQLite store, event bus payloads
  fathom-harness   engine drivers, turn/session machinery, REST+SSE API (127.0.0.1:8799)
  fathom-app       GPUI desktop app (roster, chat, approvals, settings)
```

`fathom-app` runs the harness **in-process**: the UI shares the same
`Arc<AppState>` and subscribes to the broadcast bus directly, while the axum
REST + SSE server stays available on port 8799 for external clients
(a web UI, curl, automation). A standalone `fathombot-harness` binary serves
the same API headless.

## Run

```bash
cd Bot/beta
cargo run -p fathom-app            # desktop app (needs a GPU/Vulkan)
cargo run -p fathom-harness --bin fathombot-harness   # API only
```

Linux dev-deps for GPUI: `pkg-config libssl-dev libxcb1-dev libxcb-xkb-dev
libxkbcommon-dev libxkbcommon-x11-dev libfontconfig-dev libwayland-dev`.

## Engines

| engine           | backing                                                     | config keys |
|------------------|-------------------------------------------------------------|-------------|
| `claude`         | `claude --output-format stream-json` (long-lived process)   | cli, model, auto_approve |
| `codex`          | `codex exec --json`                                         | cli, model, auto_approve |
| `fathom`         | `fathom run` (the Fathom core CLI)                          | cli, model |
| `grok`           | `https://api.x.ai/v1` chat completions (SSE)                | api_key_env=`XAI_API_KEY`, model |
| `open-ai-compat` | any OpenAI-compatible endpoint                              | api_key_env=`OPENAI_API_KEY`, url, model |

Engine status is probed at startup and surfaced via `GET /api/engines`
(available binary path, version, why-unavailable reason). Custom binary paths,
base URLs and API-key env vars are editable in the in-app Settings dialog and
persisted in the store (`engines.<kind>` kv rows).

## Permissions

Per-bot `approval_mode` mirrors OpenMausBot's five modes
(`ask`/`edits`/`auto`/`full`/`custom`):

- claude: `ask`/`custom` → interactive prompts; `edits`/`auto` →
  `--permission-mode acceptEdits`; `full` → `--dangerously-skip-permissions`.
- codex: `full` → `danger-full-access`, anything else → `workspace-write`.
- `auto_approve` (legacy bool) is still accepted on PATCH and folds into
  `approval_mode=auto`.

When a permission prompt surfaces, a `PermissionRequest` parks a oneshot in the
harness `decisions` map. `POST /api/approvals/{id}` with
`{"decision":"allow"|"always"|"deny"|"<option>"}` resolves it; `always`
adds the tool to the bot's `always_allow` list so future prompts auto-grant.
Structured question cards (`options[]`) return the chosen option as the answer.

Mid-turn **steering** works for stdin-driven sessions (claude): sending while a
turn is running injects the message into the live session when `park_dms=false`;
otherwise sends queue per thread and drain in order after the turn completes.

## API surface

```
GET  /api/health
GET/PATCH /api/config                profile_name, analytics_enabled
GET  /api/events                     SSE stream of ServerEvent
GET  /api/search?q=                  bots + rooms + message text search

GET/POST /api/bots                   list / create {name, engine, model?, soul?, cwd?, title?}
GET/PATCH/DELETE /api/bots/{id}      all bot fields incl. approval_mode,
                                   always_allow, effort, pinned, hidden,
                                   section, notifications, park_dms, avatar
GET  /api/bots?archived=1            archived bots
POST /api/bots/{id}/read             mark read (clears unread badge)
GET/PUT /api/bots/{id}/soul          persona (SOUL.md, written to <data>/souls/)
GET  /api/bots/{id}/thread           direct thread for a bot
GET/POST /api/bots/{id}/threads      task threads list / create

GET/PATCH/DELETE /api/threads/{id}   read / {title?, pinned_message_id?, cwd?,
                                   archived_at?} / delete
GET/POST /api/threads/{id}/messages  history / send {text, model?, reply_to?,
                                   attachments?[]id, sender?, via_api?,
                                   send_id? (dedupe), channel_mode?}
                                   — returns {user, pending} | {user, queued}
                                   | {user, deduped:true}
POST /api/threads/{id}/stop          abort running turns (incl. room members)
GET  /api/threads/{id}/export        markdown transcript download

PATCH/DELETE /api/messages/{id}      edit text / delete
POST /api/messages/{id}/reactions    toggle reaction {emoji, by}

POST /api/attachments                raw bytes + x-file-name / content-type
GET  /api/attachments/{id}           serve stored blob

GET/POST /api/rooms                  rooms list / create {name, member_ids,
                                   responder, bulletin?, section?, cwd?, dm?}
GET/PATCH/DELETE /api/rooms/{id}     responder is serde-tagged:
                                   {"kind":"member","bot_id":"…"} |
                                   {"kind":"everyone"} | {"kind":"mentions"}
                                   also: thread_id (task switch), bulletin,
                                   section, cwd, setup_completed/skipped
GET/POST /api/rooms/{id}/threads     task threads inside a room
POST /api/rooms/{id}/read            clear room unread
/api/groups*                         wire-name aliases for /api/rooms*

GET  /api/approvals                  pending approvals
POST /api/approvals/{id}             resolve {decision, reason?}
GET  /api/decisions                  recent resolved approvals (audit log)
GET  /api/sidebar-sections           distinct bot section labels
GET  /api/engines                    probe status + model catalog + config
PUT  /api/engines/{kind}             persist engine config
GET  /api/models[?engine=..]         model catalog
POST /api/cli-test                   probe a CLI engine ({engine} → {installed})
GET  /api/cli-candidates             resolved CLI path per engine
GET  /api/usage + /api/usage.csv     per-bot message/reply counters
GET  /api/files[?path=]              directory listing for the cwd picker
GET/POST/DELETE /api/mcp/servers     app-wide MCP server registry (kv)
GET  /api/computers                  companion computers (stub — returns [])

POST /api/internal/ask-bot           engine tool: synchronous peer question
POST /api/internal/delegate-bot      engine tool: spawn a task on a peer
POST /api/internal/post-to-room      engine tool: post into a room as a bot
POST /api/internal/coordinate-bots   engine tool: spin up a room + directive
                                     (all gated by bot.approve_peer_comms and
                                      restricted by bot.peers)
```

All mutations publish `ServerEvent`s on the broadcast bus (`/api/events`):
`message_upsert`, `message_deleted`, `bot_upsert`, `room_upsert`,
`thread_upsert`, `approval_upsert`, `engines`, `turn_error`,
`queued_messages`, `bot_deleted`, `room_deleted`, `notify`.

## UI

Dark sidebar roster (avatar, engine badge, last message, unread + waiting-on-
you indicators, pinned ordering, custom sections, Rooms group, archived panel),
chat pane with user/bot bubbles, `from_bot` + sender attribution for rooms,
streamed thinking segments, tool-call cards, approval cards (allow / always /
deny / option answers), flat reply quotes, reactions, message pin/delete,
queued & steered chips, per-bot task picker (multi-thread), approval-mode
selector, effort selector, model picker, attachment staging + inline images,
global search, per-thread drafts, markdown export, archived-bot restore, and a
stubbed computer panel for remote-companion bots.

## Parity vs OpenMausBot

Ported and verified end-to-end (`/api/*` exercised via curl):

| OpenMausBot | Bot/beta |
|---|---|
| approval modes ask/edits/auto/full/custom | `ApprovalMode` per bot, cycled in header/profile |
| `autoApprove` legacy flag | folded into `approval_mode=auto` (accepts both on PATCH) |
| always-allow list | `always_allow[]`, granted via `decision:"always"`, removable chips in profile |
| permission/option question cards | `ApprovalRequest.options[]`, answered via decision=option label |
| multi-thread tasks per bot | `bot.threads`, tasks dropdown + create/rename/delete |
| rooms (multi-bot group chat) | `Room` type, `member|everyone|mentions` responders, `from_bot` attribution |
| attachments | `attachments` table + blobs dir, upload/serve, inline image render |
| reactions | `reactions[]` toggle per (emoji, by) |
| reply-to / pin / delete / edit | `reply_to`, `Thread.pinned_message_id`, PATCH/DELETE routes |
| steer + send queue | per-thread queue; mid-turn stdin injection (claude) or parked queue |
| unread / waiting-on-you | unread counters on bots + rooms, `waiting_on_you` from pending approvals |
| search | `/api/search?q=` across bots, rooms, message text |
| export | `GET /api/threads/{id}/export` markdown |
| drafts | per-thread composer drafts in kv store |
| profile name / analytics opt-in | `GET/PATCH /api/config` |
| effort levels none..max | `EFFORT_LEVELS`, passed to engines (`model_reasoning_effort` on codex) |
| pinned/hidden/section roster metadata | stored + rendered (sections group, hidden → archive filter) |
| send_id dedupe / auto-title | `find_by_send_id` short-circuit; `title_from_first_message` |
| thread cwd / archive / rewind flags | `Thread.{cwd, archived_at, rewound, turn_started_at}` in `extra` |
| room tasks + bulletin + room cwd | `room_threads`, `Room.{bulletin, section, cwd, busy_bot_id}` |
| wire "groups" naming | `/api/groups*` alias routes on top of `/api/rooms*` |
| peer comms (ask / delegate / post / coordinate) | `/api/internal/*` endpoints + `approve_peer_comms` approval card + `peers` allow-list |
| decisions log / sidebar sections | `GET /api/decisions`, `GET /api/sidebar-sections` |
| usage counters | `GET /api/usage` + `.csv` export |
| cwd file picker | `GET /api/files` directory listing |
| MCP server registry | `GET/POST/DELETE /api/mcp/servers` (kv), `bot.mcp_servers` mount list |
| CLI probe/candidates | `POST /api/cli-test`, `GET /api/cli-candidates` |
| notification toasts | `ServerEvent::Notify` → in-app toast overlay |
| wire parity fields | `projects`, `model_variant`, `soul_hash/drift`, `mascot_*`, `avatar_crop`, `computer`, `cloud_backend`, `auto_start_vps`, `speak_replies`, `voice`, `rewound`, `chief_of_staff`, `managed_sections`, `composio`, `browser`, `browser_profile`, `playbooks`, `approve_peer_comms`, `peers`, `pinned_message_id` |

Intentionally out of scope for `beta` (OpenMausBot infra scale):
fleet/teams management, cloud computers + VPS provisioning, browser engine
execution, live TTS/voice calls, live Composio connectors, phone pairing,
routines/goals scheduler, webhooks, auth tokens, mobile apps, Cloudflare
relay, Electron shell (GPUI replaces it). The wire fields (`computer`,
`speak_replies`, `voice`, `composio`, `browser`, `projects`, `playbooks`, …)
are already persisted/patched, so filling in the runtimes is additive.
