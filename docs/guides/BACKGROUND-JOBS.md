# Background jobs

A guide to Fathom's durable background-job system — submitting long-running research tasks, monitoring progress, handling failures, and automating recurring work.

## Overview

Fathom jobs let you run research tasks **detached from your terminal**. A job survives terminal closure, machine sleep, and even `fathom` process restarts. The system is built on three layers:

1. **SQLite registry** (`~/.fathom/jobs.db`) — tracks every job's state, attempts, and error history
2. **Detached runner** — a fully independent OS process (`fathom job-run <id>`) that runs the research and reports back
3. **Self-healing retries** — failed attempts are retried with an augmented task that includes the previous error, so the agent can diagnose and fix its own mistakes

```
┌─────────────────────────────────────────────────────┐
│  fathom jobs submit "task"                           │
│    ↓                                                 │
│  Create job row (status=queued) in jobs.db           │
│    ↓                                                 │
│  Spawn detached: fathom job-run <id>                 │
│    ├── Attempt 1 → success    → mark completed       │
│    ├── Attempt 1 → fail       → record error, retry  │
│    ├── Attempt 2 (augmented) → success → mark done   │
│    └── Attempt N → fail       → mark failed, exit    │
└─────────────────────────────────────────────────────┘
```

---

## 1. Submitting a job

```bash
# Submit a research task (default: 3 attempts)
fathom jobs submit "Market analysis of AI coding assistants — 10 competitors, pricing, funding"

# Submit with a specific number of retry attempts
fathom jobs submit "Find 5 emails of CTOs at Berlin fintechs" --attempts 1

# Submit with a copious number of attempts for flaky tasks
fathom jobs submit "Deep research on RAG systems" --attempts 5
```

The CLI prints a summary:

```
Submitted job 550e8400
  Task:     Market analysis of AI coding assistants — 10 competitors, pricing, fund…
  Attempts: 3
  Dir:      /Users/you/.fathom/jobs/550e8400-e29b-7a1c-...
  Log:      /Users/you/.fathom/jobs/550e8400-e29b-7a1c-.../job.log

  Watch it live:  fathom jobs status 550e8400 --watch 5
  Tail the log:   fathom jobs logs 550e8400
```

### What happens on submit

1. `cmd_jobs_submit` creates a new row in `~/.fathom/jobs.db` with `status='queued'`, `attempt=0`, and a UUID v7 id
2. A workspace directory is created at `~/.fathom/jobs/<job-id>/`
3. `spawn_detached_runner` launches `fathom job-run <job-id>` as a completely independent process:
   - `stdin` is closed (no terminal input)
   - `stdout` and `stderr` are appended to `job.log` in the job's workspace
   - On Unix, the child process gets its own session (`setsid`) so it survives the parent terminal closing
4. The submit command returns immediately; the job runs in the background

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PR_JOBS_DB` | `~/.fathom/jobs.db` | Path to the jobs SQLite database |
| `PR_JOBS_DIR` | `~/.fathom/jobs` | Root directory for per-job workspaces |

---

## 2. Checking status

```bash
# List all jobs
fathom jobs list

# Show detailed status of one job
fathom jobs status 550e8400

# Watch a job live (refresh every 5 seconds until terminal)
fathom jobs status 550e8400 --watch 5
```

### List output

```
ID         STATUS      ATTEMPT   CREATED              TASK
550e8400   running     0/3       2026-08-19T10:30:00   Market analysis of AI coding…
a1b2c3d4   completed   2/2       2026-08-18T22:15:00   Find 5 emails of CTOs at Ber…
f0f1f2f3   failed      3/3       2026-08-17T14:00:00   Deep research on RAG systems
```

The list command detects **stale** jobs — rows marked `running` whose recorded process ID is no longer alive — and displays them as `stale` instead of `running`.

### Status output

```
Job:       550e8400 (550e8400-e29b-7a1c-4d5b-b6c7-8d9e0f1a2b3c)
Task:      Market analysis of AI coding assistants — 10 competitors, pricing, fund…
Status:    running (pid 87234)
Attempts:  1/3
Created:   2026-08-19T10:30:00+00:00
Started:   2026-08-19T10:30:02+00:00
Output:    /Users/you/.fathom/jobs/550e8400-e29b-7a1c-...
Log:       /Users/you/.fathom/jobs/550e8400-e29b-7a1c-.../job.log
```

Status types:

| Status | Meaning |
|--------|---------|
| `queued` | Job created, runner not yet started |
| `running (pid N)` | Runner is executing attempt N |
| `running, but the process is gone` | DB says running but PID is dead (stale) |
| `completed` | All attempts exhausted or success |
| `failed` | All attempts exhausted without success |
| `cancelled` | Cancelled by user |

### Watching with `--watch`

The `--watch` flag polls the database every N seconds (clamped to 1–3600). It exits automatically when the job reaches a terminal state (`completed`, `failed`, or `cancelled`). This is useful in automation scripts:

```bash
# Wait for a job to finish, then check the log
fathom jobs status 550e8400 --watch 5 && fathom jobs logs 550e8400
```

---

## 3. Reading logs

Each job captures all stdout and stderr from the runner into `<job_dir>/job.log`. This includes the research agent's output, tool calls, and error messages.

```bash
# Show the last 50 lines (default)
fathom jobs logs 550e8400

