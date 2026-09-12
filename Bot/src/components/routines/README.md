# Automated Routines & Scheduled Tasks

The `routines` directory contains the frontend user interface for creating, viewing, scheduling, and monitoring recurring automated workflows, cron triggers, webhook dispatches, and background bot tasks within Parallel.

---

## Directory Architecture

```text
src/components/routines/
├── CalendarSidebar.tsx        # Month date navigator and draggable bot list sidebar
├── MiniMonth.tsx              # Compact calendar grid for jumping between schedule dates
├── RoutineList.tsx            # Chronological list of configured routines with status badges
├── RoutineLogs.tsx            # Run history audit log with status filtering and output inspect
├── ResultsDestination.tsx     # Destination thread picker (new thread vs. existing task thread)
├── ResultsDestination.test.ts # Unit tests for results thread selection and project grouping
└── RoutineViews.test.ts       # Test suite validating routine sorting and status display
```

---

## Data Model & Execution Invariants (`src/lib/routines.ts`)

Routines run in the background on behalf of a specific bot or team room goal without requiring user interaction.

```text
┌────────────────────────────────────────────────────────┐
│                        TRIGGER                         │
│  - Schedule ("once", "daily", or recurring "interval") │
│  - Webhook Ingress (POST /hooks/:id)                   │
│  - Manual User Dispatch ("Run Now")                    │
└───────────┬────────────────────────────────────────────┘
            │
            ▼
┌────────────────────────────────────────────────────────┐
│                      ROUTINE RUN                       │
│  - Target: Bot Task or Room Goal                       │
│  - Execution Thread: Isolated or Pinned Destination    │
│  - Safety Constraints: Duration & Timeout limits       │
│  - Status State Machine:                               │
│      queued ──► running ──► [waiting] ──► completed    │
│                    │                         │         │
│                    └──────► failed / cancelled / missed│
└────────────────────────────────────────────────────────┘
```

### 1. Schedule Types (`RoutineSchedule`)
- `once`: Runs once at a designated Unix timestamp (`at: number`).
- `daily`: Runs every day (or specified `weekdays: number[]`, Sunday = 0) at a specific 24-hour wall-clock time (`time: "09:30"`).
- `interval`:
  - `everyMinutes`: Cadence in minutes.
  - `anchorAt`: Epoch millisecond anchor point determining schedule phase alignment.
  - `window`: Optional time window boundary (`start: "09:00"`, `end: "17:00"`).
  - `weekdays`: Optional weekday restrictions.
  - `endsAt`: Optional epoch cutoff after which the routine halts.

### 2. Trigger Sources (`RoutineRunTrigger`)
- `schedule`: Triggered by the server's background scheduler clock.
- `manual`: Dispatched directly by the user clicking "Run Now".
- `webhook`: Triggered via external HTTP request through the webhook ingress edge (`POST /hooks/:webhookId`).

### 3. Execution Safety Limits
- `durationMinutes`: Expected target duration for the task.
- `timeoutMinutes`: Wall-clock safety cutoff. If an agent process stalls, hangs on shell I/O, or loops infinitely, the engine enforces process termination when the timeout threshold elapses.
- `attention`: Set when a routine run enters the `waiting` state, capturing a redacted explanation if the bot paused to wait for interactive human approval (e.g., when `approvalMode === "ask"`).

---

## Component Roles & Interactions

### 1. `RoutineList.tsx`
- Renders configured routines sorted by active state, next scheduled execution time (`nextRunAt`), and name.
- Displays schedule summaries via `scheduleLabel(routine.schedule)`.
- Highlights the latest run state using status color tones (`completed` = green, `failed` = red, `running` = blue, `waiting` = amber).

### 2. `ResultsDestination.tsx`
Configures where the bot posts its output and conversation logs:
- `new` (`null`): Spawns a clean, dedicated thread for every execution, avoiding context contamination.
- `threadId`: Appends results into an existing pinned conversation or project folder.
- Filters out previous routine execution threads to prevent circular thread nesting.

### 3. `CalendarSidebar.tsx` & `MiniMonth.tsx`
- Date picker enabling day/week schedule exploration.
- Supports drag-and-drop scheduling: users can drag a bot avatar from the sidebar (`BOT_CALENDAR_DRAG_TYPE = "application/x-openmaus-bot"`) directly onto a calendar time slot to initiate routine creation.

### 4. `RoutineLogs.tsx`
- Comprehensive execution audit viewer.
- Real-time filtering across bot names, routine titles, output text, errors, and run statuses (`queued`, `running`, `waiting`, `completed`, `failed`, `missed`, `cancelled`).
- Displays execution cost, tool invocation denials, and full textual outputs.

---

## Verification & Testing

Execute routines component unit tests:
```sh
npm test src/components/routines/
```

Key verification points:
- `ResultsDestination.test.ts`: Confirms that selecting a project folder partitions task threads correctly and handles deleted thread fallbacks.
- `RoutineViews.test.ts`: Verifies date formatting, relative next-run labels, and sorting invariants.
