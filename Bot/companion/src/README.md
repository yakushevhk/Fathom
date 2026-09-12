# Companion Sidecar

The Companion sidecar is a lightweight, zero-dependency Node.js bridge daemon that allows mobile devices (iOS companion client) to securely interact with a local Parallel harness over LAN, Tailscale MagicDNS, or hosted HTTPS relays without exposing raw, unauthenticated local APIs to the network.

---

## Architecture & Socket Split

The sidecar operates an explicit multi-socket trust architecture:

```text
┌────────────────────────────────────────────────────────┐
│                     MOBILE DEVICE                      │
│                  (iOS Companion App)                   │
└───────────────┬────────────────────────▲───────────────┘
                │                        │
       Bearer Token / HPKE        Scrubbed Responses & SSE
                │                        │
                ▼                        │
   ┌──────────────────────────────────────────────┐
   │    :8810 (0.0.0.0) Companion Device Port     │
   │    - Bearer authentication against SHA-256   │
   │    - Strict route allowlist (default DENY)   │
   │    - SSE & JSON payload data scrubbing       │
   │    - WebSocket upgrade for noVNC desktop     │
   └──────────────────────┬───────────────────────┘
                          │ (Proxied on 127.0.0.1)
                          ▼
   ┌──────────────────────────────────────────────┐
   │        :8799 (127.0.0.1) Core Harness        │
   │        - Unauthenticated loopback API        │
   └──────────────────────────────────────────────┘
                          ▲
                          │ Local Pairing & Control
   ┌──────────────────────┴───────────────────────┐
   │    :8811 (127.0.0.1) Control Plane           │
   │    - QR code generation with pairing token   │
   │    - 6-digit PIN redemption window           │
   │    - Paired device revocation                │
   │    - Tailscale & Bonjour status              │
   └──────────────────────────────────────────────┘
```

1. **Companion Device Port (`0.0.0.0:8810`)**:
   - Exposed to local network / Tailscale.
   - Enforces bearer token authentication for all requests except pairing redemption (`POST /api/companion/redeem`).
   - Replays authorized requests to `127.0.0.1:8799` with local host/origin headers to satisfy DNS-rebinding protections.
   - Forwards WebSocket upgrade events for remote desktop access (`noVNC`).
2. **Control Port (`127.0.0.1:8811`)**:
   - Strictly bound to loopback. Never accessible off-machine.
   - Used by the desktop UI / Electron to generate pairing windows, review connected devices, toggle cloud desktop access permissions, and revoke tokens.
3. **Core Harness Port (`127.0.0.1:8799`)**:
   - The unmodified Parallel engine. Relies on loopback isolation as its sole authentication layer.
4. **Private IPC / UDS Socket**:
   - When run inside Electron, receives an unforgeable `parallel:companion-mutation-token` over `process.parentPort` to authorize privileged harness state mutations.

---

## Pairing Protocol & Cryptographic Invariants

### 1. Pairing Window Lifecycle (`devices.ts`)
- **Dual Credentials**:
  - `token`: High-entropy, URL-safe random string embedded in the desktop QR code.
  - `code`: 6-digit numeric PIN (`randomInt(100_000, 1_000_000)`) for manual typing.
- **Constraints**:
  - `PAIRING_TTL_MS = 120_000` (2-minute window).
  - `MAX_PAIRING_ATTEMPTS = 5` attempts. Constant-time string comparison (`timingSafeEqual`) prevents timing oracle attacks on digits.
  - Redeeming either the code or the token instantly burns both credentials.
  - Device fleet is capped at `MAX_DEVICES = 20`.

### 2. Token Storage & Verification
- Mobile clients store an opaque bearer token.
- Disk storage (`~/.parallel/devices.json`) stores **only** the SHA-256 hash of the bearer token (`tokenHash`).
- If `devices.json` is compromised or stolen, stored hashes cannot be reversed into usable bearer credentials.
- `timingSafeEqual` is used for all digest comparisons.