# Show the last 200 lines
fathom jobs logs 550e8400 -n 200

# Tail the raw file (for continuous following)
tail -f ~/.fathom/jobs/550e8400-*/job.log
```

The log contains the runner's progress output:

```
══ Job 550e8400 starting (max 3 attempt(s)) ══

══ Attempt 1/3 ══
[research agent output...]
❌ Attempt 1 failed: rate limit exceeded

Next attempt in 5s (jobs cancel to stop)

══ Attempt 2/3 ══
[research agent output with augmented task...]
✅ Job 550e8400 completed on attempt 2
```

---

## 4. Cancelling a job

```bash
fathom jobs cancel 550e8400
```

Cancellation:

1. Reads the job's current status from the database
2. If the job is already terminal (`completed`, `failed`, `cancelled`), prints a message and returns
3. If the job has a live PID, sends `SIGTERM` to the runner process
4. Marks the row as `cancelled` in the database (`status='cancelled'`, `pid=NULL`)

The runner process checks for cancellation between attempts (during backoff) and before each attempt. If cancelled mid-attempt, the runner will stop at the next attempt boundary.

Only `queued` and `running` jobs can be cancelled. Already terminal jobs are no-ops.

---

## 5. Re-running a job

```bash
# Re-run a failed, cancelled, or completed job
fathom jobs rerun 550e8400

# Re-run a stale job (marked running but process is dead)
fathom jobs rerun 550e8400
```

The rerun logic:

1. Resets the job's state based on its current status:
   - **queued** — already ready, no reset needed
   - **running with dead PID** — resets via `reset_running_with_pid` (guarded: only resets if the PID matches, preventing a race with a live runner)
   - **running with live PID** — bails with an error: "cancel it first"
   - **failed/cancelled/completed** — resets via `reset_for_rerun` (clears `attempt=0`, `error=NULL`, `pid=NULL`, `completed_at=NULL`, sets `status='queued'`)
2. Creates the workspace directory if missing
3. Spawns a fresh detached runner

```bash
# Typical workflow: re-run a failed job
fathom jobs logs 550e8400 -n 20   # inspect the error
fathom jobs rerun 550e8400         # restart from scratch
fathom jobs status 550e8400 --watch 5  # wait for completion
```

---

## 6. Retry logic and error augmentation

The most distinctive feature of Fathom jobs is **self-healing retries with task augmentation**.

### How retries work

The runner (`cmd_job_run`) loops over attempts from 1 to `max_attempts`:

```rust
for attempt in 1..=job.max_attempts {
    // Check if cancelled
    let fresh = db.get(&job.id)?;
    if fresh.status == "cancelled" {
        return Ok(());
    }

    // Mark this attempt as running
    db.mark_running(&job.id, attempt, std::process::id() as i64)?;

    // Build the task
    let task = if attempt == 1 {
        job.task.clone()  // original task, unchanged
    } else {
        augment_task_for_retry(&job.task, fresh.error.as_deref())  // augmented
    };

    // Run the research
    match run_research(task, Some(out_dir.clone()), None).await {
        Ok(()) => {
            db.mark_completed(&job.id)?;
            return Ok(());
        }
        Err(e) => {
            db.record_attempt_error(&job.id, &err)?;
            // Backoff before next attempt
            // (checks for cancellation during backoff)
        }
    }
}
// All attempts exhausted → mark failed
db.mark_failed(&job.id, &last_error)?;
```

### Task augmentation

On the second and subsequent attempts, the task is **augmented** with the previous error. The agent receives:

```
Original task

---

The previous attempt to complete this task FAILED with the following error:
<previous error>

The output directory contains partial artifacts from that attempt.
Inspect them, diagnose the root cause of the failure, fix the problem,
and finish the original task.
```

This means the agent can:

- **Read partial output** from the workspace directory
- **Diagnose the root cause** (rate limit, missing data, tool failure)
- **Fix the problem** (retry with backoff, use a different source, adjust the approach)
- **Finish the work** without starting from scratch

Retries are not blind re-executions — they are informed recovery attempts.

### Backoff between attempts

Between attempts, the runner sleeps for `5 * attempt` seconds (5s, 10s, 15s, …). During this backoff, it polls the database every second for cancellation signals, so cancellation is responsive even during the wait.

### Example retry flow

```
══ Job a1b2c3d4 starting (max 2 attempt(s)) ══

