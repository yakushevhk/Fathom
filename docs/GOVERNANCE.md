# Governance

> Policy engine, audit trail, credentials vault — safety and compliance for autonomous AI workers.

Fathom's governance subsystem (`crates/governance`) provides a **policy engine**, an **immutable audit trail**, and an **AES-256-GCM credentials vault**, ensuring that every agent action is authorized, auditable, and that secrets are never exposed to the agent directly.

---

## Policy Engine

The policy engine enforces **allow/deny rules** on every tool call the agent makes. Rules are evaluated against the `<tool, target>` pair before execution proceeds.

### Rule format

```toml
# ~/.fathom/policy.toml

[[rules]]
action = "allow"
tool = "web_search"
target = "*"

[[rules]]
action = "deny"
tool = "browser_navigate"
target = "*.internal.corp"

[[rules]]
action = "deny"
tool = "shell"
target = "*"

[[rules]]
action = "allow"
tool = "shell"
target = "git status"
```

Each rule has three fields:

| Field | Type | Description |
|-------|------|-------------|
| `action` | `"allow"` \| `"deny"` | Whether the rule permits or blocks the call |
| `tool` | string | Tool name or glob pattern (e.g. `browser.*`, `shell`) |
| `target` | string | Target glob pattern (e.g. `*.internal.corp`, `git status`) |

### Evaluation

1. All matching rules are collected. A rule matches if the tool and target patterns both match the incoming call.
2. **Deny wins** — if any matching rule is `deny`, the call is blocked.
3. If no rule matches, the default is **fail-closed**: the call is denied.
4. The verdict (`allow` or `deny`) is persisted to the audit trail before the call proceeds.

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FATHOM_GOVERNANCE_ENABLED` | `false` | Master switch for the governance subsystem. Set to `true` to enforce policy decisions. |
| `FATHOM_GOVERNANCE_POLICY` | *(empty)* | Inline JSON policy document. An enabled empty or unmatched policy fails closed. |

---

## Audit Trail

Every authorization decision is recorded in an **immutable append-only audit trail** stored in SQLite. The trail is designed to be tamper-evident and inspection-friendly.

### Audit endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/governance/audit` | List audit records with optional filters (tool, agent, decision, session) |
| `POST` | `/api/v1/governance/decide` | Evaluate an action against the active policy |

### Record format

Each audit record contains:

```json
{
  "id": "019fd38a-...",
  "timestamp": "2026-08-20T10:30:00Z",
  "session_id": "019fd38a-...",
  "agent_id": "019fd38a-...",
  "tool": "web_search",
  "target": "example.com",
  "args_redacted": "{ \"query\": \"[REDACTED]\" }",
  "verdict": "deny",
  "rule_id": "rule-3",
  "reason": "shell is denied for all targets"
}
```

### Redaction

Secret-like values are **redacted before audit persistence**:

- API keys, tokens, passwords, PEM blocks, and base64-encoded secrets are detected by regex
- Redacted fields are replaced with `[REDACTED]` in the audit record
- The original plaintext is never written to the audit database

---

## Credentials Vault

The credentials vault stores sensitive values (API keys, tokens, passwords) encrypted at rest using **AES-256-GCM**. Agents never have direct access to the vault — they interact with it through a **server relay** that injects credentials into tool calls on the operator's behalf.

### Encryption

- **Algorithm**: AES-256-GCM (Galois/Counter Mode)
- **Key material**: `FATHOM_CREDENTIAL_KEY` is decoded from 64-character hex or base64 into the 32-byte AES-256-GCM key
- **Nonce**: A random 12-byte nonce per encryption operation
- **Storage**: Encrypted blobs are stored in SQLite alongside metadata (label, scope, created_at, updated_at)

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/credentials` | List credential metadata (labels, scopes, timestamps). **Never returns plaintext.** |
| `POST` | `/api/v1/credentials` | Store a new credential. Accepts `{ "label", "value", "scope" }`. |
| `GET` | `/api/v1/credentials/:id` | Retrieve a credential by ID. Requires operator authentication. |
| `DELETE` | `/api/v1/credentials/:id` | Delete a credential. |

### Operator-only access

- Credentials are **operator-only** — agents cannot read or write credentials directly
- There is **no secret-input tool** in the agent tool registry
- Operators enter secrets through the UI (Tauri desktop, Next.js web dashboard, or CLI)
- The relay injects credentials into tool calls by adding an `x-fathom-operator` claim to the request

### Environment variable

| Variable | Description |
|----------|-------------|
| `FATHOM_CREDENTIAL_KEY` | Base64-encoded 256-bit key for AES-256-GCM encryption. Must be exactly 32 bytes when decoded. |

---

## Server Relay

The server relay sits between the agent and external services, injecting credentials from the vault into tool calls without exposing them to the agent. The relay:

1. Intercepts tool calls that require authentication (e.g., API keys for search backends)
2. Looks up the corresponding credential from the vault
3. Injects it into the request headers or body
4. Adds an `x-fathom-operator` claim to the audit record
5. Never reveals the plaintext credential to the agent's context

---

## Configuration

Governance is configured through environment variables rather than an `[governance]` TOML section:

```bash
FATHOM_GOVERNANCE_ENABLED=true \
FATHOM_GOVERNANCE_POLICY='{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"}]}' \
fathom serve --port 8080
```

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `FATHOM_GOVERNANCE_ENABLED` | bool | `false` | Enable policy enforcement. |
| `FATHOM_GOVERNANCE_POLICY` | JSON string | *(empty)* | Inline policy document; enabled empty/unmatched policy fails closed. |

---

## Related

- [ARCHITECTURE.md](ARCHITECTURE.md) — crate diagram and governance crate description
- [CONFIGURATION.md](CONFIGURATION.md) — full env var reference
- [HTTP-API.md](HTTP-API.md) — governance API endpoints
- [crates/governance.md](crates/governance.md) — detailed governance crate documentation