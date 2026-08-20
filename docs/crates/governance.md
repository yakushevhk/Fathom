# Crate Documentation `crates/governance`

The `governance` crate (`pr-governance`) provides policy enforcement and audit primitives for **governed computer actions**. It is a small, dependency-light crate: it evaluates explicit **allow/deny rules** against an action context, fails closed, and produces redacted, serializable **audit decision records** that an application boundary (e.g. the server or agent crates) can persist to any sink.

> For the end-to-end governed-computer architecture (isolated browser service, relay, durable product model, deployment), see [OPENBOT_ARCHITECTURE.md](../OPENBOT_ARCHITECTURE.md). This page documents the crate itself.

## Overview

The crate deliberately has **no persistence dependency**. Audit events are plain serializable values; wiring them into the SQLite `audit_events` table (via `pr-persistence`) is the caller's responsibility. It offers:

| Capability | Description |
|---|---|
| **Policy engine** | `PolicyRule` list → `Decision::Allow` / `Decision::Deny`. Deny always wins; no matching allow fails closed. |
| **Audit records** | `AuditEvent` — a timestamped, UUID-v7-keyed decision with a **redacted** action context. |
| **Secret redaction** | Recursive redaction of credential/secret keys in args, URL query values, credentials in URLs, and MCP metadata. |
| **Target resolver** | `TargetResolver` — resolves only element references that were explicitly present in a UI snapshot. |
| **Facade** | `Governance` combines the policy engine with an optional `AuditSink` (`authorize_and_record`). |

## File structure

| File | Purpose |
|------|---------|
| `src/lib.rs` | All types and logic: `ActionContext`, `PolicyEngine`, `AuditEvent`, redaction, `TargetResolver`, `Governance` facade (single file, ~300 lines) |

Dependencies: `serde`, `serde_json`, `chrono`, `uuid`, `thiserror`, `url`. No tokio, no database access.

---

## 1. `ActionContext` — the action being evaluated

```rust
pub struct ActionContext {
    pub agent: String,
    pub session: String,
    pub tool: String,
    pub args: Value,             // serde_json::Value, defaults to null
    pub url: Option<String>,
    pub element: Option<String>,
    pub file: Option<String>,
    pub intent: Option<String>,
    pub mcp_metadata: Option<Value>,
}
```

- `ActionContext::new(agent, session, tool, args)` — constructor with the three required fields; all optional fields default to `None`.
- Rules match against `tool`, `url` (host/path), `file`, and `intent`; `agent`, `session`, `args`, `element`, and `mcp_metadata` are carried for context and audit but are **not** rule-matching dimensions (except through redaction).

## 2. Policy engine — allow/deny rules, fail-closed

### Types

```rust
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect { Allow, Deny }

pub struct PolicyRule {
    pub effect: PolicyEffect,
    pub tool: Option<String>,   // glob, e.g. "browser.*"
    pub host: Option<String>,   // glob, matched against URL host
    pub path: Option<String>,   // glob, matched against file path / URL
    pub intent: Option<String>, // glob, matched against intent
}

pub struct PolicyConfig { pub rules: Vec<PolicyRule> }

#[serde(rename_all = "lowercase")]
pub enum Decision { Allow, Deny }
```

- Convenience constructors: `PolicyRule::allow()` and `PolicyRule::deny()` (all match fields `None` — matches every action).
- `Decision::is_allowed()` / `Decision::allowed()` — test helper for the decision result.
- The crate never evaluates code: **rules use simple, safe string matching only**; there is no regex or expression evaluation in policy files.

### Evaluation algorithm — `PolicyEngine::decide(action)`

```
1. allowed = false
2. for each rule in config.rules:
     if !rule_matches(rule, action): continue
     match rule.effect:
       Deny  → return Decision::Deny     # deny always takes precedence
       Allow → allowed = true
3. return allowed ? Allow : Deny          # fail closed
```

- **Fail-closed**: an empty policy (`PolicyConfig::default()` → no rules) denies everything. An action that matches no rule is denied.
- **Deny wins**: if any deny rule matches, the result is `Deny` even when an allow rule also matches (precedence is by effect, not by list order).

### Rule matching — `rule_matches(rule, action)`

A rule matches when **all** of its set match fields match (i.e. AND of: tool ∧ host ∧ path ∧ intent). A `None` field is a wildcard that matches anything.

- **tool**: `glob_match(rule.tool, action.tool)` — glob is applied against the tool name (e.g. `browser.*`).
- **host**: only matches when `action.url` is a present, parseable URL; `glob_match(rule.host, parsed.host_str())`.
- **path**: three candidate sources, any of which may match:
  1. `action.file` glob-match against the rule's path;
  2. the raw `action.url` string;
  3. the parsed URL path (`url::Url::parse(url).path()`).
- **intent**: glob-match against `action.intent`; only matches if intent is present.

### `glob_match` — wildcard matcher

A small iterative two-pointer matcher: `*` matches any sequence of characters (including empty). Used everywhere instead of regex so policy files stay declarative and safe.

### Loading policies

- `PolicyEngine::new(PolicyConfig)` — construct from a typed config.
- `PolicyEngine::from_json(&str) -> Result<Self, PolicyError>` — parse policy JSON; `PolicyError::InvalidJson` wraps `serde_json::Error`.
- `PolicyEngine::config()` — accessor.
- Example policy JSON (used by the server, see `FATHOM_GOVERNANCE_POLICY` below):

```json
{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"},{"effect":"deny","tool":"browser.type","path":"/admin/*"}]}
```

## 3. Audit decision records — `AuditEvent`