══ Attempt 1/2 ══
❌ Attempt 1 failed: API rate limit exceeded — 429 Too Many Requests

Next attempt in 5s (jobs cancel to stop)

══ Attempt 2/2 ══
[Agent receives augmented task describing the rate limit error.
 The agent sees the partial output, waits before making requests,
 and completes the research.]
✅ Job a1b2c3d4 completed on attempt 2
```

---

## 7. How jobs survive restarts

Jobs are fully durable across process restarts because all state lives outside the Fathom process:

1. **SQLite database** (`~/.fathom/jobs.db`) — persisted on disk with WAL mode and synchronous=NORMAL for crash safety
2. **Detached OS processes** — the runner is a child of the init process (via `setsid`), not of the terminal or Fathom CLI
3. **Workspace directory** — each job's output lives in `~/.fathom/jobs/<id>/`

If you restart Fathom (or reboot your machine):

- **Running jobs** continue executing — they were spawned with `setsid` and are independent
- **Stale jobs** (marked running but process died) are detected by `fathom jobs list` and `fathom jobs status` — they show as `stale` and can be re-run with `fathom jobs rerun`
- **Completed/failed jobs** remain in the database with their logs and errors

The only caveat: if a runner is killed abruptly (e.g., `SIGKILL`), the database row remains `status='running'` with a stale PID. This is handled by the `stale` detection and `rerun` command.

---

## 8. Comparison with foreground runs

| Aspect | Foreground (`fathom run`) | Background (`fathom jobs submit`) |
|--------|--------------------------|-----------------------------------|
| Terminal blocking | Yes — blocks until complete | No — returns immediately |
| Survivability | Dies with terminal | Survives terminal close, restart, reboot |
| Live output | Real-time in terminal | Written to `job.log` |
| Cancellation | Ctrl+C | `fathom jobs cancel` |
| Retries | None | Configurable with error augmentation |
| Status monitoring | n/a | `fathom jobs status`, `--watch`, TUI panel |
| Output directory | `--output` argument | `~/.fathom/jobs/<id>/` |
| Session history | `fathom sessions list` | `fathom jobs list` |

### When to use each

**Use foreground runs when:**
- You want interactive control (steering, question answering)
- The task is quick (< 1 minute)
- You're iterating on a query and want to see results immediately
- You need the TUI with live agent visualization

**Use background jobs when:**
- The task is long-running (minutes to hours)
- You want to close the terminal and come back later
- You're running multiple tasks in parallel
- You're automating research from scripts or cron
- The task is flaky and benefits from retry logic

---

## 9. Automation examples

### Shell script: submit and wait

```bash
#!/bin/bash
set -e

TASK="${1:-"Research top 5 competitors in the AI code review space"}"
ATTEMPTS="${2:-3}"

# Submit the job
OUTPUT=$(fathom jobs submit "$TASK" --attempts "$ATTEMPTS")
JOB_ID=$(echo "$OUTPUT" | grep -oP 'Submitted job \K\w+')
echo "Submitted job $JOB_ID"

# Watch until complete
fathom jobs status "$JOB_ID" --watch 10

# Check the final status
STATUS=$(fathom jobs status "$JOB_ID" 2>&1 | grep "Status:")
echo "Final status: $STATUS"

# Print the log
echo ""
echo "=== Log ==="
fathom jobs logs "$JOB_ID" -n 100
```

### Cron: recurring research

```cron
# Every Monday at 9 AM, run competitor tracking
0 9 * * 1 cd /home/user/research && \
  fathom jobs submit "Weekly competitor check: new funding, products, hires in AI code review" \
  --attempts 2 >> /home/user/research/jobs.log 2>&1
```

### CI: automated research pipeline

```yaml
# .github/workflows/research.yml
jobs:
  research:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Fathom
        run: cargo install --path .
      - name: Submit research job
        run: |
          fathom jobs submit "Analyze the Q3 funding landscape for European AI startups" \
            --attempts 3 > job.txt
          JOB_ID=$(grep -oP 'Submitted job \K\w+' job.txt)
          echo "JOB_ID=$JOB_ID" >> $GITHUB_ENV
      - name: Wait for completion
        run: |
          fathom jobs status ${{ env.JOB_ID }} --watch 30
      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: research-output
          path: ~/.fathom/jobs/${{ env.JOB_ID }}*/
```

### Parallel batch processing

```bash
#!/bin/bash
# Submit multiple jobs in parallel and wait for all to complete

