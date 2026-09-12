# Parallel & AI Agents Infrastructure Guide

Comprehensive guide to architecture, configuration, deployment, AI engines, and virtual machines (Local VM) for Parallel on the server `code.y7.hk`.

---

## 1. System Architecture Overview

```text
                                [ Internet / Clients ]
                                          │
                        (HTTPS 443 / WSS, port 80 -> 443)
                                          ▼
                      ┌────────────────────────────────────────┐
                      │    Nginx (aaPanel Reverse Proxy)       │
                      │   SSL: Let's Encrypt (/root/.acme.sh)  │
                      └───────────────────┬────────────────────┘
                                          │
                ┌─────────────────────────┼────────────────────────┐
                │                         │                        │
         http://127.0.0.1:8799     http://127.0.0.1:8800    http://127.0.0.1:6080
         (Web UI, REST, SSE)       (Webhooks / Ingress)     (noVNC Desktop Viewer)
                │                         │                        │
                ▼                         ▼                        ▼
    ┌───────────────────────────────────────────┐    ┌───────────────────────────┐
    │       Docker: parallel (Harness)          │    │  Docker: parallel-        │
    │  network_mode: "host"                     │    │          computer (VM)    │
    │  Volumes:                                 │    │  XFCE4 + Cua Driver       │
    │   - /opt/parallel-data:/data              │    │  noVNC Web Viewer (:6901) │
    │   - /var/run/docker.sock                  │    │  Workspace:               │
    │   - /data/.parallel                       │    │   /data/.parallel/        │
    │                                           │    │    vm-home                │
    │  AI Engines:                              │    └───────────────────────────┘
    │   - opencode (Helium API router)          │
    │   - hermes (Nous Research ACP)            │
    │   - pi (Pi Coding Agent JSON-RPC)         │
    │   - claude (Anthropic CLI)                │
    │   - grok (xAI CLI ACP)                    │
    │   - qwen (Qwen Code ACP)                  │
    │   - antigravity (Google ACP runtime)      │
    └─────────────────────┬─────────────────────┘
                          │ (OpenAI-compatible protocol)
                          ▼
            https://router.y7.hk/v1
            Key: sk-haus
            Models: Gemini 3.8/3.7, Claude Sonnet 4.6,
                    Qwen 3.8 Max, DeepSeek V4, Grok 4.6, etc.
```

---

## 2. Endpoints & Access

* **Web UI & API:** `https://code.y7.hk/`
* **noVNC Virtual Desktop Viewer:** `https://code.y7.hk/vnc.html` (auto-connect via `?autoconnect=true&resize=remote`)
* **API Router (Models):** `https://router.y7.hk/v1`
* **Host Ports:**
  * `8799` — Parallel server HTTP/REST/SSE (loopback only)
  * `8800` — Webhook ingress listener
  * `6080` — noVNC WebSocket & HTTP proxy
  * `80 / 443` — Public Nginx reverse proxy

---

## 3. Directory Layout on the Host

| Path | Purpose |
| :--- | :--- |
| `/opt/Parallel/deploy/` | Deployment repository, `docker-compose.yml`, `.env` |
| `/opt/parallel-data/` | Persistent container data volume (`/data` inside container) |
| `/opt/parallel-data/.parallel/` | Database `messages.db`, `config.json`, sessions, credentials |
| `/opt/parallel-data/.parallel/vm-home/` | Workspace folder for the virtual bot computer |
| `/opt/parallel-data/.local/bin/` | Installed engine binaries (`pi`, `hermes`, `claude`, `grok`, `qwen`, `antigravity`, `docker`, `uv`) |
| `/opt/parallel-data/.opencode/bin/` | `opencode` binary |
| `/opt/parallel-data/.pi/agent/` | Pi configuration (`models.json`, `auth.json`, `settings.json`) |
| `/opt/parallel-data/.hermes/` | Hermes configuration (`config.yaml`, `.env`) |
| `/opt/parallel-data/.qwen/` | Qwen Code configuration (`settings.json`) |
| `/opt/parallel-data/.grok/` | Grok configuration (`config.toml`, `auth.json`) |
| `/www/server/panel/vhost/nginx/code.y7.hk.conf` | Nginx virtual host configuration and reverse proxy |
| `/www/server/panel/vhost/cert/code.y7.hk/` | Let's Encrypt SSL certificates |

---

## 4. Nginx Reverse Proxy Configuration

Nginx on the host manages SSL termination and proxies requests to loopback ports:

* `/` -> `http://127.0.0.1:8799` (Parallel UI & API, supports `Upgrade: websocket` for SSE and events).
* `/api/events` -> `http://127.0.0.1:8799/api/events` (`proxy_buffering off`, `proxy_read_timeout 86400s` for real-time streams).
* `/websockify` -> `http://127.0.0.1:6080/websockify` (WebSocket proxy for noVNC desktop viewer).
* `/vnc.html`, `/core/`, `/app/` -> `http://127.0.0.1:6080` (static noVNC interface files).

---

## 5. Docker Compose Services

