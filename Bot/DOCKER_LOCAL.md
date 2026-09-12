# Local Docker Deployment Guide

Complete guide for building, running, and pairing **Parallel** locally using Docker and Docker Compose.

---

## Quick Start

Run from repository root:

```bash
# 1. Build the local engine image from source
docker build --platform linux/amd64 -t parallel:local .

# 2. Start the services (server + caddy reverse proxy)
docker compose up -d --build

# 3. Generate a pairing code for your browser/device
docker compose exec omb node dist-server/openmausbot.js pair --label "Chrome on Mac"
```

Then open **http://localhost:8080** and enter the pairing code (or click the URL printed in terminal).

---

## 2. Architecture & Components

```text
[ Browser / Client ]
        │
   (HTTP :8080)
        ▼
┌────────────────────────────────────────┐
│        Caddy Reverse Proxy             │  (image: caddy:2)
│  - Routes / to Parallel app            │
│  - Network mode: service:omb           │
└──────────────────┬─────────────────────┘
                   │
                   │ (internal network)
                   ▼
┌────────────────────────────────────────┐
│     parallel-omb-1 (Main Server)       │  (image: parallel-local:latest)
│  - Node.js 24 + Express + SSE Events   │
│  - Loopback mutation authentication    │
│  - Port 8799: App & REST API           │
│  - agent-browser / Chromium installed  │
│  - AI CLI engines (@anthropic-ai, etc) │
│  - Persistent Volume: /data            │
└────────────────────────────────────────┘
```

### Why two containers?
- **`omb`**: Contains the core application logic, session store (`sessions.json`), message database (`messages.db`), AI agent drivers, and embedded browser runner. It strictly binds to loopback (`127.0.0.1:8799`) for security.
- **`caddy`**: Runs in the network namespace of `omb` (`network_mode: service:omb`). It exposes port `8080` to the host machine and securely proxies requests to `127.0.0.1:8799`.

---

## 3. How Authentication & Pairing Works

### The Security Model
Parallel utilizes a **loopback capability token** (`OMB_MUTATION_TOKEN` / `mutation-token`) and **time-limited pairing codes**:
1. **Mutation Protection**:
   - The server verifies requests modifying sensitive configuration or creating pairing codes.
   - It requires the mutation token matching `OMB_MUTATION_TOKEN` or reads the token from `/data/.parallel/mutation-token` (or `/data/.openmausbot/mutation-token`).
   - Pairing codes are 12-character alphanumeric tokens (e.g., `HS3V-TVZ6-6FU3`).
   - Valid for **5 minutes** and single-use only.
   - When entered on `http://localhost:8080/pair`, the server exchanges it for a permanent session token (`omb_sess_...`), stored as an HTTP cookie / local storage.

### Generating Pairing Codes
To connect a new browser, mobile device, or client:
```bash
# Connect with a specific name/label
docker compose exec omb node dist-server/openmausbot.js pair --label "My Laptop"
```

The CLI prints:
- 12-character code
- Direct pairing URL (`http://localhost:8080/pair#code=...`)
- Terminal QR code for mobile camera scanning

---

## 4. Useful Management Commands

### Viewing Logs
```bash
# View real-time logs of the server
docker compose logs -f omb

# View Caddy proxy logs
docker compose logs -f caddy
```

### Checking Container Health
```bash
docker compose ps
```

Expected status:
```text
NAME                  IMAGE                      STATUS
parallel-caddy-1      caddy:2                    Up (healthy)
parallel-omb-1        parallel-local:latest      Up (healthy)
```

### Stopping & Restarting
```bash
# Stop containers without losing data
docker compose stop

# Restart containers
docker compose start

# Tear down containers (keeps data volume intact)
docker compose down

# Wipe data completely (fresh install)
docker compose down -v
```

### Checking Active Sessions
```bash
docker compose exec omb node dist-server/openmausbot.js sessions
```

### Revoking a Session
```bash
docker compose exec omb node dist-server/openmausbot.js sessions --revoke <SESSION_ID>
```

---

## 5. Troubleshooting

### 1. `no match for platform in manifest: not found`
- **Cause**: Apple Silicon (M1/M2/M3/M4) running ARM64 while some binary dependencies (such as Chrome for Testing or prebuilt wheels) target `linux/amd64`.
- **Solution**: The `compose.yaml` has `platform: linux/amd64` enabled so Docker/OrbStack emulates it seamlessly.

### 2. `server refused to mint a pairing code: forbidden`
- **Cause**: The CLI inside the container did not provide the mutation token to authenticate against the running server.
- **Solution**: Handled automatically in `server/cli.ts` and `compose.yaml` via `OMB_MUTATION_TOKEN` and persistence into `/data/.parallel/mutation-token` (or legacy `/data/.openmausbot/mutation-token`).
