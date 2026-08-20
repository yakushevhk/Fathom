# Computer Use

> A real browser the agent operates — Playwright loopback service, accessibility-tree snapshots, human takeover, screen streaming, Docker supervisor.

Fathom's computer use capability (`apps/computer`) gives agents a **real browser-based computer** they can operate just like a human: navigate, click, type, press keys, take screenshots, and manage files. Each agent can be provisioned an **isolated computer** with its own persistent workspace, browser profile, and network namespace.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Fathom Server                        │
│  ┌──────────────┐    ┌──────────────────────────────┐    │
│  │ Agent Runtime │───▶│  Computer Client (tools)     │    │
│  │               │    │  computer_snapshot           │    │
│  │               │    │  computer_navigate           │    │
│  │               │    │  computer_click              │    │
│  │               │    │  computer_type               │    │
│  │               │    │  computer_key                │    │
│  │               │    │  computer_screenshot         │    │
│  └──────────────┘    └──────────┬───────────────────┘    │
│                                 │                        │
│                          HTTP relay                      │
│                    /api/v1/computers/:agent_id/*          │
└─────────────────────────────────┬────────────────────────┘
                                  │
┌─────────────────────────────────▼────────────────────────┐
│              Computer Service (apps/computer)              │
│                                                           │
│  ┌────────────────┐    ┌──────────────────────────────┐   │
│  │ Playwright      │    │  File Workspace             │   │
│  │ Browser Control │    │  (confined, bounded)        │   │
│  │                 │    │                              │   │
│  │ · Navigation    │    │  · Read/write files         │   │
│  │ · Click/Type    │    │  · Path confinement         │   │
│  │ · Key press     │    │  · Size limits              │   │
│  │ · Screenshot    │    └──────────────────────────────┘   │
│  │ · Snapshot      │                                       │
│  └────────────────┘    ┌──────────────────────────────┐   │
│                         │  Control / Screen            │   │
│  ┌────────────────┐    │                              │   │
│  │ Accessibility   │    │  · /control/ws — human      │   │
│  │ Tree Snapshots  │    │    takeover WebSocket       │   │
│  │ with opaque     │    │  · /screen — screen         │   │
│  │ refs            │    │    streaming WebSocket      │   │
│  └────────────────┘    └──────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

---

## Computer Tools

The agent interacts with the computer through 6 dedicated tools, exposed via the server's relay at `/api/v1/computers/:agent_id/`.

### `computer_snapshot`

Takes an accessibility-tree snapshot of the current page. The agent "sees" the page like a screen-reader — a structured tree of interactive elements with **opaque refs** that the agent uses to interact (never brittle CSS selectors).

| Parameter | Type | Description |
|-----------|------|-------------|
| `tab` | string? | Tab identifier (default: active tab) |

**Returns**: accessibility tree with elements annotated with `[ref=eN]` identifiers. Multiple tab-scoped snapshots are supported; stale refs are rejected with a clear error.

### `computer_navigate`

Navigates the browser to a URL.

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | string | URL to navigate to |
| `tab` | string? | Tab identifier (default: active tab) |

**Returns**: navigation result with page title and URL.

### `computer_click`

Clicks an element identified by its accessibility ref.

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Accessibility ref (e.g., `e5`) |
| `tab` | string? | Tab identifier |

### `computer_type`

Types text into an element identified by its accessibility ref.

| Parameter | Type | Description |
|-----------|------|-------------|
| `ref` | string | Accessibility ref |
| `text` | string | Text to type |
| `tab` | string? | Tab identifier |

### `computer_key`

Presses a key or key combination (e.g., `Enter`, `Control+C`, `Escape`).

| Parameter | Type | Description |
|-----------|------|-------------|
| `key` | string | Key or key combination |
| `tab` | string? | Tab identifier |

### `computer_screenshot`

Takes a screenshot of the current page.

| Parameter | Type | Description |
|-----------|------|-------------|
| `full_page` | bool? | Full-page screenshot (default: false) |
| `tab` | string? | Tab identifier |

**Returns**: base64-encoded PNG image.

---

## Accessibility-Tree Snapshots

Unlike CDP-based browser automation that relies on CSS selectors, the computer service uses **accessibility-tree snapshots** with opaque refs. This approach:

- **Matches how screen-readers see the page** — the tree contains only meaningful interactive elements
- **Eliminates brittle selectors** — refs are stable within a snapshot and scoped to that snapshot
- **Supports multiple tabs** — each tab has its own snapshot scope
- **Rejects stale refs** — if the page changes after a snapshot, old refs return a clear error

The snapshot format:

```yaml
url: https://example.com
title: Example Domain
snapshot:
  - ref: e1
    role: link
    name: "More information..."
    href: https://iana.org/domains/example
  - ref: e2
    role: heading
    name: "Example Domain"
    level: 1
  - ref: e3
    role: text
    name: "This domain is for use in illustrative examples..."
```

---

## Human / Bot Control Leases

The control WebSocket at `/control/ws` allows a human operator to **take over** the browser at any time. This is a lease-based system:

1. **Agent mode** — the agent controls the browser exclusively
2. **Human takeover** — the operator connects via `/control/ws`, acquires the lease, and the agent is notified
3. **Agent resumes** — when the operator releases the lease, control returns to the agent

The WebSocket protocol:

```
Agent connected → ["mode", "agent"]
Human connects  → ["mode", "human", "operator_id"]
Human releases  → ["mode", "agent"]
```

Human takeover is useful for:
- Unblocking the agent on a form that requires human judgment
- Demonstrating a UI flow to the agent
- Debugging a failed interaction

---

## Screen Streaming

The `/screen` WebSocket endpoint streams the browser's viewport as a real-time video feed. The stream is:

- **Low-latency** — sub-second frame delivery
- **Adaptive** — frame rate adjusts to network conditions
- **Secure** — only accessible to authenticated operators

The screen stream is used by:
- **Tauri v2 desktop app** (`apps/desktop`) — native window showing the live browser
- **Next.js 16 web dashboard** (`apps/web`) — embedded in the dashboard panel

---

## Confined File Workspace

Each computer has a **confined file workspace** — a bounded, path-confined directory where the agent can read and write files.

- **Path confinement** — all file operations are restricted to the workspace directory; symlinks and path traversal attempts are blocked
- **Size limits** — total workspace size is bounded (configurable, default 100 MB)
- **Persistence** — the workspace is persisted across agent restarts (within the same Docker container)

---

## Browser Egress Guard

The computer service enforces strict egress controls on the browser:

| Target | Default | Notes |
|--------|---------|-------|
| Public internet | ✅ Allowed | Standard web browsing |
| localhost / 127.0.0.1 | ❌ Denied | SSRF protection |
| Private RFC1918 ranges | ❌ Denied | 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 |
| Link-local / metadata | ❌ Denied | 169.254.0.0/16, 100.100.100.200 (cloud metadata) |
| Multicast | ❌ Denied | 224.0.0.0/4 |

The `COMPUTER_ALLOW_PRIVATE_HOSTS` environment variable can override this for local development:

```bash
# Allow localhost and private ranges (development only)
COMPUTER_ALLOW_PRIVATE_HOSTS=true fathom serve
```

**This never bypasses metadata/multicast denies** — even with `COMPUTER_ALLOW_PRIVATE_HOSTS=true`, cloud metadata endpoints and multicast addresses remain blocked.

---

## Docker Supervisor

The supervisor crate (`crates/supervisor`) provisions **one isolated computer per agent** using Docker containers. Each container has:

- **Persistent workspace volume** — per-agent filesystem
- **Persistent browser profile** — cookies, sessions, extensions survive restarts
- **Loopback network** — isolated from other agent computers
- **Restrictive capabilities** — no `--privileged`, no host networking, limited syscalls
- **Health checks** — liveness probe on the computer service port

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `COMPUTER_IMAGE` | `ghcr.io/fathom/computer:latest` | Docker image for the computer container |
| `COMPUTER_NETWORK` | `fathom-computer` | Docker network for computer containers |
| `COMPUTER_BASE_PORT` | `9200` | Base port for loopback mapping (each agent gets `base_port + agent_index`) |
| `COMPUTER_TOKEN` | *(auto-generated)* | Shared secret for authenticating computer service requests |

### Container lifecycle

1. **Provision** — when an agent spawns with computer use, the supervisor pulls the image (if not cached) and starts a container
2. **Connect** — the agent receives the container's loopback URL and authentication token
3. **Use** — all computer tool calls are routed to the container
4. **Teardown** — when the agent finishes or is cancelled, the container is stopped and removed

### API routes

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/computers/:agent_id/snapshot` | Accessibility-tree snapshot |
| `POST` | `/api/v1/computers/:agent_id/navigate` | Navigate to URL |
| `POST` | `/api/v1/computers/:agent_id/click` | Click an element by ref |
| `POST` | `/api/v1/computers/:agent_id/type` | Type text into an element |
| `POST` | `/api/v1/computers/:agent_id/key` | Press a key |
| `POST` | `/api/v1/computers/:agent_id/screenshot` | Take a screenshot |
| `GET` | `/api/v1/computers/:agent_id/screen` | Screen streaming WebSocket |
| `GET/POST` | `/api/v1/computers/:agent_id/files/*` | File workspace operations |
| `GET` | `/api/v1/computers/:agent_id/control` | Human takeover WebSocket |
| `POST` | `/api/v1/computers/:agent_id/ensure` | Ensure the computer is running (start if not) |
| `POST` | `/api/v1/computers/:agent_id/stop` | Stop the computer container |
| `POST` | `/api/v1/computers/:agent_id/reset` | Reset the computer (clear workspace, restart browser) |

---

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FATHOM_COMPUTER_SERVICE_URL` | `http://localhost:9100` | URL of the computer service (used by the server relay) |
| `COMPUTER_SERVICE_URL` | `http://localhost:9100` | URL of the computer service (internal) |
| `COMPUTER_TOKEN` | *(auto-generated)* | Authentication token for computer service requests |
| `COMPUTER_IMAGE` | `ghcr.io/fathom/computer:latest` | Docker image for supervisor-provisioned containers |
| `COMPUTER_NETWORK` | `fathom-computer` | Docker network name |
| `COMPUTER_BASE_PORT` | `9200` | Base port for loopback mapping |
| `COMPUTER_ALLOW_PRIVATE_HOSTS` | `false` | Allow private/localhost targets (development only) |

---

## Related

- [ARCHITECTURE.md](ARCHITECTURE.md) — crate diagram and supervisor crate description
- [TOOLS.md](TOOLS.md) — computer tool reference
- [HTTP-API.md](HTTP-API.md) — computer API endpoints
- [CONFIGURATION.md](CONFIGURATION.md) — computer env vars
- [crates/supervisor.md](crates/supervisor.md) — detailed supervisor crate documentation