TASKS=(
  "Research 10 AI note-taking apps with pricing"
  "Find 5 CTOs at Berlin climate tech startups"
  "Analyze the RAG framework landscape (LlamaIndex, LangChain, etc.)"
)

for task in "${TASKS[@]}"; do
  fathom jobs submit "$task" --attempts 2
done

echo "All jobs submitted. Monitoring..."

# Wait for all non-terminal jobs to finish
while true; do
  RUNNING=$(fathom jobs list | grep -c -E "queued|running")
  if [ "$RUNNING" -eq 0 ]; then
    echo "All jobs complete!"
    break
  fi
  echo "$RUNNING job(s) still running..."
  sleep 30
done

# Print all logs
fathom jobs list | tail -n +2 | while read -r ID REST; do
  echo "=== Job $ID ==="
  fathom jobs logs "$ID" -n 20
  echo ""
done
```

### Notification on completion

```bash
#!/bin/bash
# Submit, wait, and notify via Telegram

JOB_ID=$(fathom jobs submit "Monitor new AI GitHub repos" --attempts 2 | \
  grep -oP 'Submitted job \K\w+')

fathom jobs status "$JOB_ID" --watch 10

STATUS=$(fathom jobs status "$JOB_ID" 2>&1 | grep "Status:")
curl -s -X POST "https://api.telegram.org/bot$TOKEN/sendMessage" \
  -d "chat_id=$CHAT_ID" \
  -d "text=Research job $JOB_ID finished: $STATUS"
```

---

## 10. Architecture reference

### Database schema

```sql
CREATE TABLE IF NOT EXISTS jobs (
    id            TEXT PRIMARY KEY,          -- UUID v7
    task          TEXT NOT NULL,             -- the research task
    status        TEXT NOT NULL DEFAULT 'queued',  -- queued|running|completed|failed|cancelled
    attempt       INTEGER NOT NULL DEFAULT 0,      -- current attempt number
    max_attempts  INTEGER NOT NULL DEFAULT 1,      -- max retry count
    output_dir    TEXT NOT NULL,             -- workspace directory
    error         TEXT,                      -- last attempt error message
    pid           INTEGER,                   -- runner process ID
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    started_at    TEXT,
    completed_at  TEXT
);

CREATE INDEX idx_jobs_status ON jobs (status);
```

### Key operations

| Operation | Database action | OS action |
|-----------|----------------|-----------|
| Submit | `INSERT` with `status='queued'` | `spawn_detached_runner` (setsid, logger) |
| Start attempt | `UPDATE status='running', pid=N` | — |
| Log error | `UPDATE error='...'` | — |
| Complete | `UPDATE status='completed', error=NULL` | — |
| Fail | `UPDATE status='failed', error='...'` | `exit(1)` |
| Cancel | `UPDATE status='cancelled'` | `kill(pid, SIGTERM)` |
| Rerun (terminal) | `UPDATE status='queued', attempt=0` | `spawn_detached_runner` |
| Rerun (stale) | `UPDATE status='queued' WHERE pid=dead_pid` | `spawn_detached_runner` |

### PID-alive check

The `pid_alive` function uses `kill -0 <pid>` — a signal that doesn't deliver a signal but reports whether the process exists. This is used for:

- Displaying stale status in `fathom jobs list`
- Guarding the stale-job rerun path
- Deciding whether to SIGTERM before cancelling

### TUI integration

The TUI has a **Jobs panel** that opens the jobs database (best-effort) and shows live job status alongside the main agent view. The TUI receives the `jobs_db` reference and includes it in the event loop, so you can monitor jobs while running interactive sessions.

---

## 11. Troubleshooting

### Job stuck on "running" but no process

```bash
# Check if the PID is actually alive
ps aux | grep "$(fathom jobs status <id> | grep 'pid' | grep -oP '\d+')"

# If dead, re-run it
fathom jobs rerun <id>
```

The `rerun` command will only reset if the PID is confirmed dead (via `kill -0`). A live runner will not be disturbed.

### Job log is empty

The log file is created as soon as the runner starts. If empty:
- The runner may still be initializing (check `fathom jobs status`)
- The runner may have crashed before producing output (check for errors in the status)
- The database path may be different if `PR_JOBS_DB` or `PR_JOBS_DIR` is set

### Database path

```bash
# Check where the jobs database lives
ls -la ~/.fathom/jobs.db

# Override with environment variable
PR_JOBS_DB=/custom/path/jobs.db fathom jobs list
```

### Runner crashes

If the runner crashes (SIGKILL, OOM, etc.), the job remains in `running` state. The next `fathom jobs list` will show it as `stale`. Use `fathom jobs rerun` to reset and restart from scratch.