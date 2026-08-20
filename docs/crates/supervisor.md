# Crate Documentation `crates/supervisor`

The `supervisor` crate (`pr-supervisor`) provides a **Docker-backed lifecycle supervisor** for per-agent computer environments. It manages one Docker container per agent, each with a dedicated workspace volume, profile volume, loopback port, restrictive capabilities, and a health check. The Docker client is intentionally private to the crate — callers can only create, stop, reset, and inspect containers.

> For the end-to-end governed-computer architecture (isolated browser service, relay, durable product model, deployment), see [OPENBOT_ARCHITECTURE.md](../OPENBOT_ARCHITECTURE.md). This page documents the crate itself.

## Overview

| Capability | Description |
|---|---|
| **Container lifecycle** | `ensure` (create-or-start), `stop`, `reset` (stop + remove container, retain volumes), `list` (all managed containers). |
| **Per-agent isolation** | Each agent gets its own container, workspace volume, profile volume, and loopback port. |
| **Deterministic naming** | Container name, volume names, and port offset are derived from a FNV-1a hash of the agent ID — no shared mutable state. |
| **Health checks** | Docker health check polling the container's `/health` endpoint. |
| **Env-driven config** | `SupervisorConfig::from_env()` reads `COMPUTER_IMAGE`, `COMPUTER_NETWORK`, `COMPUTER_TOKEN`, `COMPUTER_BASE_PORT`. |
| **Agent ID validation** | `validate_agent_id` — rejects empty, too-long, or non-ASCII-alphanumeric ids (Docker name/label safety). |

## File structure and dependencies

| File | Purpose |
|------|---------|
| `src/lib.rs` | All types and logic: `SupervisorConfig`, `ComputerSupervisor`, `AgentContainer`, `Names`, validation, tests (single file, ~250 lines) |

Dependencies: `bollard` (Docker daemon client), `tokio` (async, timeout), `serde`, `thiserror`, `tracing`.

---

## 1. Configuration — `SupervisorConfig`

```rust
pub struct SupervisorConfig {
    pub image: String,          // Docker image, default "fathom/computer:latest"
    pub network: String,        // Docker network, default "fathom-computer"
    pub token: String,          // Authentication token for the computer service
    pub base_port: u16,         // Base host port, default 19_000
    pub container_port: u16,    // Container internal port, default 8765
}
```

### Constructors

| Constructor | Behaviour |
|---|---|
| `SupervisorConfig::new(image, network, token, base_port)` | Validates all fields via `validate()`; returns `Err` on invalid config. |
| `SupervisorConfig::from_env()` | Reads `COMPUTER_IMAGE` (default `fathom/computer:latest`), `COMPUTER_NETWORK` (default `fathom-computer`), `COMPUTER_TOKEN` (required, errors if missing), `COMPUTER_BASE_PORT` (default `19000`). Calls `new()` after reading. |
| `SupervisorConfig::default()` | All defaults, token empty (caller must set before use). |

### Validation (`validate`)

- `image` must not be empty.
- `network` must pass `valid_docker_component` (≤63 chars, only ASCII alphanumeric / `-` / `_` / `.`).
- `token` must not be empty.
- `base_port` and `container_port` must not be zero.

The `Debug` impl redacts the token field: `"[REDACTED]"`.

## 2. `ComputerSupervisor` — Docker lifecycle operations

```rust
pub struct ComputerSupervisor {
    docker: Arc<Docker>,    // bollard Docker client
    config: SupervisorConfig,
}
```

### Construction

| Constructor | Behaviour |
|---|---|
| `ComputerSupervisor::new(config)` | Validates config, connects to local Docker daemon (`Docker::connect_with_local_defaults`). |
| `ComputerSupervisor::from_env()` | Calls `SupervisorConfig::from_env()` then `new()`. |

### Lifecycle methods

#### `ensure(agent_id) -> Result<AgentContainer>`

Create-or-start the container for an agent:

1. **Validate** the agent ID via `validate_agent_id`.
2. **Derive names** from the stable hash: `Names::for_agent(agent_id, base_port)`.
3. **Inspect** existing managed container by name:
   - Exists and running → return metadata immediately.
   - Exists but stopped → start it, return metadata.
   - Does not exist → create and start a new container.
4. **Container creation** config:
   - Image: `config.image` (e.g. `fathom/computer:latest`).
   - Env: `COMPUTER_TOKEN={config.token}`, `COMPUTER_WORKSPACE=/data/browser`.
   - Labels: `io.fathom.supervisor=true`, `io.fathom.agent-id={agent_id}`.
   - Exposed port: `{container_port}/tcp`, bound to `127.0.0.1:{assigned_port}`.
   - Network: `config.network`.
   - Mounts: workspace volume at `/data/browser`, profile volume at `/data/profile` (both Docker volumes, persistent across resets).
   - Capabilities: `cap_drop: ["ALL"]` — drop all Linux capabilities.
   - Security opt: `no-new-privileges:true`.
   - Health check: `CMD-SHELL node -e "fetch('http://127.0.0.1:{container_port}/health')..."` / 5s interval / 3 retries / 10s start period.
5. **Common timeout**: all Docker operations are wrapped in a 20s timeout (`OP_TIMEOUT`). `SupervisorError::Timeout` on expiry.

#### `stop(agent_id) -> Result<()>`

Stop the container (graceful, 10s timeout). 404 (already gone) is treated as success.

