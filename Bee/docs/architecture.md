# Architecture

Hive is deliberately a single-process app: one Next.js server owns the
SQLite log, the pub/sub bus, the agent simulation, and the HTTP+SSE surface.
There are no external services, no build-time env requirements, and no
network dependencies — the whole workspace runs from `npm run dev`.

## Process topology

```
┌────────────────────────────── Next.js process ──────────────────────────────┐
│                                                                             │
│   Browser(s)                    Route handlers (src/app/api/**)             │
│      │                              │                                       │
│      │  GET /api/events/stream      │  all mutations                        │
│      │  (SSE, keep-alive)           ▼                                       │
│      │                        ┌──────────┐   publish()   ┌────────────┐     │
│      └───────────────────────▶│  bus.ts  │◀──────────────│   db.ts    │     │
│                               │  Set<sub>│               │ node:sqlite│     │
│                               └──────────┘               │ bee.db WAL │     │
│                                     ▲   publish()        └────────────┘     │
│                                     │                          ▲            │
│                               ┌──────────┐   insertEvent() ────┘            │
│                               │agents.ts │                                │
│                               │ replies, │   setTimeout chains            │
│                               │ ticker   │                                │
│                               └──────────┘                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

Every write path — a human posting a message, an agent replying, the ticker
advancing a workflow, a human deciding an approval — ends in `db.ts`, which
inserts into `events`/`workflows`/`approvals` **and** calls `publish()` on
the in-process bus. The SSE route is just a fan-out adapter over the bus.

## Layers

| Layer | File(s) | Responsibility |
| --- | --- | --- |
| Domain types | `src/lib/types.ts` | `Member`, `Channel`, `HiveEvent`, `Workflow`, `Approval`, `Bootstrap` — the wire contract every route returns and every component reads |
| Log | `src/lib/db.ts` | `node:sqlite` (WAL). Schema, row mappers, queries, mutations, seed. `getDb()` is a `globalThis` singleton — survives Turbopack HMR and is shared across route handlers |
| Bus | `src/lib/bus.ts` | `publish()`/`subscribe()` over a `globalThis`-held `Set<Subscriber>`. Five message types: `event`, `typing`, `workflow`, `approval`, `presence` |
| Agent engine | `src/lib/agents.ts` | `maybeTriggerAgents()` after each user message; `startTicker()` advances workflows every ~18 s. In-memory `setTimeout` chains — not persisted |
| REST + SSE | `src/app/api/**` | Thin validation over `db.ts`; every mutation republishes. `events/stream` is the single SSE endpoint |
| Client state | `src/lib/store.tsx` | `HiveProvider`: bootstrap fetch, `EventSource`, `liveEvents` queue, `typing` map, `liveVersion` counter, unread bookkeeping, `markRead` |
| UI | `src/app/**`, `src/components/**` | Server components fetch; client components render views over `events` + `liveEvents` |

## Request lifecycle (posting a message)

1. `POST /api/channels/:id/messages` validates body (non-empty, ≤8000 chars,
   channel exists, not an `announcement` room).
2. `insertEvent()` writes the row and publishes `{type:'event', data:ev}`.
3. Every SSE subscriber serializes it into `data: {...}\n\n`.
4. `HiveProvider` appends it to `liveEvents` (deduped by `id`, capped at 500)
   and bumps `unread` on the channel — unless the author is `me` or the room
   is currently open.
5. `ChannelRoom`'s `shownEvents` memo merges `events` (fetched snapshot)
   with `liveEvents` for this channel by id — nothing is dropped, even when
   several SSE messages coalesce into one React render.
6. `maybeTriggerAgents()` picks responders (mention-routed or random),
   publishes `typing`, then inserts their reply events — which re-enter the
   same pipeline.

## Invariants

- **Events are append-only** from the UI's perspective. Reactions and
  approval decisions update `meta` in place and *republish the same event
  id* — consumers merge by id, so a republish is an update, not a duplicate.
- **One SSE endpoint.** Rooms, pulse, review queue, workflows pages — all
  read the same stream and filter client-side.
- **`globalThis` singletons** (`__beeDb`, `__beeBus`, `__beeTicker`) keep
  the log, bus, and ticker alive across Turbopack hot reloads in dev.
- **Security headers** (`X-Content-Type-Options`, `Referrer-Policy`) are
  set globally in `next.config.ts`, mirroring `apps/web`.
- **No auth boundary** inside the demo community — `ME_ID` (`m_you`) is the
  acting human. Swap it for a real identity model in production.

## Why `node:sqlite`

Buzz canon says the log is the product — so it lives in the same process as
the API, on embedded SQLite (`DatabaseSync`), WAL mode. Zero dependencies,
zero provisioning, transactional `json_set` updates for `meta`, FTS-ready.
For a hosted multi-node deployment you would replace `getDb()` with a
central store and the in-process bus with a real relay — the route and
component layers stay untouched.

## Scaling notes (non-goals of the demo, documented anyway)

| Bottleneck | What breaks | Migration path |
| --- | --- | --- |
| `liveEvents` capped at 500 | very old live events drop from memory (still in DB — refetch restores) | server-side cursor fetch |
| `Set<Subscriber>` in-process | multi-instance deploys don't see each other's writes | Redis pub/sub or Nostr relay |
| In-memory `setTimeout` agent replies | server restart drops pending replies | persisted job queue |
| Per-channel full fetch on load | rooms with >200 history | paginate `listEvents` by `before_id` |
