# Deployment & Orchestration

This directory contains containerization, reverse proxying, and deployment manifests for self-hosting Parallel (formerly Parallel / OMB) across Docker, rootless Podman, and bare-metal/edge setups.

---

## Directory Architecture & Roles

```text
deploy/
├── Caddyfile              # Standalone edge reverse proxy configuration for domain routing
├── docker-compose.yml     # Production Docker Compose definition for single-host deployments
├── docker-entrypoint.sh   # Container init & permission alignment script
├── local/                 # Developer/local compose configurations
│   ├── Dockerfile         # Local test container definition
│   ├── Caddyfile          # Local edge proxy
│   └── README.md          # Local compose guide
└── podman/                # Rootless Podman & WSL2 multi-desktop orchestration
    ├── compose.yaml       # Podman compose stack (omb + caddy)
    ├── Containerfile      # Rootless container image recipe
    ├── Caddyfile          # Internal loopback / Tailscale Caddy configuration
    ├── setup.sh           # Environment bootstrap & UID/GID mapping script for Linux
    ├── parallel.ps1       # PowerShell wrapper managing WSL2 Podman machine & compose
    └── README.md          # Rootless Podman architecture documentation
```

---

## Orchestration Topologies

Parallel runs as a Node.js daemon (default port `8799`) accompanied by an internal webhook receiver (default port `8800`). It is fronted by Caddy for automatic TLS, header sanitization, and Webhook ingress routing.

```text
Public Internet / Tailnet
         │
         ▼
┌──────────────────┐
│  Caddy Edge      │ (Port 80/443 or 8080)
└────────┬─────────┘
         │
         ├── /hooks/* ──────────► [ Parallel Webhook Listener :8800 ]
         │
         └── /* ────────────────► [ Parallel Core Server     :8799 ]
                                              │
                                              ▼
                               [ Rootless Podman / Docker Socket ]
                                              │
                                              ▼
                               [ Per-Bot Linux Desktops & Workspace ]
```

### 1. Production Docker (`deploy/docker-compose.yml`)
- **Network Mode**: `host` networking for low-latency loopback communication and direct access to dynamically mapped desktop ports.
- **Permission Mapping**:
  - `group_add: ["986"]` ensures access to `/var/run/docker.sock` for spawning sidecar desktop containers.
  - `docker-entrypoint.sh` inspects container invocation: if running as UID `0` (root), it establishes ownership of `/data` to the unprivileged `parallel` (or legacy `parallel`) user, invokes `/app/scripts/setup-router-agents.mjs`, and steps down via `su "$TARGET_USER"` before running the server command.
- **Volumes**:
  - `/opt/parallel-data:/data`: Root storage mount containing configurations (`.parallel`), workspaces, CLI credentials, and SQLite database.
  - `/opt/parallel-data/dist-server` & `/opt/parallel-data/dist`: Pre-compiled server and client bundles for zero-downtime hot reloading.
  - `/var/run/docker.sock`: Direct socket access for agent container management.

### 2. Rootless Podman (`deploy/podman/`)
Designed for non-root systems and Windows WSL2 environments.
- **User Namespace**: `userns_mode: keep-id:uid=1001,gid=1001` matches the host user to the container's unprivileged user (`1001:1001`).
- **Isolation & Security Boundary**:
  - The Podman socket (`/run/user/$UID/podman/podman.sock` or Windows WSL `/run/omb-podman.sock`) is mounted into the container.
  - Per-bot desktops (`cua` user, UID 1000) and the server share the exact same filesystem path on the host filesystem via `${OMB_DATA_ROOT}`.
  - Avoid adding `:U` or running recursive chown across data roots to prevent desynchronizing user namespace mappings.

---

## Caddy Edge Routing & Security Rules

The Caddy configuration (`deploy/Caddyfile`) acts as the security edge:
1. **Webhook Bypassing**:
   ```caddy
   handle /hooks/* {
       reverse_proxy 127.0.0.1:8800
   }
   ```
   Webhooks have their own per-hook signature secrets and are routed directly to the webhook port (`8800`) without session gatekeeping.
2. **Core Server Ingress & Pairing Gate**:
   ```caddy
   handle {
       reverse_proxy 127.0.0.1:8799 {
           flush_interval -1
       }
   }
   ```
   - `flush_interval -1` is mandatory for real-time Server-Sent Events (SSE) streaming without proxy buffering.
   - Remote browsers attempting access without a valid session cookie are presented with the pair screen; unauthorized access is rejected at the application level.
   - Caddy sets `X-Forwarded-For` and `X-Forwarded-Proto`, which Parallel validates for pairing lockout policies and same-origin defenses.

---

## Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `DOMAIN` | `code.y7.hk` | Hostname for automatic TLS certificates via Let's Encrypt / ZeroSSL |
| `OMB_PORT` | `8799` | HTTP port for core application and API services |
| `OMB_WEBHOOK_PORT` | `8800` | Port for incoming third-party webhook ingestion |
| `OMB_PUBLIC_URL` | `https://${DOMAIN}` | External base URL sent to paired devices and oauth callbacks |
| `OMB_WEBHOOK_PUBLIC_URL` | `https://${DOMAIN}` | Base URL configured on webhooks |
| `OMB_DATA_DIR` | `/data/.parallel` | Path where databases, workspaces, and keys reside |
| `OMB_SIGNIN_EMAILS` | _(empty)_ | Allowed owner email accounts for SSO / email gate |
| `OMB_SIGNIN_MEMBER_EMAILS` | _(empty)_ | Allowed team member email accounts |
| `ROUTER_API_KEY` / `ROUTER_BASE_URL` | `https://router.y7.hk/v1` | LLM router configuration for model access |
| `CONTAINER_HOST` | _(empty)_ | Socket URI (`unix:///run/omb-podman.sock`) for Podman container orchestration |

---

## Operations & Verification

1. **Start Services**:
   ```sh
   docker compose -f deploy/docker-compose.yml up -d --build
   ```
2. **Health Check Inspection**:
   Container health is probed every 30 seconds:
   ```sh
   curl -sf http://127.0.0.1:8799/api/health | grep -qE 'parallel|parallel'
   ```
3. **Generate Pairing Credentials**:
   To pair a browser or companion client with the containerized instance:
   ```sh
   docker compose exec omb node dist-server/parallel.js pair
   ```
4. **Log Inspection**:
   ```sh
   docker compose logs -f omb
   ```