### 3. End-to-End Encrypted Secret Provisioning (`phone-secret-key.ts`)
- When mobile clients supply sensitive API credentials via bot secret cards (`POST /api/bots/:id/secret-cards/:id/provide`), the secrets are encrypted using HPKE (Hybrid Public Key Encryption).
- Key validation verifies canonical, on-curve, uncompressed NIST P-256 (`prime256v1`) public points before envelopes reach the OS keychain.

---

## Route Allowlisting & Request Scrubbing

### Strict Allowlist (`routes.ts`)
The sidecar adopts a **default-deny** policy against the upstream harness API. An unknown endpoint added to the core server is inaccessible to paired devices until explicitly declared:
- Allowed: Reading configurations (`/api/config`), bot rosters (`/api/bots`), thread messaging (`/api/bots/:id/messages`), active branch selection, and task lifecycle operations.
- Capability Gated:
  - `POST /api/bots/:id/computer/join`: Interactive remote desktop session. Gated by explicit `cloudDesktopAccess: true` boolean on the paired device record.
- Message File Downloads (`POST /api/threads/:id/messages/:id/file`): Passes through verbatim binary streams, bypassing JSON decoders.

### Data Scrubbing (`wire.ts`)
Outbound responses and Server-Sent Event (SSE) streams are sanitized on the fly before reaching mobile clients:
- Strips internal engine state:
  - `resumeCursors`: Native execution cursors and session IDs.
  - `sshAlias`: Hostname/IP labels of private cloud VPS instances.
- Handles both CRLF and LF SSE boundaries with an upper boundary limit (`MAX_SSE_EVENT_BYTES = 1MB`) to prevent memory exhaustion from unterminated frames.
- Automatically handles RFC 9457 structured JSON (`application/problem+json`).

---

## Discovery & Endpoint Resolution (`mdns.ts`, `endpoints.ts`, `advertise-watch.ts`)

1. **mDNS / Bonjour Broadcasts**:
   - Advertises service `_openmausbot._tcp` on local network interfaces.
   - Monitors network changes (`createAddressWatcher`) to re-advertise when DHCP assigns new IPs or network topology changes.
2. **Endpoint Prioritization**:
   The pairing handshake returns up to 8 prioritized connection candidates:
   1. `hosted` (Priority 0): Configured reverse proxy (`OMB_COMPANION_HOSTED_URL`).
   2. `tailnet` (Priority 100): MagicDNS hostname (e.g., `my-mac.tailnet-xyz.ts.net`).
   3. `lan` (Priority 200+): Direct local network IPv4/IPv6 addresses.
   4. `bonjour` (Priority 300): Local `.local` hostname fallback.

---

## Key Files & Modules

| File | Purpose |
|---|---|
| `index.ts` | Process entry point, socket initialization, lifecycle teardown, signal trapping |
| `devices.ts` | Device registry, pairing code minting/redemption, token hashing, JSON persistence |
| `proxy.ts` | Forwarding HTTP proxy, headers timeout, payload limits, WebSocket upgrade relay |
| `routes.ts` | Regex-anchored route allowlist and capability checkers (`isCloudDesktopAccess`) |
| `wire.ts` | Recursive object key scrubber and streaming SSE frame rewriter |
| `endpoints.ts` | Base URL resolution, candidate ranking, and validation of HTTPS hosted origins |
| `control.ts` | Loopback HTTP endpoints for desktop pairing UI and device revocation |
| `mdns.ts` | Minimal multicast DNS packet serializer and UDP responder |
| `viewer-relay.ts` | WebSocket bridge for cloud desktop noVNC frame forwarding |
| `phone-secret-key.ts` | P-256 public key validation for HPKE credential provisioning |
| `advertise-watch.ts` | Periodic poll checking network interface state transitions |

---

## Verification & Manual Testing

Run the sidecar directly from source:
```sh
node --loader ts-node/esm companion/src/index.ts
```

Test pairing window generation:
```sh
curl -s http://127.0.0.1:8811/pair | jq .
```

Verify route refusal on an unauthorized endpoint:
```sh
curl -i -H "Authorization: Bearer invalid_token" http://127.0.0.1:8810/api/config
# Returns HTTP 401 Unauthorized
```
