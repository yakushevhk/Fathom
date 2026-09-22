# pr-ax — AX orchestrator for Fathom

`pr-ax` is Fathom's standalone Rust implementation of the
[google/ax](https://github.com/google/ax) runtime model: a declarative
agentic-task orchestrator driven by `ax.io/v1alpha1` YAML manifests.

Upstream AX targets Kubernetes (reconciler → suspended actor containers).
This crate runs the **same resource model and lifecycle against a local
single-writer controller** with a durable SQLite event log — no cluster
required — and plugs Fathom's agent runtime in as the default task
harness.

## Mapping: google/ax → pr-ax

| google/ax (Kubernetes) | pr-ax (local) |
|---|---|
| Controller (single-writer, `internal/controller`) | `AxController` — one owner of the store; all mutations serialized through it |
| etcd + `WatchTask` RPC | SQLite (WAL) `resources`/`events`/`actors` tables + broadcast watch channel |
| Actor = suspended sidecar container (`ax.io/actor`) | Actor = isolated child process in its own process group |
| Suspend/resume (SandboxedImage) | `SIGSTOP`/`SIGCONT` on the process group; respawn after loss |
| `AX_STATE_DIR` PVC | `~/.fathom/ax` (`$FATHOM_AX_HOME`) — store, logs, workspaces |
| atespace (tenant namespace) | `metadata.atespace` (default `default`) |
| Execution forking by seq | `fathom ax fork <task> --fork-name X --at-seq N` |
| `ax apply / get / watch / logs / ssh` | `fathom ax apply / get / status / watch / logs` (+ suspend/resume/fork/delete/describe/run/events) |

## Manifests (`ax.io/v1alpha1`)

Four kinds — `Task`, `Gateway`, `Workspace`, `Model` — parsed strictly
(unknown fields rejected) and multi-document YAML supported:

```yaml
apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: research-agent
spec:
  workspaces:
    - name: fathom-src
      path: src
      goal: "Audit the repo and write a report"
  gateway:
    name: corp-egress
```

See [`examples/`](examples/) for a full multi-document pipeline and a
suspend/fork demo.

### Task lifecycle

`Pending → Running → Completed | Failed`
`Running → Suspended → Running` (SIGSTOP/SIGCONT)
`Running → Interrupted` (actor lost across a controller restart) → `resume` respawns it
`* → Terminating` (delete) → teardown → row removed

### Environment injected into actors

Mirroring upstream's reconciler, every actor receives:

- `AX_TASK_YAML` — the task's own manifest
- `AX_WORKSPACES_YAML` — its bound Workspace documents
- `AX_EGRESS_ALLOWLIST` — effective egress policy (default `*:443`; the
  allowlist is exported declaratively — local process actors have no
  kernel-level network enforcement)
- `AX_MODEL_*` / `MODEL_API_KEY` — from the atespace's default `Model`
  (`secretKey.name` is resolved from the process environment, the local
  stand-in for a k8s Secret)

### Fathom adapter

`spec.command` empty ⇒ the actor is Fathom itself:
`fathom run <goal> -o <task-dir>/output`, where `<goal>` is the first
bound workspace's `goal` (falling back to the task name). Git entries in
bound Workspaces are cloned into `.ax/workspaces/<task>/` before the
actor starts.

## CLI

```bash
fathom ax apply -f examples/simple.yaml     # declarative apply
fathom ax run   -f examples/simple.yaml     # apply + watch to terminal
fathom ax status [task]                     # phase/pid/actor table
fathom ax list task|workspace|gateway|model
fathom ax get task <name>                   # YAML
fathom ax describe task <name>              # manifest + conditions + events
fathom ax watch <name> [--from-seq N]       # durable replay + live stream
fathom ax logs <name> [-f] [--tail N]
fathom ax suspend <name> / resume <name>
fathom ax fork <name> --fork-name X [--at-seq N]
fathom ax delete task <name>                # two-phase (Terminating → gone)
fathom ax events [--from-seq N]             # raw event log
```

## Durability, resumption, forking

Every state change is appended to `store/events` **before** the status row
is written — replay recovers exact history (`watch --from-seq`,
`ax events`). On controller start, `recover()`:

- reattaches monitors to still-running actors (pid registry), or
- marks tasks whose actor vanished `Interrupted` — `ax resume` then
  respawns the declared command durably.

`fork` creates a sibling task with `status.forked_from` +
`status.fork_seq` recorded and its own event chain — divergent execution
from a chosen sequence number.

## Layout

```
src/manifest.rs    ax.io/v1alpha1 kinds, strict multi-doc parse, validation
src/store.rs       SQLite WAL: resources + append-only events + actor registry
src/controller.rs  single-writer reconciler, suspend/resume/fork, recovery
src/actor.rs       process actors: spawn, SIGSTOP/SIGCONT, kill, log tail
src/ops.rs         CLI read paths (rows, describe, log read)
```
