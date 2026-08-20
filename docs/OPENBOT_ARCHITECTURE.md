# Governed Computer Architecture

Fathom now exposes an OpenBot-like governed computer surface without replacing its Rust agent runtime.

## Runtime

- `crates/governance` evaluates explicit allow/deny rules. Deny wins; no matching allow fails closed.
- `crates/agent` authorizes every tool call before execution and persists redacted authorization events.
- `crates/persistence` stores `audit_events`, durable `coworkers`, and `channels` in the existing SQLite database.
- `crates/server` exposes authenticated policy, audit, coworker/channel, computer relay, lifecycle, and AG-UI SSE endpoints.

Set `FATHOM_GOVERNANCE_ENABLED=true` to enforce the policy engine. The optional JSON policy is read from `FATHOM_GOVERNANCE_POLICY`, for example:

```json
{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"},{"effect":"deny","tool":"browser.type","path":"/admin/*"}]}
```

An empty or unmatched policy denies. Credentials and secret-like URL query values are redacted before audit persistence.

## Isolated browser

`apps/computer` is a loopback Playwright service. It owns a persistent Chromium profile/workspace, emits accessibility-tree snapshots with opaque refs, rejects stale refs, supports multiple tab-scoped snapshots, human control over `/control/ws`, and bot/human control leases. Browser egress rejects localhost, private/link-local/multicast/metadata targets by default; `COMPUTER_ALLOW_PRIVATE_HOSTS=true` is for local development only and never bypasses metadata or multicast denies.

Secrets are operator-only. The agent tool registry has no secret-input tool. The UI calls `POST /api/v1/computers/secret`; the relay adds `x-fathom-operator: true` and forwards to `/operator/secret`. Direct `/secret` is not available. Credentials stored through `/api/v1/credentials` are AES-256-GCM encrypted using `FATHOM_CREDENTIAL_KEY`; list responses never include plaintext.

Computer routes also expose `/tabs`, confined `/files` workspace operations, `/screen` screenshot streaming, and `/control/ws` input forwarding. Workspace writes are bounded and path-confined; human lease blocks bot actions.

`crates/supervisor` optionally provisions one Docker computer per agent with persistent workspace/profile volumes, loopback ports, restrictive capabilities, and health checks. Configure `COMPUTER_TOKEN`, `COMPUTER_IMAGE`, `COMPUTER_NETWORK`, and `COMPUTER_BASE_PORT`; the server exposes `/api/v1/computers/:agent_id/{ensure,stop,reset}` when Docker is available.

The server relay proxies the computer service and provides `/api/v1/computers/:agent_id/screen` as a bounded screenshot WebSocket. When the Docker supervisor is configured, agent-scoped routes call `ensure(agent_id)` and route to that container's loopback port; without Docker they use the single `FATHOM_COMPUTER_SERVICE_URL` fallback. The Tauri and Next.js surfaces provide live screen, human takeover, masked secret entry, policy editing, audit review, and computer lifecycle states.

## Durable product model

Coworkers are persisted profiles; channels link coworkers to optional Fathom sessions. REST routes are under `/api/v1/coworkers` and `/api/v1/channels`; scheduled coworker runs use `/api/v1/schedules` and are claimed atomically. `/api/v1/credentials` is write-only for plaintext and returns metadata only. `/api/v1/replay` returns only explicitly recorded redacted action rows. `/api/v1/observability/summary` reports bounded live counters and audit totals. Notifications accept symbolic configured channels only.

The `/api/v1/ag-ui/events` stream provides versioned event envelopes with bounded reconnect replay via `Last-Event-ID`. Desktop component responses are allowlisted and rendered as text; arbitrary code is never evaluated.

## Deployment configuration

For local single-computer mode:

```bash
COMPUTER_URL=http://127.0.0.1:8765
COMPUTER_TOKEN=$(openssl rand -hex 32)
```

For Docker per-agent computers, set `COMPUTER_IMAGE`, `COMPUTER_NETWORK`, `COMPUTER_TOKEN`, and `COMPUTER_BASE_PORT`; the main server does not need the Docker socket exposed to the browser service. For encrypted credentials, set `FATHOM_CREDENTIAL_KEY` to a 32-byte key encoded as 64 hex characters or base64. Never commit these values. In production use rootless Docker/gVisor or equivalent host isolation and keep all services loopback/private-network bound.

## Local startup

1. Start the Fathom server with `FATHOM_GOVERNANCE_ENABLED=true` and a policy JSON.
2. Start the computer service: `cd apps/computer && npm install && npm run start`.
3. Set `COMPUTER_URL=http://127.0.0.1:8765` for agent HTTP computer tools, or let the server relay use its default loopback URL.
4. Open the desktop app or Next.js web panel. Computer service and Docker are optional; surfaces show explicit offline states when unavailable.
