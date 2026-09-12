# Parallel Server (`server/`)

## 1. Purpose and Scope

The `server/` directory implements the core backend harness host for Parallel. Under the harness architectural model, clients (the React desktop and web applications, phone companions, and CLI tools) maintain no direct provider transport connections. Instead, all agent orchestration, child process lifecycle management, provider protocol translation, state persistence, credentials handling, and sandboxed tool executions are hosted within this Node.js server.

Clients interact with the server by dispatching typed REST commands over HTTP and consuming a single, unified Server-Sent Events (SSE) event stream.

---

## 2. Architectural Role and Data Flow

```
+-----------------------------------------------------------------------+
|                         Clients & Companions                          |
|         (React UI / Desktop Electron / Phone / Remote Browsers)       |
+-----------------------------------+-----------------------------------+
                                    |
                  HTTP Commands     | SSE Event Stream
                 (Bearer / Cookie)  | (/api/events)
                                    v
+-----------------------------------------------------------------------+
|                      Request Authentication Layer                     |
|            (request-auth.ts / sessions.ts / peer-approval.ts)         |
+-----------------------------------+-----------------------------------+
                                    |
                    Route Handlers (index.ts)
                                    |
        +---------------------------+---------------------------+
        |                           |                           |
        v                           v                           v
+---------------+           +---------------+           +---------------+
| Bot / Thread  |           |  Turn Engine  |           |  Subsystems   |
| State (Store) |           | & Dispatcher  |           | & Proxies     |
| (store.ts,    |           | (turn-context,|           | (tts/, mcp,   |
| message-db.ts)|           | steer-queue.ts)           | container-vm) |
+---------------+           +-------+-------+           +---------------+
                                    |
                                    v
                        +-----------------------+
                        |   Provider Harness    |
                        | (harness/registry.ts) |
                        +-----------+-----------+
                                    |
                                    v
                        +-----------------------+
                        |    Driver Adapters    |
                        |      (drivers/)       |
                        +-----------+-----------+
                                    |
                      JSON-RPC / Stdio / REST API
                                    v
                        +-----------------------+
                        | AI Provider CLIs/APIs |
                        | (Claude, Grok, Codex) |
                        +-----------------------+
```

### Turn Lifecycle
1. **Command Ingress**: A client issues a `POST /api/bots/:id/send` request with message text, optional attachments, and model selection.
2. **Authentication & Validation**: `request-auth.ts` verifies session credentials or loopback origin; input is validated against Zod schemas.
3. **Turn Preparation**: `turn-context.ts` resolves thread history, memory topics, workspace instructions, and peer roster prompts.
4. **Dispatch**: The turn is routed via `harness/registry.ts` to the configured `ProviderInstance` driver (e.g., Claude Code, Grok, Codex, or ACP harness).
5. **Execution & Event Fan-In**: The driver spawns or drives the provider CLI/agent. Runtime events (`content.delta`, `item.started`, `turn.completed`, `request.opened`) emit to `harness/bus.ts`.
6. **Persistence & Fan-Out**: The bus redacts sensitive credentials, appends the canonical event to an NDJSON log (`~/.parallel/events/<threadId>.ndjson`), updates the conversation store (`store.ts` / `message-db.ts`), and broadcasts the event via SSE to connected clients.

---

## 3. Key Files and Exported Modules

### Entry Points and Configuration
- **`index.ts`**: Main HTTP server entrypoint. Sets up HTTP routing, SSE connection pools, WebSocket/tunnel handlers, and server boot sequence.
- **`cli.ts`**: Command-line interface for the server (`parallel setup`, `serve`, `start`, `pair`, `sessions`, `status`, `tunnel`, `fleet`).
- **`config.ts`**: Manages server directories (`DATA_DIR`, `EVENTS_DIR`, `ATTACHMENTS_DIR`), loads and validates `~/.parallel/config.json` via Zod, and synchronizes credential environment variables.
- **`contracts.ts`**: Canonical harness contracts, types, and error classes (`RuntimeEvent`, `ProviderDriver`, `ProviderInstance`, `ModelSelection`, `ProviderError`).

