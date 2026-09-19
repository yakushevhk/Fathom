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

Claude turns run with `--permission-prompt-tool stdio`: when the engine asks
for approval, a `PermissionRequest` turn event parks a oneshot in the harness
`decisions` map. The UI shows an inline approval card; `POST /api/approvals/{id}`
with `{"decision":"allow"|"deny"}` resolves it. `auto_approve` per-bot or
per-engine skips the prompt entirely (`--dangerously-skip-permissions`).

## API surface

```
GET  /api/health
GET  /api/config
GET  /api/events                     SSE stream of ServerEvent
GET/POST /api/bots                   list / create {name, engine, model?, soul?, cwd?}
PATCH/DELETE /api/bots/{id}
POST /api/bots/{id}/read             mark read (clears unread badge)
GET/PUT /api/bots/{id}/soul          persona (SOUL.md, written to <data>/souls/)
GET  /api/bots/{id}/thread           direct thread for a bot
GET/POST /api/threads/{id}/messages  history / send {text, model?} — starts a turn
POST /api/threads/{id}/stop          abort the running turn
GET  /api/approvals                  pending approvals
POST /api/approvals/{id}             resolve {decision: allow|deny|answer}
GET  /api/engines                    probe status + model catalog + config
PUT  /api/engines/{kind}             persist engine config
GET  /api/models[?engine=..]         model catalog
GET  /api/computers                  companion computers (stub — returns [])
```

All mutations publish `ServerEvent`s on the broadcast bus (`/api/events`):
`message_upsert`, `bot_upsert`, `approval_upsert`, `thread_upsert`, `engines`,
`turn_error`.

## UI

Dark sidebar roster (avatar, engine, last message, unread badge, working
spinner), chat pane with user/bot bubbles, streamed thinking segments,
tool-call cards, approval cards, markdown-lite code blocks, model picker per
bot, new-bot dialog (name/engine/model/persona/cwd), per-bot profile editor
(SOUL.md), settings dialog (per-engine binary/URL/key env/auto-approve), and a
stubbed computer panel for remote-companion bots.

## Status vs OpenMausBot

Ported: roster, chat, streaming segments (text/thinking/tool use), approval
loop, personas (SOUL.md), per-bot model+cwd, unread badges, engine probing and
config, standalone harness server, SSE for external clients.

Not yet: companion iOS/Android app, Cloudflare remote relay, room threads
(multi-bot), and the Electron shell — GPUI replaces it natively.
