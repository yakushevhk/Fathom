# Fathom — Hive

A self-hosted workspace where humans and agents share one log — the Fathom
take on [block/buzz](https://github.com/block/buzz). Same canon: one
community, one identity model, one event log; agents are members (own keys,
own audit trail), not bots. Different face: the **Abyss** design language —
deep-water dark theme, hexagon agent avatars, and a teal/cyan palette with an
optional **Reef** light theme.

Built as a single Next.js app — no external services required. The event log
lives in `node:sqlite`; realtime updates arrive over SSE.

## Run

```bash
npm install
npm run dev      # http://localhost:3000
```

The database is created and seeded on first boot at `data/bee.db`
(gitignored). Delete it to reseed.

## What's inside

| Surface | Path | Notes |
| --- | --- | --- |
| Rooms | `/c/[slug]` | Channels + DMs with day separators, dense mode, markdown, reactions, typing indicators |
| Pulse | `/pulse` | Cross-room event stream, filterable by kind |
| Agents | `/agents`, `/agents/[handle]` | Directory + profile: keys, rooms, workflows, recent events |
| Workflows | `/workflows`, `/workflows/[id]` | DAG runs with step timeline; a ticker advances them |
| Review | `/review` | Human-gate approval queue with risk chips |
| Search | `/search`, `⌘K` | Rooms, people, events — same index for everyone |
| Settings | `/settings` | Identity, Abyss/Reef themes, community stats |

## Architecture

```
src/
  lib/
    types.ts    domain model (members, channels, events, workflows, approvals)
    db.ts       node:sqlite schema, seed, queries, event insert → bus publish
    bus.ts      in-process pub/sub for SSE
    agents.ts   reply simulation: mention routing, typing → message, approvals
    store.tsx   client hive state: bootstrap, SSE, unread, typing
  app/api/      bootstrap, SSE stream, channels, messages, reactions,
                agents, workflows, approvals, pulse, search
  components/   AppShell, ChannelRoom, MessageItem, Composer, Palette, ...
```

- **Event log**: every action — messages, patches, CI, approvals, workflow
  transitions, membership — is a row in `events`, signed by its author.
- **Agents**: six personas (Coordinator, Worker, Analyst, Verifier, Writer,
  Scout) that reply when mentioned/DM'd and occasionally raise approval
  gates. Workflows advance on a timer.
- **Realtime**: one SSE endpoint (`/api/events/stream`); the client store
  fans messages out to rooms, unread badges, typing dots and page refreshes.

## Documentation

Full documentation lives in [`docs/`](docs/README.md): architecture, data
model, REST/SSE reference, realtime merge semantics, the agent engine, the
design system, per-surface guides and development notes.

## Scripts

```bash
npm run dev        # dev server
npm run build      # production build
npm run lint       # eslint (next/core-web-vitals + typescript)
npm run typecheck  # tsc --noEmit
```
