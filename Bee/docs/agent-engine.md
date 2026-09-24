# Agent engine — `src/lib/agents.ts`

The Fathom fleet inside the workspace is simulated but *canon-faithful*:
agents are real `members` rows, their replies are real `events` rows, and a
human gating them works through the real `approvals` flow — nothing in the
UI knows they're simulated.

## Trigger: `maybeTriggerAgents(channelId, authorId, body, parentId)`

Called by `POST /api/channels/:id/messages` after a human's event lands.
The engine picks responders per room kind:

| Room | Responders |
| --- | --- |
| `dm` | the channel's `peerId` agent only |
| `channel`/`announcement` | `@handle` mentions pick that agent (max 2); no mention → 1–2 random agents |

Only `kind='agent'` members in the room are candidates.

## Reply choreography — `scheduleReply()`

Each reply is two `setTimeout`s, both publishing onto the bus:

1. `delay ms` → `{type:'typing', channelId, memberId, until}` — the room
   shows "Atlas is typing…" for `until − now`.
2. `delay + typingMs` → `insertEvent(... 'message', text, {replyTo:
   parentId})` — a real event that re-enters the SSE pipeline like any
   other.

Typical timeline for one responder: typing appears ~1.2 s after the human
message, the reply lands ~1.4–3.0 s later. Multiple responders are
staggered by +2.6–5.0 s each, so rooms read like conversations, not a
burst.

Templates live in `REPLIES` keyed by agent id — each agent has a voice
(Atlas decomposes into DAGs, Forge talks patches, Sonar cites prior
incidents, Beacon signs receipts, Quill drafts prose, Ping watches
telemetry). `@atlas`-mentions get the clipped user text echoed back.

## Approval gates — `addApproval()`

After an agent reply, `approvalChance = 0.14` rolls whether it *also* files
a gated action (`workspace:execute`, risk `medium`, detail naming the
triggering message). This is what populates `/review` and the HUMAN GATE
cards in rooms during normal use — plus the seeded pending gates.

## Workflow ticker — `startTicker()`

A single `setInterval(18_000)` (guarded by `globalThis.__beeTicker`)
picks a random `queued`/`running` workflow and calls `advanceWorkflow()`:

- `queued` → `running` (stamps `started_at`), or
- advances the first `pending` step to `running` → next tick `succeeded`,
- last step `succeeded` → workflow `succeeded`, stamps `finished_at`,
  and inserts a `workflow` event into its channel.

This keeps `/workflows`, its DAG timelines, and Pulse visibly alive with no
user input. Replies and ticks are `try/catch`-guarded — the simulation
never crashes the server.

## Replacing the simulation with real Fathom

The engine is intentionally boxed:

- Swap `REPLIES` template selection for a call into the real agent runtime
  (`crates/agent` / ACP bridge) — `scheduleReply` already models the
  typing→respond contract the runtime emits.
- Swap `addApproval` calls for real gated tool invocations; the
  `approvals` row + event-card + `decideApproval` path is already
  production-shaped.
- Keep `startTicker` semantics by subscribing it to real workflow runs —
  the UI only needs `{type:'workflow'}` frames.
