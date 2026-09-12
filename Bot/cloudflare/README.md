# Cloudflare Edge Infrastructure (`cloudflare/`)

This directory contains serverless edge workers running on Cloudflare's global network, handling account identity, installation leases, secure remote tunnel brokering, and third-party credential proxies.

---

## Architectural Role & Worker Topology

Edge services decouple the local desktop application from sensitive global credentials and coordinate remote companion access:

```
                                  +---------------------------------------+
                                  |            Cloudflare Edge            |
                                  +---------------------------------------+
                                      /                               \
                                     /                                 \
                                    v                                   v
             +-------------------------------+     +-------------------------------+
             |      Control Plane Worker     |     |    Composio Broker Worker     |
             | (`cloudflare/control-plane/`) |     | (`cloudflare/composio-broker/`|
             +-------------------------------+     +-------------------------------+
                |                         |           |                         |
            (Auth / OTP)            (Tunnel DNS)  (Per-install               (Composio
                |                         |        session token)             Project Key)
                v                         v           v                         v
           +----------+             +----------+ +----------+             +----------+
           | D1 Auth  |             |  Tunnel  | | D1 Token |             | Composio |
           | Database |             | Connector| | Database |             | Service  |
           +----------+             +----------+ +----------+             +----------+
```

---

## Edge Services

### 1. Control Plane Worker (`cloudflare/control-plane/`)
Handles user identity, installation lifecycle, and automated Cloudflare Tunnel orchestration for mobile/companion access:
- **Authentication**: Powered by Better Auth with email one-time passwords (OTP), signed bearer sessions, and Cloudflare Email Sending bindings. No third-party tracking or marketing telemetry.
- **Zero Local Data Exfiltration**: Never receives or stores chats, prompts, bot models, files, or local SQLite state.
- **Installation Registry**: Issues unique installation credentials (`omb_install_…`) with a 90-day expiration window, serialized rotation cooldowns, and per-account limits (100 active installations max).
- **Remotely Managed Tunnels**:
  - Dynamically provisions one Cloudflare Tunnel per desktop installation.
  - Generates opaque public CNAME records (`c-<32-hex>.<COMPANION_HOST_SUFFIX>`) pointing to `<tunnel-id>.cfargotunnel.com`.
  - Configures tunnel traffic strictly to the Electron-owned gateway at `http://127.0.0.1:8812` with a mandatory `http_status:404` catch-all rule.
- **D1 Schema & Migration Pipeline (`migrations/`)**:
  - `0001_better_auth_1_7_1.sql`: Core auth models.
  - `0002_installations.sql`: Owner-scoped installation metadata and credential hashes.
  - `0003_otp_recipient_rate_limits.sql`: Recipient rate limits using HMAC-keyed hashes.
  - `0004_managed_companion_endpoints.sql`: Monotonic generation leases, endpoint state tracking, and sanitized error codes.
  - `0005_endpoint_cleanup_backoff.sql`: Backoff tracking for orphaned resource sweeps.

### 2. Composio Broker Worker (`cloudflare/composio-broker/`)
Maintains isolation between the shared Composio project master key and desktop clients:
- **Credential Protection**: The desktop app never receives the root Composio API key.
- **Per-Installation Isolation**: Each desktop receives a random bearer token (stored only as a SHA-256 hash in D1).
- **Session & Link Brokering**:
  - Automatically manages distinct Composio user IDs and sessions per installation.
  - Proxies Model Context Protocol (MCP) server endpoints.
  - Returns short-lived connected account authorization URLs on demand.

---

## Security Invariants & Operational Rules

1. **Credential Segregation**: Account bearer tokens (`omb_session_…`) and installation credentials (`omb_install_…`) cannot be substituted for one another.
2. **Digest-Only Storage**: Secrets (OTPs, installation tokens, broker credentials) are stored solely as SHA-256 hashes or cryptographic HMACs.
3. **Leased Mutation & Idempotency**: Tunnel creation and endpoint modifications utilize a 60-second D1 lease with monotonic generation counters, preventing race conditions during concurrent requests.
4. **Graceful Resource Reclamation**: Scheduled worker sweeps automatically clean up abandoned tunnels and DNS records for revoked installations, utilizing an exponential backoff schedule (5m, 15m, 1h, 6h, 24h) to remain within Cloudflare subrequest bounds.
5. **Strict Origin & Header Control**: Edge endpoints enforce exact-origin CORS allowlists, reject wildcards, and apply `Cache-Control: no-store` on all authenticated responses.

---

## Deployment & Verification

### Control Plane
```sh
# Type-checking and tests
pnpm control-plane:check
pnpm control-plane:test

# Local D1 migration and dev execution
pnpm --filter @parallel/control-plane exec wrangler d1 migrations apply DB --local --config wrangler.jsonc
pnpm --filter @parallel/control-plane exec wrangler dev --config wrangler.jsonc
```

### Composio Broker
```sh
# Generate types and run tests
pnpm broker:types
pnpm test cloudflare/composio-broker/

# Deploy remote migrations
pnpm exec wrangler d1 migrations apply parallel-composio --remote --config cloudflare/composio-broker/wrangler.jsonc
```