### State and Persistence
- **`store.ts`**: Persists bot definitions, avatars, active thread mappings, and resume cursors in `bots.json` using atomic disk writes (`atomic.ts`).
- **`message-db.ts`**: Message persistence and retrieval engine supporting fast thread queries, message searches, and citation crossings.
- **`atomic.ts`**: Safe file writing (`writeFileAtomic`) using temporary files and atomic renaming to prevent corrupted states across process crashes.
- **`workspace.ts`**: Manages bot-scoped file workspaces, memory files, and topic journals.

### Turn and Concurrency Management
- **`turn-context.ts`**: Assembles turn context, including system prompts, soul files, memory summaries, and recovery states for failed turns.
- **`steer-queue.ts` & `channel-queue.ts`**: In-flight steering and message queuing mechanics for interleaving user interruptions during active turns.
- **`turn-watchdog.ts` & `room-turn-timeout.ts`**: Deadline enforcement and stall monitoring for stuck CLI child processes.
- **`turn-resources.ts`**: Locking and serialization mechanisms preventing concurrent conflicting mutations on the same workspace or thread.

### Security and Networking
- **`request-auth.ts`**: Multi-tiered authentication verifying loopback status, host headers, reverse proxy headers, bearer tokens, and session cookies.
- **`sessions.ts`**: Management of authenticated pairing sessions, device tokens, and scope delegations (`admin`, `client`).
- **`peer-approval.ts` & `auto-approve.ts`**: Permission proxy handling security evaluation for tool execution, command execution, and peer communications.
- **`tunnel.ts` & `tailscale.ts`**: Remote ingress adapters supporting Cloudflare Tunnels and Tailscale Serve HTTPS termination.

---

## 4. Configuration and State Persistence Invariants

1. **Atomic File Writes**: All configuration (`config.json`), bot definitions (`bots.json`), and transcripts (`messages-<threadId>.json`) must be written via `writeFileAtomic()`. Writes write to a random sibling temporary file before calling `renameSync()` to avoid partial writes.
2. **Forward/Backward Configuration Compatibility**: Unknown properties or unrecognized driver kinds in configuration maps must not crash server initialization. Unrecognized drivers degrade into shadow instances (`ShadowInstance`) so that newer configs can safely run on older server versions without data loss.
3. **Redacted Canonical Logs**: Event logs written to `EVENTS_DIR/<threadId>.ndjson` are sanitized via `redactSecrets()` before disk writes. Disk write failures must emit a warning event and degrade gracefully rather than crash the event bus.
4. **Credential Isolation**: Credentials (`API keys`, OAuth tokens, session secrets) must remain write-only from the perspective of client APIs. `GET /api/config` returns status booleans indicating whether credentials are configured, never raw secret strings.

---

## 5. Security Constraints and Boundaries

- **Loopback Trust**: Unauthenticated administrative commands are permitted only over loopback (`127.0.0.1`, `::1`, `localhost`). A request is verified using `isLoopbackHost()`. If reverse proxy headers (`x-forwarded-proto`, `x-forwarded-for`) are detected via `isProxied()`, loopback trust is revoked, and explicit session tokens are mandatory.
- **Origin Checking & CSRF Defense**: Origin headers from browser clients are strictly checked against `isAllowedOrigin()`. Untrusted third-party origins are rejected with HTTP 403.
- **Child Process Sandbox**: Driver CLI processes are executed with pruned environment variables. High-risk environment variables (`AWS_*`, `GITHUB_TOKEN`, server session secrets) are explicitly stripped using `stripWorkspaceCredentialEnv()` unless specifically whitelisted by the target driver.
- **Fail-Closed Permission Model**: In interactive approval modes (`ask`, `prompt`), tool invocations and external side effects pause execution until explicit user approval is received. Options without explicit `allow` semantics default to denial.

---

## 6. Verification and Testing
Server unit and integration tests are written using Vitest:

```bash
# Run server test suite
pnpm test

# Target specific server tests
pnpm vitest run server/config.test.ts
pnpm vitest run server/request-auth.test.ts
pnpm vitest run server/store.test.ts
```
