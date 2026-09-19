---
name: testing-fathom-bot
description: How to run and end-to-end test the Fathom Bot beta GPUI desktop app + standalone harness (Bot/beta) on a headless/no-GPU VM, including the nested wlroots/cage workaround, its repaint/input quirks, and the REST fallback via the embedded 127.0.0.1:8799 API.
---

# Testing the Fathom Bot beta (Bot/beta)

Binaries: `Bot/beta/target/debug/fathombot` (GPUI app, embeds harness in-process on 127.0.0.1:8799) and `fathombot-harness` (standalone axum REST+SSE server, same API). Build once: `cd Bot/beta && cargo build`.

## Data isolation (required by Bot/AGENTS.md)
Always set `FATHOM_BOT_DATA=/tmp/fathom-test-<ts>` (fresh dir), never `~/.fathombot`. The dir gets `fathom.db` (SQLite), `souls/`, `files/`. Note: `docs/verification/README.md` and the `verify-omb` skill describe the old pnpm/OpenMausBot tooling — not applicable to the Rust beta; apply the isolation principle manually.

## Standalone harness (shell-only, fastest verification)
```
cd Bot/beta && FATHOM_BOT_DATA=$D ./target/debug/fathombot-harness &   # serves 127.0.0.1:8799
curl -sN 127.0.0.1:8799/api/events > $D/sse.log &                     # capture SSE
```
Key routes: GET /api/health, /api/engines, /api/config, /api/models, GET/POST /api/bots, PATCH/DELETE /api/bots/{id}, GET /api/bots/{id}/thread, GET/POST /api/threads/{id}/messages, POST /api/threads/{id}/stop, GET /api/approvals, PUT /api/engines/{kind} (returns 204 empty body — verify via GET /api/engines). On a box with no engine CLIs/API keys, send-message produces SSE sequence `bot_upsert → message_upsert(user) → message_upsert(bot pending) → bot_upsert → message_upsert(bot error) → turn_error` with error "fathom binary not found — build the core workspace" (graceful failure = correct). Edge cases: bad engine/empty body → 422, unknown id/route → 404, DELETE is idempotent 204.

## GPUI app under nested compositor (no GPU)
Direct X11 run opens but never paints. Working recipe:
```
mkdir -p /tmp/xdg-ubuntu && chmod 700 /tmp/xdg-ubuntu
DISPLAY=:0 XDG_RUNTIME_DIR=/tmp/xdg-ubuntu WLR_BACKENDS=x11 WLR_RENDERER=pixman \
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json FATHOM_BOT_DATA=$D \
  cage -- ./target/debug/fathombot &
```
App renders inside an X11 window titled "wlroots - X11-1". Focus with `wmctrl -ia <id>`.

### Quirks (verified on this VM)
- **Stale frames**: the window does NOT repaint after state changes (clicks, SSE-driven updates). Actions DO register; force a redraw by nudging the window size: `wmctrl -ir <id> -e 0,180,120,651,510` (change width by 1px each time). Always re-screenshot after a nudge before concluding a click failed.
- **Flaky typing**: multi-char `type` may drop leading characters ("UI-Scout" → "-cout"). Click the field, `ctrl+a`/`Delete`, retype, nudge+screenshot to confirm; or drive via REST and verify the UI reflects it.
- **Kill cleanly**: `pkill -x fathombot` then `pkill -x cage` — never loose `pkill -f` (matches your own wrapper shell).
- `pgrep -a fathombot-harness` may show nothing (process name truncates to "fathombot-harne"); use `ss -tlnp | grep 8799` or `pgrep -f`.
- **Stale-instance trap**: if :8799 is already bound, a new harness fails with EADDRINUSE and your curls silently hit the OLD process with a DIFFERENT data dir. Always `ss -tlnp | grep 8799` before launching and verify `GET /api/config` returns YOUR `data_dir`.
- **Composer input position**: the Message field sits at the very bottom of the window — at ~930x620 it's around y=428, not mid-window. Clicking empty chat area does NOT focus it.
- **Missing glyph font**: emoji icons (📌🗄🖥☺🗑 etc.) render as empty boxes — count positions, don't hunt for shapes. Zoom the header/action row to locate them; tiny icons (~8px) are hard to hit — enlarge the window first.
- **img element never paints under pixman** — inline image attachments show nothing (data is correct via REST; mark env-limited, not a bug).
- Search dialog retains the last query between opens; Enter triggers `Submitted` only when the field is focused.

## UI structure (crates/fathom-app/src/views.rs)
Sidebar (250px): "F fathom" header + "+" (New bot dialog); bot rows = colored avatar (initial from `avatar_seed`) + name + engine label/last message. Header: avatar + name + "Engine · model" + "model ▾" + stop/edit buttons. Composer: "Message" input + Send + ⏎. Settings gear bottom-left → engine config dialog (persists via PUT /api/engines/{kind}). Error messages render as red-bordered bubbles; user bubbles right-aligned blue.

## Minimal e2e pass
1. Harness: health, engines, create bot "Sentry" (engine=fathom), get thread, POST "ping" → verify SSE + error message.
2. Launch app with same `$D`: bot appears, auto-selected, ping + error bubbles render.
3. POST another message via REST while app runs → nudge resize → new bubbles appear (proves broadcast pump; repaint quirk is env, not data).
4. Click "+" → New bot dialog; create via UI if input cooperates, else REST.
5. Gear → Settings shows persisted engine config.
6. `pkill -x fathombot`, relaunch same `$D` → bots + messages reload (SQLite persistence).

## Devin Secrets Needed
None — engine CLIs/API keys intentionally absent; graceful failure is the expected behavior.
