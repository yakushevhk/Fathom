# Governance

> Policy engine, audit trail, credentials vault — safety and compliance for autonomous AI workers.

Fathom's governance subsystem provides a **policy engine**, an **immutable audit trail**, and an **AES-256-GCM credentials vault**, ensuring that every agent action is authorized, auditable, and that secrets are never exposed to the agent directly.

The policy engine lives in `crates/governance` (the `PolicyEngine`, `Governance`, and `AuditEvent` types). The credentials vault lives in `crates/persistence` (`CredentialsVault` / `Persistence`). HTTP endpoints are in `crates/server` (`governance_api`, `credentials_api`).

---

## Policy Engine

The policy engine enforces **allow/deny rules** on every tool call the agent makes. Rules are evaluated against an `ActionContext` (tool name, optional host/path/intent filters) before execution proceeds.

### Rule format

Rules are provided as JSON through the `FATHOM_GOVERNANCE_POLICY` environment variable or the `PUT /api/v1/governance/policy` endpoint.

```json
{
  "rules": [
    { "effect": "allow", "tool": "web_search" },
    { "effect": "deny", "tool": "browser_navigate", "host": "*.internal.corp" },
    { "effect": "deny", "tool": "shell" },
    { "effect": "allow", "tool": "shell", "path": "git status" }
  ]
}
```

Each rule has these fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `effect` | `"allow"` \| `"deny"` | yes | Whether the rule permits or blocks the call |
| `tool` | string (glob) | no | Tool name pattern — matches against the action's tool name |
| `host` | string (glob) | no | Host pattern — matches against the URL's host component |
| `path` | string (glob) | no | Path pattern — matches against the URL path or file path |
| `intent` | string (glob) | no | Intent pattern — matches against the action's intent string |

All fields are optional except `effect`. An omitted field matches anything (wildcard). Glob patterns use `*` to match any sequence of characters.

### Action context

When the engine evaluates a tool call, it receives an `ActionContext`:

```json
{
  "agent": "agent-id",
  "session": "session-id",
  "tool": "web_search",
  "args": { "query": "example" },
  "url": "https://example.com",
  "file": "/tmp/output.md",
  "intent": "search for AI news",
  "mcp_metadata": null
}
```

### Evaluation

1. All matching rules are collected. A rule matches if all its specified fields (`tool`, `host`, `path`, `intent`) match the incoming action.
2. **Deny wins** — if any matching rule has `effect: "deny"`, the call is blocked.
3. If no rule matches, the default is **fail-closed**: the call is denied.
4. The verdict (`allow` or `deny`) is persisted to the audit trail before the call proceeds.

### Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `FATHOM_GOVERNANCE_ENABLED` | `false` | Master switch for the governance subsystem. Set to `true` to enforce policy decisions. |
| `FATHOM_GOVERNANCE_POLICY` | *(empty)* | Inline JSON `PolicyConfig` object. An enabled empty or unmatched policy fails closed. |

---

## Audit Trail

Every authorization decision is recorded in an **immutable append-only audit trail** stored in SQLite. The trail is designed to be tamper-evident and inspection-friendly.

### API endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/governance/audit` | List audit records with optional filters (`?limit=`, `?decision=`, `?agent=`, `?session=`) |
| `POST` | `/api/v1/governance/decide` | Evaluate an action against the active policy |
| `GET` | `/api/v1/governance/policy` | Get the current policy configuration |
| `PUT` | `/api/v1/governance/policy` | Replace the active policy (max 1000 rules) |

### Audit event record

Each audit record contains the redacted action context and the decision:

```json
{
  "id": "019fd38a-...",
  "timestamp": "2026-08-20T10:30:00Z",
  "context": {
    "agent": "agent-id",
    "session": "session-id",
    "tool": "web_search",
    "args": { "query": "[REDACTED]" },
    "url": "https://example.com",
    "file": null,
    "element": null,
    "intent": null,
    "mcp_metadata": null
  },
  "decision": "deny"
}
```

### Redaction

Secret-like values are **redacted before audit persistence**:

- API keys, tokens, passwords, PEM blocks, and base64-encoded secrets are detected by regex
- Redacted fields are replaced with `[REDACTED]` in the audit record
- The original plaintext is never written to the audit database

---

## Credentials Vault

The credentials vault stores sensitive values (API keys, tokens, passwords) encrypted at rest using **AES-256-GCM**. Agents never have direct access to the vault — the credential endpoints are operator-only, authenticated by API key.

### Encryption

- **Algorithm**: AES-256-GCM (Galois/Counter Mode) via the `ring` crate
- **Key material**: `FATHOM_CREDENTIAL_KEY` is decoded from 64-character hex or base64 into the 32-byte AES-256-GCM key
- **Nonce**: A random 12-byte nonce per encryption operation (generated via `ring::rand`)
- **Storage**: Encrypted blobs are stored in SQLite alongside metadata (name, kind, created_at, updated_at). The plaintext secret is never persisted.

### API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/v1/credentials` | List credential metadata (names, kinds, timestamps). **Never returns plaintext.** |
| `POST` | `/api/v1/credentials` | Store a new credential. Accepts `{ "name", "kind", "secret" }`. |
| `DELETE` | `/api/v1/credentials/:id` | Delete a stored credential by ID. Returns 204 on success. |

**Store request body:**

```json
{
  "name": "serper-api-key",
  "kind": "api_key",
  "secret": "sk-abc123..."
}
```

**List response:**

```json
[
  {
    "id": "019fd38a-...",
    "name": "serper-api-key",
    "kind": "api_key",
    "created_at": "2026-08-20T10:00:00Z",
    "updated_at": "2026-08-20T10:00:00Z"
  }
]
```

### Agent boundary

- Agents cannot read or write credentials through the tool registry
- HTTP credential endpoints require an authenticated API key; deployments that need operator-only access must enforce that distinction at their gateway or principal layer
- There is **no secret-input tool** in the agent tool registry
- Operators enter secrets through the UI (Tauri desktop or Next.js web dashboard)

### Environment variable

| Variable | Description |
|----------|-------------|
| `FATHOM_CREDENTIAL_KEY` | Hex-encoded (64 chars) or base64-encoded 256-bit key for AES-256-GCM encryption. Must decode to exactly 32 bytes. |

---

## Configuration

Governance is configured through environment variables:

```bash
FATHOM_GOVERNANCE_ENABLED=true \
FATHOM_GOVERNANCE_POLICY='{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"},{"effect":"deny","tool":"shell"}]}' \
fathom serve --port 8080
```

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `FATHOM_GOVERNANCE_ENABLED` | bool | `false` | Enable policy enforcement. |
| `FATHOM_GOVERNANCE_POLICY` | JSON string | *(empty)* | Inline `PolicyConfig` JSON; enabled empty/unmatched policy fails closed. |
| `FATHOM_CREDENTIAL_KEY` | string | *(empty)* | AES-256-GCM key for the credentials vault (hex or base64, must decode to 32 bytes). |

---

## Related

- [ARCHITECTURE.md](ARCHITECTURE.md) — crate diagram and governance crate description
- [CONFIGURATION.md](CONFIGURATION.md) — full env var reference
- [HTTP-API.md](HTTP-API.md) — governance API endpoints
- [crates/governance.md](crates/governance.md) — detailed governance crate documentation
