//! Policy enforcement and audit primitives for governed computer actions.
//!
//! This crate intentionally has no persistence dependency.  Audit events are
//! plain serializable values and can be sent to any sink (including the
//! persistence crate) by an application boundary.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

/// The action being evaluated by the policy engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionContext {
    pub agent: String,
    pub session: String,
    pub tool: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub element: Option<String>,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub mcp_metadata: Option<Value>,
}

impl ActionContext {
    pub fn new(agent: impl Into<String>, session: impl Into<String>, tool: impl Into<String>, args: Value) -> Self {
        Self { agent: agent.into(), session: session.into(), tool: tool.into(), args, url: None, element: None, file: None, intent: None, mcp_metadata: None }
    }
}

/// Whether a matching policy rule grants or rejects an action.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect { Allow, Deny }

/// A rule uses simple, safe string matching; it never evaluates code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRule {
    pub effect: PolicyEffect,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
}

impl PolicyRule {
    pub fn allow() -> Self { Self { effect: PolicyEffect::Allow, tool: None, host: None, path: None, intent: None } }
    pub fn deny() -> Self { Self { effect: PolicyEffect::Deny, tool: None, host: None, path: None, intent: None } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyConfig {
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

/// The result of policy evaluation.  Policy evaluation fails closed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision { Allow, Deny }

impl Decision {
    pub fn is_allowed(self) -> bool { matches!(self, Self::Allow) }
    pub fn allowed(self) -> bool { self.is_allowed() }
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("invalid policy JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default)]
pub struct PolicyEngine { config: PolicyConfig }

impl PolicyEngine {
    pub fn new(config: PolicyConfig) -> Self { Self { config } }

    pub fn from_json(input: &str) -> Result<Self, PolicyError> {
        Ok(Self::new(serde_json::from_str(input)?))
    }

    pub fn config(&self) -> &PolicyConfig { &self.config }

    pub fn decide(&self, action: &ActionContext) -> Decision {
        // A deny rule always takes precedence, including when an allow rule
        // also matches.  An empty policy and unmatched actions are denied.
        let mut allowed = false;
        for rule in &self.config.rules {
            if !rule_matches(rule, action) { continue; }
            match rule.effect {
                PolicyEffect::Deny => return Decision::Deny,
                PolicyEffect::Allow => allowed = true,
            }
        }
        if allowed { Decision::Allow } else { Decision::Deny }
    }
}

fn rule_matches(rule: &PolicyRule, action: &ActionContext) -> bool {
    rule.tool.as_deref().map_or(true, |v| glob_match(v, &action.tool))
        && rule.host.as_deref().map_or(true, |v| action.url.as_deref().map_or(false, |u| host_matches(v, u)))
        && rule.path.as_deref().map_or(true, |v| path_matches(v, action))
        && rule.intent.as_deref().map_or(true, |v| action.intent.as_deref().map_or(false, |i| glob_match(v, i)))
}

fn path_matches(expected: &str, action: &ActionContext) -> bool {
    if let Some(file) = action.file.as_deref() {
        if glob_match(expected, file) { return true; }
    }
    let Some(url) = action.url.as_deref() else { return false; };
    if glob_match(expected, url) { return true; }
    url::Url::parse(url).ok().map_or(false, |parsed| glob_match(expected, parsed.path()))
}

fn host_matches(expected: &str, url: &str) -> bool {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_owned));
    host.map_or(false, |h| glob_match(expected, &h))
}

/// Small wildcard matcher (`*` means any sequence).  This avoids regex or
/// expression evaluation in policy files.
fn glob_match(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut i, mut j, mut star, mut mark) = (0usize, 0usize, None, 0usize);
    while j < t.len() {
        if i < p.len() && (p[i] == t[j]) { i += 1; j += 1; }
        else if i < p.len() && p[i] == b'*' { star = Some(i); i += 1; mark = j; }
        else if let Some(s) = star { i = s + 1; mark += 1; j = mark; }
        else { return false; }
    }
    while i < p.len() && p[i] == b'*' { i += 1; }
    i == p.len()
}

/// A persisted decision record.  Context is redacted before it is put here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub context: ActionContext,
    pub decision: AuditDecision,
}

impl AuditEvent {
    pub fn new(context: &ActionContext, decision: Decision) -> Self {
        Self { id: Uuid::now_v7().to_string(), timestamp: Utc::now(), context: redact_action_context(context), decision: decision.into() }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuditDecision { Allow, Deny }

impl From<Decision> for AuditDecision {
    fn from(value: Decision) -> Self { if value.is_allowed() { Self::Allow } else { Self::Deny } }
}

/// Recursively redact values below common credential/secret keys.
pub fn redact_secrets(value: &Value) -> Value {
    match value {
        Value::Object(obj) => {
            let mut result = Map::new();
            for (key, val) in obj {
                if is_secret_key(key) { result.insert(key.clone(), Value::String("[REDACTED]".into())); }
                else { result.insert(key.clone(), redact_secrets(val)); }
            }
            Value::Object(result)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_secrets).collect()),
        other => other.clone(),
    }
}

fn is_secret_key(key: &str) -> bool {
    let mut normalized = String::with_capacity(key.len() + 8);
    for (index, ch) in key.chars().enumerate() {
        if ch == '-' || ch == ' ' || ch == '_' { normalized.push('_'); continue; }
        if ch.is_ascii_uppercase() && index > 0 { normalized.push('_'); }
        normalized.push(ch.to_ascii_lowercase());
    }
    ["password", "passwd", "secret", "token", "api_key", "apikey", "authorization", "cookie", "credential", "private_key", "access_key", "client_secret", "access_token"]
        .iter().any(|needle| normalized == *needle || normalized.starts_with(&format!("{needle}_")) || normalized.ends_with(&format!("_{needle}")) || normalized.contains(&format!("_{needle}_")))
}

pub fn redact_action_context(action: &ActionContext) -> ActionContext {
    let mut copy = action.clone();
    copy.args = redact_secrets(&copy.args);
    if copy.tool == "computer_type" || copy.tool == "browser_type" || copy.tool == "computer_secret" {
        if let Value::Object(object) = &mut copy.args {
            for key in ["text", "value", "secret"] { if object.contains_key(key) { object.insert(key.to_owned(), Value::String("[REDACTED]".into())); } }
        }
    }
    copy.mcp_metadata = copy.mcp_metadata.as_ref().map(redact_secrets);
    copy.url = copy.url.as_deref().map(redact_url);
    copy
}

fn redact_url(raw: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(raw) else { return "[REDACTED_URL]".to_owned(); };
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.set_fragment(None);
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    let mut had_query = false;
    for (key, value) in parsed.query_pairs() {
        had_query = true;
        let safe_value = if is_secret_key(&key) { "[REDACTED]".to_owned() } else { value.into_owned() };
        serializer.append_pair(&key, &safe_value);
    }
    let query = serializer.finish();
    parsed.set_query(if had_query { Some(query.as_str()) } else { None });
    parsed.to_string()
}

/// Resolves only references that were explicitly present in a snapshot.
#[derive(Debug, Clone, Default)]
pub struct TargetResolver { targets: HashMap<String, Value> }

impl TargetResolver {
    pub fn new() -> Self { Self::default() }
    pub fn register(&mut self, reference: impl Into<String>, target: Value) { self.targets.insert(reference.into(), target); }
    pub fn resolve(&self, reference: &str) -> Option<&Value> { self.targets.get(reference) }
    pub fn contains(&self, reference: &str) -> bool { self.targets.contains_key(reference) }
    pub fn clear(&mut self) { self.targets.clear(); }
}

#[derive(Debug, Error)]
pub enum GovernanceError {
    #[error("audit sink failed: {0}")]
    Sink(String),
}

pub trait AuditSink: Send + Sync {
    fn record(&self, event: &AuditEvent) -> Result<(), String>;
}

/// Facade combining policy decisions with an optional audit sink.
#[derive(Clone, Default)]
pub struct Governance { policy: PolicyEngine, sink: Option<Arc<dyn AuditSink>> }

impl Governance {
    pub fn new(policy: PolicyEngine) -> Self { Self { policy, sink: None } }
    pub fn with_audit_sink(mut self, sink: Arc<dyn AuditSink>) -> Self { self.sink = Some(sink); self }
    pub fn policy(&self) -> &PolicyEngine { &self.policy }
    pub fn authorize(&self, context: &ActionContext) -> Decision { self.policy.decide(context) }
    pub fn record(&self, event: &AuditEvent) -> Result<(), GovernanceError> {
        if let Some(sink) = &self.sink { sink.record(event).map_err(GovernanceError::Sink)?; }
        Ok(())
    }
    pub fn authorize_and_record(&self, context: &ActionContext) -> Result<Decision, GovernanceError> {
        let decision = self.authorize(context);
        self.record(&AuditEvent::new(context, decision))?;
        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ctx() -> ActionContext { ActionContext::new("a", "s", "browser.click", serde_json::json!({"nested":{"password":"raw"}})) }

    #[test]
    fn deny_precedence_and_fail_closed() {
        assert_eq!(PolicyEngine::new(PolicyConfig::default()).decide(&ctx()), Decision::Deny);
        let p = PolicyConfig { rules: vec![PolicyRule { effect: PolicyEffect::Allow, tool: Some("browser.*".into()), host: None, path: None, intent: None }, PolicyRule { effect: PolicyEffect::Deny, tool: Some("browser.click".into()), host: None, path: None, intent: None }] };
        assert_eq!(PolicyEngine::new(p).decide(&ctx()), Decision::Deny);
    }

    #[test]
    fn matches_tool_host_path_intent() {
        let mut c = ctx(); c.url = Some("https://example.com/a".into()); c.intent = Some("read".into());
        let p = PolicyConfig { rules: vec![PolicyRule { effect: PolicyEffect::Allow, tool: Some("browser.*".into()), host: Some("example.com".into()), path: Some("https://example.com/*".into()), intent: Some("read".into()) }] };
        assert_eq!(PolicyEngine::new(p).decide(&c), Decision::Allow);
    }

    #[test]
    fn redacts_nested_secrets() {
        let event = AuditEvent::new(&ctx(), Decision::Allow);
        assert_eq!(event.context.args["nested"]["password"], "[REDACTED]");
    }

    #[test]
    fn resolver_only_known_refs() {
        let mut r = TargetResolver::new(); r.register("e1", serde_json::json!({"role":"button"}));
        assert!(r.resolve("e1").is_some()); assert!(r.resolve("e2").is_none());
    }
}
