//! Durable, redacted governed action timelines.
//!
//! Replay rows are deliberately separate from governance audit decisions:
//! audit records explain *why* an action was allowed or denied, while replay
//! rows describe the bounded execution timeline when a caller records it.
//! This module never creates synthetic execution records.

use crate::Persistence;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Maximum number of replay rows returned by one query.
pub const MAX_REPLAY_LIMIT: usize = 200;
/// Maximum size of each stored JSON/text payload.
pub const MAX_REPLAY_TEXT_BYTES: usize = 64 * 1024;
/// Maximum size of identifiers and references.
const MAX_REPLAY_FIELD_BYTES: usize = 2048;

/// A persisted governed action execution. Payload fields must be redacted;
/// `record_replay_action` applies a second defensive redaction pass.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReplayActionRow {
    pub id: String,
    pub agent: String,
    pub session: String,
    pub tool: String,
    pub args_redacted: String,
    pub decision: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub result_redacted: Option<String>,
    pub screenshot_before: Option<String>,
    pub screenshot_after: Option<String>,
    pub policy_version: String,
}

fn is_secret_key(key: &str) -> bool {
    let key: String = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(|character| character.to_lowercase())
        .collect();
    [
        "password", "passwd", "secret", "token", "apikey", "authorization",
        "credential", "privatekey", "accesskey", "clientsecret", "cookie",
    ]
    .iter()
    .any(|needle| key == *needle || key.ends_with(needle))
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut result = Map::new();
            for (key, value) in object {
                result.insert(
                    key.clone(),
                    if is_secret_key(key) {
                        Value::String("[REDACTED]".to_owned())
                    } else {
                        redact_value(value)
                    },
                );
            }
            Value::Object(result)
        }
        Value::Array(values) => Value::Array(values.iter().map(redact_value).collect()),
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            let marker = [
                "password", "passwd", "secret", "token", "api_key", "apikey",
                "authorization", "credential", "private_key", "access_key", "cookie",
            ];
            if marker.iter().any(|needle| lower.contains(needle)) {
                Value::String("[REDACTED]".to_owned())
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

fn redact_payload(value: &str, field: &str, required_json: bool) -> anyhow::Result<String> {
    if value.len() > MAX_REPLAY_TEXT_BYTES {
        anyhow::bail!("{field} exceeds {} bytes", MAX_REPLAY_TEXT_BYTES);
    }
    match serde_json::from_str::<Value>(value) {
        Ok(value) => Ok(serde_json::to_string(&redact_value(&value))?),
        Err(error) if required_json => anyhow::bail!("{field} must be valid JSON: {error}"),
        Err(_) => {
            // A non-JSON result is accepted only when it does not advertise a
            // credential-bearing field. Callers should pass result_redacted.
            let lower = value.to_ascii_lowercase();
            let has_secret_marker = [
                "password", "passwd", "secret", "token", "api_key", "apikey",
                "authorization", "credential", "private_key", "access_key", "cookie",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            if has_secret_marker {
                anyhow::bail!("{field} contains a possible secret field; provide redacted JSON");
            }
            Ok(value.to_owned())
        }
    }
}

fn validate_field(value: &str, field: &str) -> anyhow::Result<()> {
    if value.is_empty() {
        anyhow::bail!("{field} must not be empty");
    }
    if value.len() > MAX_REPLAY_FIELD_BYTES {
        anyhow::bail!("{field} exceeds {} bytes", MAX_REPLAY_FIELD_BYTES);
    }
    Ok(())
}

impl Persistence {
    /// Insert one replay action after defensively redacting JSON payloads.
    /// The caller supplies execution timestamps and duration; this method does
    /// not invent timeline events from audit rows.
    pub fn record_replay_action(&self, action: &ReplayActionRow) -> anyhow::Result<()> {
        for (value, field) in [
            (&action.id, "id"),
            (&action.agent, "agent"),
            (&action.session, "session"),
            (&action.tool, "tool"),
            (&action.decision, "decision"),
            (&action.started_at, "started_at"),
            (&action.policy_version, "policy_version"),
        ] {
            validate_field(value, field)?;
        }
        if action.duration_ms.is_some_and(|duration| duration < 0) {
            anyhow::bail!("duration_ms must not be negative");
        }
        let args = redact_payload(&action.args_redacted, "args_redacted", true)?;
        let result = action
            .result_redacted
            .as_deref()
            .map(|value| redact_payload(value, "result_redacted", false))
            .transpose()?;
        for (value, field) in [
            (action.screenshot_before.as_deref(), "screenshot_before"),
            (action.screenshot_after.as_deref(), "screenshot_after"),
            (action.completed_at.as_deref(), "completed_at"),
        ] {
            if let Some(value) = value {
                validate_field(value, field)?;
                if field.starts_with("screenshot_") {
                    let lower = value.to_ascii_lowercase();
                    if lower.starts_with("data:") || lower.contains("token=") || lower.contains("secret=") {
                        anyhow::bail!("{field} must be a non-sensitive reference");
                    }
                }
            }
        }

        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO replay_actions
             (id, agent, session, tool, args_redacted, decision, started_at,
              completed_at, duration_ms, result_redacted, screenshot_before,
              screenshot_after, policy_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                action.id,
                action.agent,
                action.session,
                action.tool,
                args,
                action.decision,
                action.started_at,
                action.completed_at,
                action.duration_ms,
                result,
                action.screenshot_before,
                action.screenshot_after,
                action.policy_version,
            ],
        )?;
        Ok(())
    }

    /// List recorded actions newest-first, bounded and optionally filtered.
    pub fn list_replay_actions(
        &self,
        session: Option<&str>,
        agent: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<ReplayActionRow>> {
        let limit = limit.min(MAX_REPLAY_LIMIT);
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, agent, session, tool, args_redacted, decision, started_at,
                    completed_at, duration_ms, result_redacted, screenshot_before,
                    screenshot_after, policy_version
             FROM replay_actions
             WHERE (?1 IS NULL OR session = ?1) AND (?2 IS NULL OR agent = ?2)
             ORDER BY started_at DESC, id DESC LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![session, agent, limit as i64], |row| {
                Ok(ReplayActionRow {
                    id: row.get(0)?,
                    agent: row.get(1)?,
                    session: row.get(2)?,
                    tool: row.get(3)?,
                    args_redacted: row.get(4)?,
                    decision: row.get(5)?,
                    started_at: row.get(6)?,
                    completed_at: row.get(7)?,
                    duration_ms: row.get(8)?,
                    result_redacted: row.get(9)?,
                    screenshot_before: row.get(10)?,
                    screenshot_after: row.get(11)?,
                    policy_version: row.get(12)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, started_at: &str) -> ReplayActionRow {
        ReplayActionRow {
            id: id.into(), agent: "agent-1".into(), session: "session-1".into(),
            tool: "browser.click".into(), args_redacted: r#"{"password":"oops","x":1}"#.into(),
            decision: "allow".into(), started_at: started_at.into(), completed_at: Some(started_at.into()),
            duration_ms: Some(12), result_redacted: Some(r#"{"token":"oops","ok":true}"#.into()),
            screenshot_before: None, screenshot_after: None, policy_version: "v1".into(),
        }
    }

    #[test]
    fn replay_rows_are_redacted_and_newest_first() {
        let db = Persistence::in_memory().unwrap();
        db.record_replay_action(&action("a", "2026-01-01T00:00:00Z")).unwrap();
        db.record_replay_action(&action("b", "2026-01-02T00:00:00Z")).unwrap();
        let rows = db.list_replay_actions(None, None, 10).unwrap();
        assert_eq!(rows[0].id, "b");
        assert!(!rows[0].args_redacted.contains("oops"));
        assert!(rows[0].args_redacted.contains("[REDACTED]"));
        assert!(!rows[0].result_redacted.as_deref().unwrap().contains("oops"));
    }

    #[test]
    fn replay_filters_and_bounds_are_enforced() {
        let db = Persistence::in_memory().unwrap();
        db.record_replay_action(&action("a", "2026-01-01T00:00:00Z")).unwrap();
        let mut other = action("b", "2026-01-02T00:00:00Z");
        other.agent = "agent-2".into();
        other.session = "session-2".into();
        db.record_replay_action(&other).unwrap();
        assert_eq!(db.list_replay_actions(Some("session-1"), None, 999).unwrap().len(), 1);
        assert_eq!(db.list_replay_actions(None, Some("agent-2"), 1).unwrap()[0].id, "b");
    }
}
