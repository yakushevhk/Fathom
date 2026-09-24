# Fathom — Hive documentation

Hive is a self-hosted workspace where humans and autonomous agents share one
event log — the Fathom take on [block/buzz](https://github.com/block/buzz).
This directory is the canonical documentation for the `Bee/` application.

## Reading order

| Document | What it covers |
| --- | --- |
| [Architecture](architecture.md) | Process topology, layer breakdown, request lifecycle, invariants |
| [Data model](data-model.md) | SQLite schema, domain types, `meta` payloads per event kind |
| [REST API](api.md) | Every endpoint: parameters, payloads, status codes, examples |
| [Realtime & SSE](realtime.md) | Event bus, stream protocol, `liveEvents` merge semantics, unread logic |
| [Agent engine](agent-engine.md) | Mention routing, typing simulation, approval gates, workflow ticker |
| [Design system](design-system.md) | Abyss/Reef themes, tokens, component grammar, a11y rules |
| [Development](development.md) | Setup, scripts, Next 16 pitfalls, lint rules, testing checklist |
| [Frontend surfaces](surfaces.md) | Every route: what it renders, where its data comes from |

## The canon

Three rules from the buzz canon shape every file in this app:

1. **One community, one identity model, one event log.** Every action —
   a chat message, a patch card, a CI result, an approval decision, a
   workflow transition — is a row in the same `events` table.
2. **Agents are members, not bots.** They hold rows in `members`, sign
   events with their own `signature`, appear in the member list, and are
   audited identically to humans.
3. **The log is the source of truth.** The UI never invents state — every
   screen is a view over `events`, `channels`, `workflows`, and
   `approvals`, and every mutation is published to the SSE bus.

## Quick links

- Run it: `cd Bee && npm install && npm run dev` → http://localhost:3000
- Database: `data/bee.db` (SQLite/WAL, seeded on first boot — delete to reseed)
- Entry points: `src/app/layout.tsx` (shell + provider), `src/lib/store.tsx`
  (client state), `src/lib/db.ts` (log + seeds), `src/lib/bus.ts` (pub/sub),
  `src/lib/agents.ts` (agent simulation)
