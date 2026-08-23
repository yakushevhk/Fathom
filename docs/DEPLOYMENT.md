# Deployment Guide

Production deployment options and operational procedures for the Fathom HTTP API server.

---

## Table of Contents

- [Deployment Options](#deployment-options)
- [Server Configuration](#server-configuration)
- [Reverse Proxy](#reverse-proxy)
- [Systemd Service](#systemd-service)
- [Monitoring](#monitoring)
- [Backup & Recovery](#backup--recovery)
- [Security Hardening](#security-hardening)
- [Scaling](#scaling)

---

## Deployment Options

### Binary (Recommended for Production)

Build the release binary on the target machine (or cross-compile) and run it directly:

```bash
git clone <repo-url> fathom
cd fathom
cargo build --release

# Verify
./target/release/fathom --help
```

The release profile uses LTO and symbol stripping for a small, fast binary. Place it in a standard location:

```bash
sudo cp target/release/fathom /usr/local/bin/fathom
```

The binary requires no runtime dependencies beyond `ca-certificates` and the system C library. Run with `fathom serve --host 0.0.0.0 --port 8080` for production use.

### Docker / Docker Compose

Build the image:

```bash
docker build -t fathom .
```

The Dockerfile uses a multi-stage build:
- **Stage 1** (`rust:1.97-bookworm`) -- builds the release binary from source.
- **Stage 2** (`debian:bookworm-slim`) -- minimal runtime image with `ca-certificates` and a non-root `researcher` user (uid 1000).

Run the container:

```bash
docker run -d \
  --name fathom \
  -p 8080:8080 \
  -v ~/.fathom/config.toml:/home/researcher/.fathom/config.toml:ro \
  -v fathom-data:/data \
  -e FATHOM_API_KEYS=your-secret-key \
  -e RUST_LOG=info \
  fathom \
  serve --port 8080
```

**Docker Compose** example (`docker-compose.yml`):

```yaml
version: "3.8"
services:
  fathom:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - ./config.toml:/home/researcher/.fathom/config.toml:ro
      - fathom-data:/data
    environment:
      - FATHOM_API_KEYS=${FATHOM_API_KEYS}
      - RUST_LOG=info
    restart: on-failure

volumes:
  fathom-data:
```

The container exposes port `8080`, mounts `/data` for research output and databases, and reads the config from `/home/researcher/.fathom/config.toml`.

### Systemd Service

See [Systemd Service](#systemd-service) below for a complete unit file and installation steps.

---

## Server Configuration

### Required Configuration

The LLM API key is the only mandatory field. Set it in `~/.fathom/config.toml`:

```toml
[llm]
provider = "deepseek"
base_url = "https://api.deepseek.com"
api_key = "sk-your-key"          # REQUIRED
model = "deepseek-chat"
```

If using search backends, configure their API keys:

```toml
[search]
backend = "hybrid"    # linkup|exa|tavily|serper|brave|parallel|duckduckgo|hybrid|smart

[search.linkup]
api_key = "..."
```

### Server-Specific Settings

The server binds to `127.0.0.1:8080` by default. For production, bind to a non-loopback address:

```bash
fathom serve --host 0.0.0.0 --port 8080
```

Binding to a non-loopback address **requires** the `FATHOM_API_KEYS` environment variable. Without it, startup is rejected.

Public endpoints (`/health`, `/metrics`, `/dashboard`) remain open regardless of API key configuration. All `/api/v1/*` endpoints require a valid key when `FATHOM_API_KEYS` is set.

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `FATHOM_API_KEYS` | Comma-separated API keys for HTTP auth | *(unset = open access on loopback)* |
| `FATHOM_RATE_LIMIT` | Per-client rate limit (requests/min) | `120` |
| `PR_CONFIG` | Path to config file | `~/.fathom/config.toml` |
| `PR_MEMORY_DB` | Path to memory SQLite database | `~/.fathom/memory.db` |
| `PR_OUTPUT_DIR` | Default task output directory | `./research-output` |
| `PR_JOBS_DB` | Jobs registry database path | `~/.fathom/jobs.db` |
| `RUST_LOG` | Log level (`error`, `warn`, `info`, `debug`, `trace`) | `info` |
| `FATHOM_GOVERNANCE_ENABLED` | Enable governance policy engine | `false` |
| `FATHOM_GOVERNANCE_POLICY` | Inline JSON governance policy | *(empty)* |
| `FATHOM_CREDENTIAL_KEY` | Encrypted credential vault key (64 hex chars) | *(empty)* |

See [CONFIGURATION.md](CONFIGURATION.md) for the full configuration reference and [INSTALLATION.md](INSTALLATION.md) for the complete environment variable table.

---

## Reverse Proxy

Place Fathom behind a reverse proxy (nginx or Caddy) for TLS termination, rate limiting, and request buffering.

### nginx

```nginx
upstream fathom {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 443 ssl http2;
    server_name fathom.example.com;

    ssl_certificate     /etc/letsencrypt/live/fathom.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/fathom.example.com/privkey.pem;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=fathom:10m rate=30r/s;

    location / {
        limit_req zone=fathom burst=20 nodelay;

        proxy_pass http://fathom;
        proxy_http_version 1.1;

        # Standard proxy headers
        proxy_set_header Host              $host;
        proxy_set_header X-Real-IP         $remote_addr;
        proxy_set_header X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket / SSE support
        proxy_set_header Upgrade    $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 3600s;
        proxy_buffering off;
        proxy_cache off;
    }

    # Health check (no auth required)
    location = /health {
        proxy_pass http://fathom;
    }
}
```

### Caddy

```caddy
fathom.example.com {
    reverse_proxy 127.0.0.1:8080 {
        header_up X-Forwarded-For {remote_host}
        header_up X-Forwarded-Proto {scheme}
    }

    # Automatic TLS via Let's Encrypt
}

fathom.example.com:8080 {
    reverse_proxy 127.0.0.1:8080
}
```

### Key Points

- **TLS termination** -- terminate TLS at the proxy. Fathom itself does not support TLS.
- **SSE support** -- disable proxy buffering (`proxy_buffering off` in nginx) for Server-Sent Events streams at `/api/v1/events` and `/api/v1/sessions/:id/events`.
- **Rate limiting** -- apply rate limits at the proxy level to protect against abuse. The server's built-in limiter (`FATHOM_RATE_LIMIT`, default 120 req/min) is a second layer.
- **Long timeouts** -- agent sessions can run for minutes. Set `proxy_read_timeout` to at least `3600s` (1 hour) for nginx.

---

## Systemd Service

Create `/etc/systemd/system/fathom.service`:

```ini
[Unit]
Description=Fathom AI Worker Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=fathom
Group=fathom
WorkingDirectory=/home/fathom
ExecStart=/usr/local/bin/fathom serve --port 8080
Restart=on-failure
RestartSec=5

# Environment
Environment=RUST_LOG=info
EnvironmentFile=-/etc/fathom/env

# Hardening
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true
NoNewPrivileges=true
CapabilityBoundingSet=
ReadWritePaths=/home/fathom/.fathom /data

# Resource limits
MemoryMax=4G
CPUQuota=200%

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=fathom

[Install]
WantedBy=multi-user.target
```

Create the environment file `/etc/fathom/env`:

```bash
FATHOM_API_KEYS=your-production-api-key
FATHOM_RATE_LIMIT=120
```

Install and start:

```bash
sudo useradd --create-home --shell /usr/sbin/nologin fathom
sudo cp /usr/local/bin/fathom /usr/local/bin/
sudo cp fathom.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable fathom
sudo systemctl start fathom
```

### Restart Policy

`Restart=on-failure` ensures the process recovers from crashes with a 5-second backoff. For always-on deployments, use `Restart=always` instead.

### Resource Limits

- `MemoryMax=4G` -- prevents runaway memory usage from large context windows. Adjust based on workload.
- `CPUQuota=200%` -- allows up to 2 full CPU cores. The agent runtime is async and benefits from parallelism.

### Logging with journald

```bash
# Follow live logs
journalctl -u fathom -f

# Last 100 lines
journalctl -u fathom -n 100

# Since last boot
journalctl -u fathom -b

# Errors only
journalctl -u fathom -p err
```

---

## Monitoring

### Health Endpoint

```bash
curl http://localhost:8080/health
```

Returns HTTP 200 when the server is running. Use this for load balancer health checks and uptime monitoring.

### Prometheus Metrics

```bash
curl http://localhost:8080/metrics
```

The `/metrics` endpoint returns Prometheus text exposition format. Key metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `pr_sessions_total` | counter | Total number of research sessions created |
| `pr_sessions_active` | gauge | Number of sessions currently running |
| `pr_agents_spawned_total` | counter | Total agents spawned across all sessions |
| `pr_tokens_used_total` | counter | Total LLM tokens consumed |
| `pr_tool_calls_total` | counter | Total tool invocations |
| `pr_requests_total` | counter | Total HTTP requests to the API |
| `pr_request_duration_seconds` | histogram | HTTP request duration (buckets: 5ms to 10s) |

Example Prometheus scrape config:

```yaml
scrape_configs:
  - job_name: "fathom"
    static_configs:
      - targets: ["localhost:8080"]
    metrics_path: "/metrics"
```

### Log Levels and Configuration

Set the log level via `RUST_LOG`:

```bash
RUST_LOG=info    # Default: info and above
RUST_LOG=debug   # Verbose: debug and above (includes tool calls, LLM requests)
RUST_LOG=trace   # Maximum: everything (very noisy)
RUST_LOG=fathom_server=debug  # Module-specific filtering
```

All logging goes to stderr (captured by journald under systemd).

### Alert Rules

Example Prometheus alerting rules for `rules.yml`:

```yaml
groups:
  - name: fathom
    rules:
      # Alert if no sessions have run in 24h
      - alert: FathomNoActivity
        expr: pr_sessions_total == 0
        for: 24h
        labels:
          severity: warning
        annotations:
          summary: "Fathom has no session activity for 24 hours"

      # Alert on high error rate (if you add error counters)
      - alert: FathomHighErrorRate
        expr: rate(pr_requests_total{status="500"}[5m]) > 0.1
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Fathom error rate exceeds 10%"

      # Alert on high memory usage
      - alert: FathomHighMemory
        expr: process_resident_memory_bytes > 3.5e9
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Fathom memory usage exceeds 3.5 GB"
```

---

## Backup & Recovery

### Data Locations

All persistent data lives under `~/.fathom/` (or a custom path via environment variables):

| Path | Content | Env Override |
|------|---------|-------------|
| `~/.fathom/config.toml` | Server configuration | `PR_CONFIG` |
| `~/.fathom/memory.db` | Long-term semantic memory database | `PR_MEMORY_DB` |
| `~/.fathom/jobs.db` | Durable jobs registry | `PR_JOBS_DB` |
| `~/.fathom/jobs/` | Per-job workspace directories | `PR_JOBS_DIR` |
| `~/.fathom/profiles/` | Persona/profile TOML files | -- |
| `~/.fathom/sessions/` | Session history and replays | -- |
| `~/.fathom/parked/` | Parked (idle) agent state | -- |
| `<output_dir>/` | Research output per session | `PR_OUTPUT_DIR` or `[output] dir` |

### Memory DB Backup

```bash
# Hot backup (safe while server is running)
sqlite3 ~/.fathom/memory.db ".backup '/backup/fathom-memory-$(date +%Y%m%d).db'"

# Or stop the server first, then copy
sudo systemctl stop fathom
cp ~/.fathom/memory.db /backup/fathom-memory-$(date +%Y%m%d).db
sudo systemctl start fathom
```

### Session History Backup

```bash
# Copy the entire sessions directory
tar czf /backup/fathom-sessions-$(date +%Y%m%d).tar.gz ~/.fathom/sessions/
```

### Jobs Database Backup

```bash
sqlite3 ~/.fathom/jobs.db ".backup '/backup/fathom-jobs-$(date +%Y%m%d).db'"
```

### Parked Agents

Parked agent state in `~/.fathom/parked/` is ephemeral. Agents parked across server restarts are revived from the persistence layer (sessions and memory databases). A backup of this directory is optional but preserves any agent state not yet flushed to the database.

### Automated Backup Script

```bash
#!/bin/bash
BACKUP_DIR="/backup/fathom"
DATE=$(date +%Y%m%d)
mkdir -p "$BACKUP_DIR"

# Stop service briefly for consistent snapshot
sudo systemctl stop fathom

# Backup databases
cp ~/.fathom/memory.db "$BACKUP_DIR/memory-$DATE.db"
cp ~/.fathom/jobs.db "$BACKUP_DIR/jobs-$DATE.db"
tar czf "$BACKUP_DIR/sessions-$DATE.tar.gz" -C ~/.fathom sessions/
cp ~/.fathom/config.toml "$BACKUP_DIR/config-$DATE.toml"

# Restart
sudo systemctl start fathom

# Cleanup backups older than 30 days
find "$BACKUP_DIR" -type f -mtime +30 -delete
```

---

## Security Hardening

### Run as a Non-Root User

Never run Fathom as root. Create a dedicated user:

```bash
sudo useradd --create-home --shell /usr/sbin/nologin fathom
```

The systemd unit file in this guide already sets `User=fathom`. For Docker, the image runs as the `researcher` user (uid 1000) by default.

### Firewall Rules

Only expose the ports you need. For a typical deployment behind a reverse proxy:

```bash
# Allow SSH, HTTP, HTTPS
sudo ufw allow 22/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Block direct access to Fathom port (proxy handles it)
sudo ufw deny 8080/tcp

sudo ufw enable
```

If Fathom is exposed directly (no proxy), allow only port 8080 and restrict source IPs.

### API Key Rotation

1. Generate a new key.
2. Update `FATHOM_API_KEYS` in the environment file.
3. Restart the service: `sudo systemctl restart fathom`.
4. Update all clients with the new key.
5. Verify old key no longer works.

### Credentials Vault Key Management

The credential vault encrypts stored API keys and passwords with AES-256-GCM. The key is set via `FATHOM_CREDENTIAL_KEY` (64 hex characters or base64-encoded 32 bytes).

- Store the key securely (e.g., in a secrets manager, not in the repository).
- Changing the key requires re-encrypting all stored credentials.
- Never commit the key to version control.
- Use a different key per environment (staging vs production).

### Additional Hardening

- **Reverse proxy** -- always use TLS. See [Reverse Proxy](#reverse-proxy).
- **Rate limiting** -- set `FATHOM_RATE_LIMIT` to a reasonable value for your workload.
- **Governance** -- enable the policy engine for tool-level access control. See [GOVERNANCE.md](GOVERNANCE.md).
- **Log sanitization** -- SSE streams redact secret-like keys automatically (see `serialize_sse_event` in the server source).
- **SSRF protection** -- browser/computer-use tools reject requests to localhost, private ranges, and cloud metadata endpoints by default. Never set `COMPUTER_ALLOW_PRIVATE_HOSTS=true` in production.

See the server source for the full CORS, auth, and rate-limiting implementation.

---

## Scaling

### Horizontal Scaling

Fathom supports running multiple server instances behind a load balancer. Each instance handles its own set of sessions independently. The limitations:

- **SQLite is single-writer** -- concurrent writes from multiple processes will encounter `SQLITE_BUSY`. For multi-instance deployments, use the PostgreSQL backend for the contacts database (`[contacts] pg_url`) and consider PostgreSQL for the persistence layer.
- **Memory DB** -- the semantic memory database (`~/.fathom/memory.db`) is SQLite-based. Each instance has its own copy unless shared via a network filesystem (not recommended due to locking). For shared memory across instances, configure `[memory] db_path` to point to a common PostgreSQL-compatible store.
- **SSE streams** -- each instance only streams events from its own sessions. Clients connecting to different instances will see different event streams.

### Database Tuning

**SQLite** (default) is suitable for single-instance deployments. Key tuning:

- The database uses WAL (Write-Ahead Logging) mode by default for concurrent reads.
- For heavy workloads, increase SQLite's busy timeout via the connection string.
- Regular backups prevent WAL file growth.

**PostgreSQL** (optional) is recommended for:

- Multi-instance deployments.
- High write throughput (many concurrent sessions).
- The contacts database when shared across processes.

Set `[contacts] pg_url` to enable PostgreSQL for contacts. The memory and sessions databases are currently SQLite-only.

### LLM Rate Limit Management

Fathom includes built-in rate limit protection:

- **`ModelSemaphore`** -- bounds concurrent requests per model to 3 (hardcoded).
- **`FallbackCooldown`** -- after a 429 response, the model lane cools down for 60 seconds.
- **Retry logic** -- up to 3 retries with exponential backoff (500ms, 1s, 2s, +/-25% jitter).

For higher throughput:

- Upgrade your LLM provider plan for higher rate limits.
- Use `fast_model` to route auxiliary calls (extraction, classification) to a cheaper, higher-limit model.
- Use `role_models` to distribute load across different provider endpoints.
- Set `session_token_limit` to cap per-session token usage.

### Multi-Instance with Shared DB

For a production multi-instance setup:

1. Use PostgreSQL for contacts (`[contacts] pg_url`).
2. Use a shared file system (NFS/EFS) for research output (`[output] dir`).
3. Each instance reads the same config file (mount `~/.fathom/config.toml` as a read-only volume).
4. Each instance maintains its own memory.db and sessions -- cross-instance session lookup is not supported.
5. Use a load balancer that supports sticky sessions if clients need to reconnect to the same instance for SSE streams.

---

## Related

- [INSTALLATION.md](INSTALLATION.md) -- build instructions and requirements
- [CONFIGURATION.md](CONFIGURATION.md) -- full configuration reference
- [HTTP-API.md](HTTP-API.md) -- API route contract
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) -- common issues and fixes
- [GOVERNANCE.md](GOVERNANCE.md) -- policy engine and access control