Configuration file: `/opt/Parallel/deploy/docker-compose.yml`

* `network_mode: "host"` — Parallel listens directly on `127.0.0.1:8799` on the host network stack.
* Volume mounts:
  * `/opt/parallel-data:/data` (data, configurations, home directory of user `parallel`)
  * `/opt/parallel-data/dist-server:/app/dist-server` (server bundle override)
  * `/var/run/docker.sock:/var/run/docker.sock` (access to host Docker daemon for VM lifecycle management)
* `group_add: ["986"]` — access to Docker socket.
* Router environment variables:
  * `ROUTER_BASE_URL=https://router.y7.hk/v1`
  * `ROUTER_API_KEY=sk-haus`
  * API keys for all compatible drivers configured to `sk-haus`.

---

## 6. AI Engines & Driver Profiles

All engines are pre-configured to communicate via `https://router.y7.hk/v1`.

### 6.1. OpenCode Go
* CLI binary: `/data/.opencode/bin/opencode` (v1.2.14)
* Configuration: `/data/.config/opencode/opencode.json`
* Provider: `router` (`https://router.y7.hk/v1`, models: `qwen-3.8-plus`, `antigravity/gemini-3.8-flash-high`, `deepseek-v4-flash`, etc.)
* Auth: `/data/.local/share/opencode/auth.json`

### 6.2. Hermes Agent
* ACP server binary: `/data/.local/bin/hermes` (v0.6.0)
* Runtime layout: uv virtualenv at `/usr/local/lib/hermes-agent/`
* Configuration: `/data/.hermes/config.yaml`
* Provider: `router` (OpenAI-compatible)

### 6.3. Pi Coding Agent
* CLI binary: `/data/.local/bin/pi` (v0.52.12)
* Configuration: `/data/.pi/agent/models.json` & `/data/.pi/agent/settings.json`
* Provider: `router` (endpoint: `https://router.y7.hk/v1`, API key: `sk-haus`)

### 6.4. Grok CLI
* ACP server binary: `/usr/local/bin/grok` (x.ai standalone binary)
* Configuration: `/data/.grok/config.toml` & `/data/.grok/auth.json`
* Models mapped into `[model."..."]` blocks routing to `https://router.y7.hk/v1`.

### 6.5. Qwen Code
* ACP server binary: `/data/.local/bin/qwen` (v0.1.13)
* Configuration: `/data/.qwen/settings.json`
* OpenAI provider routing configured to `https://router.y7.hk/v1`.

### 6.6. Google Antigravity
* Official executable: `/data/.local/bin/agy_acp_server.par`, `localharness_external`
* Location: `/data/.parallel/tools/antigravity-acp/linux-x64/versions/38f62d01b32deb0907b3d39a71ec301fd36369f6ffd1cf262d4af385177f79df/`

### 6.7. Claude Code
* Binary: `/data/.local/bin/claude` (v2.1.269, `@anthropic-ai/claude-code`)
* Settings: `/data/.claude/settings.json` with `ANTHROPIC_BASE_URL=https://router.y7.hk/v1` and custom router models.

---

## 7. Virtual Bot Desktops (Local VM / Cua)

### 7.1. Bot Desktop Architecture
1. **Base image:** `docker.io/trycua/xfce-cua@sha256:274eb636f5cf3fc58f705916ee72b7a701270b3877369d08533a385c5325be9b`
2. **Final image:** `localhost/parallel/cua-local-vm:driver-0.20.0-v5`
   * Contains XFCE4 Desktop, Chromium, `cua-driver 0.20.0`, and Noto CJK fonts.
   * `cua-driver` runs under supervisor and listens on socket `/run/user/1000/parallel-cua.sock`.
   * noVNC server listens on port `6901` internally and forwards to host `127.0.0.1:6080`.
3. **Container security:**
   * `--cap-drop ALL --cap-add SETUID --cap-add SETGID`
   * Memory: 4 GB, CPUs: 2, PIDs limit: 512, shm-size: 512m.
   * Workspace directory: `/home/cua/workspace` mounted to `/data/.parallel/vm-home`.

### 7.2. VM Management Commands
```bash
# Restart the computer container
docker restart parallel-computer

# View virtual computer logs
docker logs --tail 50 parallel-computer

# Check availability status via API
curl -s http://127.0.0.1:8799/api/local-computer | jq .

# Capture a screenshot of the bot's screen
curl -s -X POST http://127.0.0.1:8799/api/local-computer/screenshot
```

---

## 8. Management & Diagnostic Commands

### 8.1. Pairing the First Device with Parallel
```bash
# Generate a pairing code for your browser
docker exec parallel node dist-server/openmausbot.js pair --public-url https://code.y7.hk

# View active sessions
docker exec parallel node dist-server/openmausbot.js sessions
```

### 8.2. Viewing Server Logs
```bash
# Parallel server logs
docker logs -f --tail 50 parallel
```

### 8.3. Checking & Reloading Nginx
```bash
/www/server/nginx/sbin/nginx -t && /www/server/nginx/sbin/nginx -s reload
```