```rust
pub struct AuditEvent {
    pub id: String,               // Uuid::now_v7()
    pub timestamp: DateTime<Utc>,
    pub context: ActionContext,   // ALWAYS redacted before storage
    pub decision: AuditDecision,  // Allow | Deny (from Decision)
}

pub enum AuditDecision { Allow, Deny }   // From<Decision> implemented
```

- `AuditEvent::new(context, decision)` — constructs the record and applies `redact_action_context` to the context **before** it is persisted. The stored context therefore never contains secrets.
- `AuditDecision::from(Decision)` — lossless conversion so audit loggers never need to match on the raw `Decision`.

## 4. Secret redaction

Redaction is applied at audit time so that decision history stays safe even when raw tool arguments contained credentials.

### `redact_secrets(value: &Value) -> Value`

Recursively walks a JSON value and replaces the values of **secret keys** with the string `"[REDACTED]"`:

- Keys are normalized first: `-`/space/`_` → `_`, and camelCase boundaries are split (`apiKey` → `api_key`).
- A key is secret if it equals, is prefixed (`token_`), suffixed (`_token`), or contains (`_token_`) one of: `password`, `passwd`, `secret`, `token`, `api_key`, `apikey`, `authorization`, `cookie`, `credential`, `private_key`, `access_key`, `client_secret`, `access_token`.
- Arrays recurse element-wise; non-object/array values pass through unchanged.

### `redact_action_context(action) -> ActionContext`

Returns a deep copy with: `args` = `redact_secrets(args)`; for `computer_type` / `browser_type` / `computer_secret` actions the `text`/`value`/`secret` args are unconditionally replaced; `mcp_metadata` = `redact_secrets`; `url` = `redact_url(url)`.

### `redact_url(raw) -> String`

- Unparseable URL → `"[REDACTED_URL]"`.
- Otherwise: strips username and password, drops the fragment, and rewrites the query string replacing values of secret query keys with `[REDACTED]` (non-secret query keys keep their values — the URL shape stays auditable).

## 5. `TargetResolver` — resolving only known UI refs

```rust
pub struct TargetResolver { targets: HashMap<String, Value> }
```

Supports the accessibility-tree snapshot model of the computer service: the UI surface exposes **opaque element refs**, and actions may only target refs explicitly present in a snapshot.

- `register(reference, target)` — record a snapshot element under its ref.
- `resolve(reference) -> Option<&Value>` — fetch a registered target; `None` for unknown refs.
- `contains(reference) -> bool`, `clear()` — membership test / snapshot rotation.

Unknown refs resolve to `None`, so a stale or fabricated ref never reaches the browser.

## 6. Audit sink and the `Governance` facade

```rust
pub trait AuditSink: Send + Sync {
    fn record(&self, event: &AuditEvent) -> Result<(), String>;
}

pub struct Governance { policy: PolicyEngine, sink: Option<Arc<dyn AuditSink>> }

impl Governance {
    pub fn new(policy: PolicyEngine) -> Self;            // no sink
    pub fn with_audit_sink(sink: Arc<dyn AuditSink>) -> Self;
    pub fn policy(&self) -> &PolicyEngine;
    pub fn authorize(&self, context) -> Decision;        // decide only
    pub fn record(&self, event) -> Result<(), GovernanceError>;  // record only
    pub fn authorize_and_record(&self, context) -> Result<Decision, GovernanceError>;
}
```

- `GovernanceError::Sink(String)` — surfaces `AuditSink::record` failures.
- The sink is optional: with no sink, `record` is a no-op and `authorize_and_record` behaves like `authorize`. Persistence crates implement `AuditSink` at the application boundary to store events in SQLite.

## 7. Integration with server and agent

| Component | How it uses `pr-governance` |
|---|---|
| `pr-agent` | Authorizes **every tool call before execution** and persists redacted authorization events (the resulting `AuditEvent`s are passed to the sink). |
| `pr-server` | Loads the policy from env (`FATHOM_GOVERNANCE_ENABLED` + `FATHOM_GOVERNANCE_POLICY`), exposes authenticated policy and audit endpoints, and stores `audit_events` in the existing SQLite database via `pr-persistence`. |
| `pr-persistence` | Implements the `AuditSink` and stores `audit_events`, durable `coworkers`, and `channels` tables. |

## 8. Environment variables

| Variable | Purpose |
|---|---|
| `FATHOM_GOVERNANCE_ENABLED` | Set to `true` to enforce the policy engine. When unset/false the policy engine is bypassed. |
| `FATHOM_GOVERNANCE_POLICY` | Optional JSON policy document, e.g. `{"rules":[{"effect":"allow","tool":"browser.*","host":"example.com"}]}`. An empty or unmatched policy denies (fail-closed). |

## 9. Edge cases and design rationale

| Concern | Behavior |
|---|---|
| Empty policy | Denies everything — safe default for a governed surface. |
| Unmatched action | Denied. An allow rule must match explicitly. |
| Overlapping allow + deny | Deny always wins, regardless of rule order. |
| Secrets in audit log | Redacted at `AuditEvent::new` time; the persisted context never contains raw credentials. |
| Unknown element refs | `TargetResolver` returns `None`; actions cannot target unseen elements. |
| No persistence dependency | Audit events are plain `Serialize` values; the crate compiles and tests standalone. |

## 10. Tests

The `tests` module in `lib.rs` covers: deny precedence and fail-closed behavior, tool/host/path/intent matching, nested secret redaction (`args["nested"]["password"] == "[REDACTED]"`), and the resolver's known-refs-only contract.