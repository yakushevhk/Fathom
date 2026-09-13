---
name: fathom-bot
description: Architecture, component hierarchy, IPC, and state management for Fathom Bot desktop client and harness server in Bot/. Use when modifying Bot/ components, server, settings dialogs, live events, or Electron integration.
---

# Fathom Bot Client & Desktop Harness

Fathom Bot is the local-first desktop application and team conversational surface for interacting with sovereign autonomous coworkers.

## 1. Directory Layout (`Bot/`)

```
Bot/
├── server/
│   └── index.ts          # Node.js 24 harness server listening on 127.0.0.1:8799
├── src/
│   ├── components/
│   │   ├── ChatView.tsx              # Main message stream, virtualized scroll, cards
│   │   ├── BotSettingsDialog.tsx     # SOUL.md tuning, persona setup, model selector
│   │   ├── UniversalCard.tsx         # Interactive action cards (status, micro-graphs, widgets)
│   │   ├── ThinkingAccordion.tsx     # Chain-of-thought streaming drawer
│   │   └── Sidebar.tsx               # Contact list (digital coworkers) & channel switcher
│   ├── lib/
│   │   ├── live-events.ts            # SSE event bus listener, heartbeats, reconnect backoff
│   │   ├── analytics.ts              # Strict opt-in telemetry (default disabled)
│   │   └── api.ts                    # REST endpoints to harness server (127.0.0.1:8799)
│   └── App.tsx                       # Main application shell and window routing
├── electron/                         # Native Electron 43 wrappers & window chrome
└── package.json                      # React 19.1, Tailwind v4, Electron 43
```

## 2. Key Architectural Invariants

* **Sovereignty & Zero Telemetry**:
  * `analyticsEnabled()` in `Bot/src/lib/analytics.ts` MUST default to `false`.
  * Telemetry is strictly opt-in (`fathom-analytics-opt-in = "true"`).
  * No external network requests to `posthog.com` or third parties on launch.
* **Ports**:
  * Bot Harness server: `127.0.0.1:8799`.
  * Fathom Core compiled daemon: `127.0.0.1:8080`.
  * NEVER map Bot Caddy/Docker to port `8080` to avoid socket collision with Fathom Core.
* **Agent Personalities & Instructions (`SOUL.md`)**:
  * Every bot contact can have its own `SOUL.md` stored in local state.
  * Personas, custom instructions, and persistent context are configured in `BotSettingsDialog.tsx`.
* **Universal Action Cards**:
  * Deliverables, tables, status monitors, and interactive buttons render via `UniversalCard.tsx` directly in the chat stream.

## 3. Development Workflow

```bash
cd Bot
pnpm install
pnpm dev              # Launch Vite dev server + Electron client
pnpm build            # Compile desktop binaries
```
