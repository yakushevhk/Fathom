# State Management Architecture (`src/state/`)

## 1. Directory Purpose & Scope

The `src/state/` directory houses the global state store, state reducers, optimistic mutation queues, and action dispatchers for the Parallel frontend application. 

Parallel employs a **server-authoritative, client-optimistic** architecture. The client maintains no direct persistent storage or ad-hoc WebSockets. Instead, it interacts through typed HTTP REST commands for actions, and reconciles state through a continuous Server-Sent Events (SSE) feed (`/api/events`) managed by a pure reducer.

---

## 2. Architectural Role & State Flow

```
                         +-----------------------------+
                         |      Backend HTTP & SSE     |
                         +--------------+--------------+
                                        |
                 +----------------------+----------------------+
                 | SSE Event Stream                            | REST Responses
                 | (/api/events)                               | (POST / PATCH / GET)
                 v                                             v
  +-------------------------------+             +-------------------------------+
  |       live-events.ts          |             |       StoreProvider           |
  |  (Supervisor, Resumption,     |             |  - Network dispatch wrapper   |
  |   Liveness Pings, Reconnect)  |             |  - Task write queues          |
  +---------------+---------------+             |  - BotPatchQueue (debounce)   |
                  |                             +---------------+---------------+
                  | raw frames                                  |
                  v                                             | actions
  +-------------------------------+                             |
  | createStreamDeltaBuffer       |                             |
  | (rAF-batched token chunks)    |                             |
  +---------------+---------------+                             |
                  |                                             |
                  | stream text                                 |
                  v                                             v
  +-------------------------------+             +-------------------------------+
  |         StreamContext         |             |          reducer()            |
  |  (Per-frame streaming state:  |             |  (Pure function: AppState     |
  |   streaming, reasoning)       |             |   transforms & reconciles)    |
  +-------------------------------+             +---------------+---------------+
                                                                |
                                                                v
                                                +-------------------------------+
                                                |         StoreContext          |
                                                |  (AppState & Dispatch to UI)  |
                                                +-------------------------------+
```

---

## 3. Core Modules & Data Contracts

### 3.1 `store.tsx` (Root State Engine)
`store.tsx` is the primary entry point for state management, exporting `StoreProvider`, `useStore`, `useStreaming`, and the pure `reducer` function.

#### Core State Model (`AppState`)
- **`bots: Bot[]`**: Full registry of configured AI agents, their configurations, personalities (`soul`), model configurations, and message history.
- **`groups: Group[]`**: Multi-agent team rooms with shared bulletin boards, member lists, and shared task transcripts.
- **`selectedId: string`**: Currently active bot ID or group ID displayed in the main viewport.
- **`activeView: "chat" | "routines" | "team-map"`**: Current top-level view mode.
- **`secondarySelectedId: string | null` & `splitRatio: number`**: Coordinates the split-screen dual workspace layout (`SplitWorkspace.tsx`).
- **`screens: Record<string, { png: string; mime: string }>`**: Latest live screen capture frame for each bot operating a computer.
- **`pendingQueued: Record<string, QueuedMessage[]>`**: Outbox of user prompts waiting to be dispatched when a busy agent finishes its turn.
- **`backgroundThreadEvents: Record<string, Action[]>`**: Bounded race buffer capturing events arriving for non-active threads.

#### Stream Isolation (`StreamContext`)
To ensure high-performance rendering during high-speed LLM generation (up to hundreds of tokens per second), assistant and reasoning token streams bypass the root `AppState`.
- Handled by `createStreamDeltaBuffer`: batches chunk insertions and executes flushes aligned with browser display refresh rates using `requestAnimationFrame`.
- Accessible strictly via `useStreaming()`, isolating re-renders to active text bubbles and keeping the rest of the application tree static.

### 3.2 `bot-patch-queue.ts` (Optimistic Patch Coalescing)
- **Role**: Solves write-amplification and race conditions when operators rapidly tweak agent settings (sliders, toggles, prompt textareas).
- **Mechanism**:
  - Implements a debounce timer (default 400ms) with in-flight tracking.
  - Maintains three layers: `fallback` (last acknowledged server state), `pending` (local uncommitted edits), and `inFlight` (currently executing HTTP PATCH).
  - Merges subsequent edits while a request is in flight.
  - Exposes `flush()` to force immediate synchronization before critical operations (such as sending a message with new model settings).

---

## 4. Key Actions & Optimistic Update Lifecycle

### 4.1 Message Lifecycle
1. **Dispatch**: Operator sends a message via `send` or `sendGroup`.
2. **Optimistic Projection**: `optimisticUserMessage` appends a provisional message with `id: optimistic-${sendId}` to the active transcript immediately.
3. **HTTP Transport**: `POST /api/bots/:id/messages` transmits the payload.
4. **Server Acknowledgment & Reconciliation**:
   - The server pushes a `message_added` frame via SSE containing the durable server message.
   - The reducer matches the durable message or cleans up the optimistic placeholder with `optimisticMessageRemoved`, ensuring no duplicate bubbles appear.

### 4.2 Bot Configuration Updates
1. `dispatch({ type: "updateBot", botId, patch })` registers the change.
2. The UI reflects the patch instantly using local state overlays.
3. `BotPatchQueue` debounces and serializes the update to `PATCH /api/bots/:id`.
4. If an error occurs, `BotPatchQueue` restores the `fallback` state and surfaces a transient notification.

### 4.3 Task and Thread Switching
- Bots support multiple concurrent tasks/threads (`Task`).
- Switching tasks dispatches `switchTask`, updating `bot.threadId` and loading associated transcripts while isolating in-flight tasks from background interruptions.

---

## 5. Invariants & Reliability Guarantees

1. **Reducer Purity**:
   - The `reducer(state: AppState, action: Action): AppState` function must remain completely pure and free of side effects. No network I/O, timers, or DOM modifications are permitted inside the reducer.
2. **Background Event Isolation**:
   - SSE messages for inactive threads must route to `backgroundThreadEvents` and not pollute the current visible transcript.
3. **Queue Tombstoning**:
   - Drained or canceled queue entries maintain a bounded tombstone (`consumedQueueIds`, max 64 items) to prevent late HTTP responses from resurrecting stale prompts.
4. **Task Write Recovery**:
   - Before executing a turn, all pending task property mutations (like working directory changes or model overrides) must resolve through `waitForExecutionSettings`.

---

## 6. Testing & Verification

The state layer is covered by Vitest suites:
- **Comprehensive Store Tests**:
  ```bash
  pnpm vitest run src/state/store.test.ts
  ```
  Validates reducer actions, message branches, task switching, and queue reconciliation.
- **Bot Patch Queue Tests**:
  ```bash
  pnpm vitest run src/state/bot-patch-queue.test.ts
  ```
  Verifies debouncing, in-flight coalescing, error rollback, and lifecycle disposal.
- **Task Write Recovery Tests**:
  ```bash
  pnpm vitest run src/state/task-write-recovery.test.ts
  ```
  Validates task setting guarantees prior to message execution.
- **Bot Creation Tests**:
  ```bash
  pnpm vitest run src/state/bot-creation.test.ts
  ```
