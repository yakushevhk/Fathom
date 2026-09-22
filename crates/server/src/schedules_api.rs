use crate::{error, json, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use chrono::Utc;
use pr_persistence::ScheduleRow;
use serde::Deserialize;
use std::sync::Arc;

const MAX_ID: usize = 128;
const MAX_CRON: usize = 256;
const MAX_TIMEZONE: usize = 128;
const MAX_QUERY: usize = 20_000;

#[derive(Debug, Deserialize)]
pub struct ScheduleRequest {
    pub coworker_id: String,
    #[serde(alias = "cron")]
    pub cron_expression: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    pub query: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub next_run: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub limit: Option<usize>,
}

fn default_timezone() -> String {
    "UTC".to_owned()
}
fn default_enabled() -> bool {
    true
}
#[allow(clippy::result_large_err)]
fn bounded(value: &str, max: usize, field: &str) -> Result<String, Response> {
    let value = value.trim();
    if value.is_empty() {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("{field} must not be empty"),
        ));
    }
    if value.chars().count() > max {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("{field} exceeds maximum length of {max}"),
        ));
    }
    Ok(value.to_owned())
}
#[allow(clippy::result_large_err)]
fn valid_id(value: &str, field: &str) -> Result<String, Response> {
    let value = bounded(value, MAX_ID, field)?;
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            format!("{field} is invalid"),
        ));
    }
    Ok(value)
}
#[allow(clippy::result_large_err)]
fn fields(
    body: &ScheduleRequest,
) -> Result<(String, String, String, String, Option<String>), Response> {
    let coworker_id = valid_id(&body.coworker_id, "coworker_id")?;
    let cron_expression = bounded(&body.cron_expression, MAX_CRON, "cron_expression")?;
    let timezone = bounded(&body.timezone, MAX_TIMEZONE, "timezone")?;
    let query = bounded(&body.query, MAX_QUERY, "query")?;
    let next_run = body
        .next_run
        .as_deref()
        .map(|value| bounded(value, 64, "next_run"))
        .transpose()?;
    Ok((coworker_id, cron_expression, timezone, query, next_run))
}
fn row_json(row: &ScheduleRow) -> Response {
    json(StatusCode::OK, serde_json::json!({"schedule": row}))
}

pub(crate) async fn list_schedules(State(state): State<Arc<AppState>>) -> Response {
    match state.db.list_schedules() {
        Ok(rows) => json(
            StatusCode::OK,
            serde_json::json!({"schedules": rows, "count": rows.len()}),
        ),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub(crate) async fn get_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let id = match valid_id(&id, "schedule id") {
        Ok(id) => id,
        Err(e) => return e,
    };
    match state.db.get_schedule(&id) {
        Ok(Some(row)) => row_json(&row),
        Ok(None) => error(StatusCode::NOT_FOUND, "schedule not found"),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub(crate) async fn create_schedule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ScheduleRequest>,
) -> Response {
    let (coworker_id, cron, timezone, query, next_run) = match fields(&body) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match state.db.create_schedule(
        &coworker_id,
        &cron,
        &timezone,
        &query,
        body.enabled,
        next_run.as_deref(),
    ) {
        Ok(row) => json(StatusCode::CREATED, serde_json::json!({"schedule": row})),
        Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

pub(crate) async fn update_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ScheduleRequest>,
) -> Response {
    let id = match valid_id(&id, "schedule id") {
        Ok(id) => id,
        Err(e) => return e,
    };
    let (coworker_id, cron, timezone, query, next_run) = match fields(&body) {
        Ok(v) => v,
        Err(e) => return e,
    };
    match state.db.update_schedule(
        &id,
        &coworker_id,
        &cron,
        &timezone,
        &query,
        body.enabled,
        next_run.as_deref(),
    ) {
        Ok(Some(row)) => row_json(&row),
        Ok(None) => error(StatusCode::NOT_FOUND, "schedule not found"),
        Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

pub(crate) async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let id = match valid_id(&id, "schedule id") {
        Ok(id) => id,
        Err(e) => return e,
    };
    match state.db.delete_schedule(&id) {
        Ok(true) => json(StatusCode::OK, serde_json::json!({"deleted": true})),
        Ok(false) => error(StatusCode::NOT_FOUND, "schedule not found"),
        Err(e) => error(StatusCode::BAD_REQUEST, e.to_string()),
    }
}

/// Claim due rows for a bounded scheduler tick; no jobs are spawned here.
pub(crate) async fn claim_schedules(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClaimRequest>,
) -> Response {
    let limit = body.limit.unwrap_or(25).min(100);
    match state.db.claim_due_schedules(Utc::now(), limit) {
        Ok(rows) => json(
            StatusCode::OK,
            serde_json::json!({"schedules": rows, "count": rows.len()}),
        ),
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_trims_and_accepts() {
        assert_eq!(bounded("  hello  ", 64, "f").unwrap(), "hello");
    }

    #[test]
    fn bounded_empty_rejected() {
        assert!(bounded("   ", 64, "f").is_err());
        assert!(bounded("", 64, "f").is_err());
    }

    #[test]
    fn bounded_overlong_rejected() {
        let v = "x".repeat(65);
        assert!(bounded(&v, 64, "f").is_err());
    }

    #[test]
    fn bounded_counts_chars_not_bytes() {
        // 10 multi-byte chars should pass a 64-char limit.
        let v = "テスト".repeat(8); // 64 chars, >64 bytes
        assert!(bounded(&v, 64, "f").is_ok());
    }

    #[test]
    fn valid_id_accepts_alnum_dash_underscore_dot() {
        assert!(valid_id("worker-1_a.b", "id").is_ok());
    }

    #[test]
    fn valid_id_rejects_spaces_and_symbols() {
        for bad in [
            "has space",
            "sl/ash",
            "col:on",
            "semi;colon",
            "at@sign",
            "uni☃",
        ] {
            assert!(valid_id(bad, "id").is_err(), "{bad} accepted");
        }
    }

    #[test]
    fn valid_id_rejects_empty() {
        assert!(valid_id("", "id").is_err());
    }

    #[test]
    fn valid_id_enforces_max_len() {
        let v = "a".repeat(MAX_ID + 1);
        assert!(valid_id(&v, "id").is_err());
    }

    #[test]
    fn default_timezone_and_enabled() {
        assert_eq!(default_timezone(), "UTC");
        assert!(default_enabled());
    }
}
