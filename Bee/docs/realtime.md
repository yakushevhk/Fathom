# Realtime: SSE, the bus, and merge semantics

## Wire protocol

`GET /api/events/stream` returns an open `text/event-stream` response.
Frames are `data: <json>\n\n`; comments (`: connected`, `: ping`) every
25 s keep intermediaries from closing the connection.

Message envelope — `src/lib/bus.ts`:

```ts
type BusMessage =
  | { type: 'event';    data: HiveEvent }                                     // new or updated log row
  | { type: 'typing';   data: { channelId; memberId; until } }                // transient indicator
  | { type: 'workflow'; data: Workflow }                                      // step/run transition
  | { type: 'approval'; data: Approval }                                      // new or decided gate
  | { type: 'presence'; data: { memberId; presence } }
```

`event` frames carry the **whole row** for both inserts and updates
(reaction toggles, approval backfills) — consumers must merge by `id`,
never append blindly.

## Client pipeline — `src/lib/store.tsx`

`HiveProvider` owns one `EventSource` for the whole app:

```
es.onmessage
├─ 'event'    → setLiveEvents(dedupe-append by id, cap 500)
│             → bump channel.unread / lastEventAt (skipped for own events
│               and the currently open room)
├─ 'typing'   → typing[channelId] += {memberId, until} (+ cleanup timeout)
└─ 'workflow'/'approval' → liveVersion++ (refetch hint)
```

### The `liveEvents` queue — why it exists

React batches state updates: several `es.onmessage` calls can land inside
one render. A single `lastEvent` slot silently drops all but the last —
this was a real bug (3 rapid posts → middle one never rendered).
`liveEvents` is an ordered, id-deduped array; the `setLiveEvents` updater
sees **every** message regardless of render batching, so nothing is lost.

Consumers then merge rather than subscribe to "the latest":

```ts
// ChannelRoom.tsx — shownEvents
const byId = new Map<number, HiveEvent>()
for (const e of events)        byId.set(e.id, e)   // fetched snapshot
for (const e of liveEvents)    byId.set(e.id, e)   // live wins (fresher)
return [...byId.values()].sort((a, b) => a.id - b.id)
```

`/pulse` merges the same way (filtered to `kind`, sorted descending).

### Rules for consumers

- **Merge by `id`**, sort by `id` — arrival order on the wire *is* creation
  order, and live copies are always at least as fresh as fetched ones.
- **Filter by `channelId`** when rendering a room; events for other rooms
  are in the queue too.
- Don't keep your own accumulator state — `liveEvents` is the accumulator.
  (Render-phase `setState` bookkeeping existed before and was removed.)
- For non-event state (workflows/approvals lists) refetch on `liveVersion`.

### Unread bookkeeping

The store tracks the open room by watching `location.pathname` (popstate +
1 s poll — App Router soft navigation doesn't emit a reliable signal).
`openChannel.current` suppresses unread bumps for the room being viewed;
`ChannelRoom` calls `markRead` on load and whenever a fresh `liveEvents`
tail lands in the open room.

### Typing indicators

`typing` is a map `channelId → {memberId, until}[]`. Each publish replaces
the member's entry and schedules a removal timeout at `until`. It lives in
the store, not the room — so typing in another room never re-renders the
open one.

## Server side

- `bus.ts` keeps `Set<Subscriber>` on `globalThis` (HMR-safe). A throwing
  subscriber is unsubscribed.
- `events/stream` wraps each subscriber in `controller.enqueue`; on
  `cancel` it unsubscribes. First frame is `: connected`; heartbeats are
  comment frames (spec-compliant, ignored by `EventSource` handlers).
- The stream route is also the ticker's lazy start point — first SSE
  connection calls `startTicker()`.