#### `reset(agent_id) -> Result<()>`

**Stop and remove** the container while retaining the named workspace and profile volumes. Volumes persist so an agent's data survives container recreation. 404 is treated as success.

#### `list() -> Result<Vec<AgentContainer>>`

List all containers managed by this supervisor (filtered by label `io.fathom.supervisor=true`). Returns `AgentContainer` metadata for each.

### `AgentContainer` — metadata

```rust
pub struct AgentContainer {
    pub agent_id: String,
    pub container_name: String,
    pub workspace_volume: String,
    pub profile_volume: String,
    pub port: u16,             // Host loopback port
    pub running: bool,
    pub health: Option<String>, // Raw Docker status string from ContainerSummary
}
```

## 3. Agent ID validation — `validate_agent_id`

```rust
pub fn validate_agent_id(agent_id: &str) -> Result<&str, SupervisorError>
```

Rejects input if it:
- Is empty,
- Exceeds 64 characters,
- Contains non-ASCII-alphanumeric characters (only `a-z`, `A-Z`, `0-9`, `-`, `_`, `.` allowed),
- Starts or ends with `'.'`.

These constraints guarantee that the ID is safe for use in Docker container names and label values.

## 4. Internal naming — `Names`

```rust
struct Names {
    container: String,        // "fathom-computer-{hash}"
    workspace_volume: String, // "fathom-workspace-{hash}"
    profile_volume: String,   // "fathom-profile-{hash}"
    port: u16,                // base_port + (hash % 1000)
}
```

- **Hash**: FNV-1a 64-bit hash of the agent ID, formatted as 16 hex characters (`stable_agent_hash`).
- **Port**: `base_port + (first 4 hex chars parsed as u16) % 1000`. Deterministic per agent ID — no port registry needed.
- The hash is deterministic and does **not** expose the raw agent ID in container/volume names.

## 5. Health checks

Each container receives a Docker `HealthConfig`:

| Field | Value |
|---|---|
| `test` | `CMD-SHELL node -e "fetch('http://127.0.0.1:{container_port}/health').then(r => process.exit(r.ok ? 0 : 1)).catch(() => process.exit(1))"` |
| `interval` | 5 seconds |
| `timeout` | 2 seconds |
| `retries` | 3 |
| `start_period` | 10 seconds |

The health status from `Docker::list_containers` is surfaced in `AgentContainer::health`.

## 6. Integration with the server relay

The `pr-server` crate exposes the supervisor lifecycle through REST endpoints:

| Endpoint | Action |
|---|---|
| `POST /api/v1/computers/:agent_id/ensure` | Calls `ComputerSupervisor::ensure(agent_id)`, returns container metadata. |
| `POST /api/v1/computers/:agent_id/stop` | Calls `ComputerSupervisor::stop(agent_id)`. |
| `POST /api/v1/computers/:agent_id/reset` | Calls `ComputerSupervisor::reset(agent_id)`. |
| `GET /api/v1/computers` | Calls `ComputerSupervisor::list()`, returns all agent containers. |

The server relay also proxies the computer service: when the Docker supervisor is configured, agent-scoped routes route to the container's loopback port; without Docker they use the single `FATHOM_COMPUTER_SERVICE_URL` fallback.

## 7. Environment variables

| Variable | Required | Default | Purpose |
|---|---|---|---|
| `COMPUTER_TOKEN` | **Yes** | — | Authentication token for the computer service inside the container. |
| `COMPUTER_IMAGE` | No | `fathom/computer:latest` | Docker image for the per-agent browser container. |
| `COMPUTER_NETWORK` | No | `fathom-computer` | Docker network to attach containers to. |
| `COMPUTER_BASE_PORT` | No | `19000` | Base host port; per-agent port = `base_port + (hash % 1000)`. |
| `FATHOM_COMPUTER_SERVICE_URL` | No | `http://127.0.0.1:8765` | Fallback single-computer URL when Docker supervisor is not used. |

## 8. Edge cases and design rationale

| Concern | Behaviour |
|---|---|
| Container already running | `ensure` returns metadata immediately (no-op recreate). |
| Container exists but stopped | `ensure` starts it and returns metadata. |
| Container removed externally | `inspect_managed` returns `None` → fresh container is created. |
| Stop on non-existent container | 404 → treated as success (idempotent). |
| Reset on non-existent container | 404 → treated as success (volumes survive). |
| Invalid agent ID | `validate_agent_id` returns `Err`; no Docker API call is made. |
| Port exhaustion | `checked_add` returns `None` → `Err(InvalidConfig)` (base port range exhausted). |
| Docker daemon unavailable | `bollard` errors propagate as `SupervisorError::Docker`. |
| Timeout (20s) | All Docker operations timeout → `SupervisorError::Timeout`. |
| Volumes persist across resets | `reset` removes the container but keeps the named volumes; `ensure` reuses them. |
| Token redacted in debug | `SupervisorConfig::fmt` redacts the token; `tracing` logs never leak secrets. |

## 9. Tests

The `tests` module in `lib.rs` covers:

| Test | What it verifies |
|---|---|
| `validates_ids` | `validate_agent_id` accepts legal IDs (alphanumeric, dashes, dots) and rejects empty, path-traversal (`../escape`), and slash-containing IDs. |
| `names_are_deterministic_and_private` | Same agent ID produces the same container name and port; the hash does not contain the raw agent ID. |