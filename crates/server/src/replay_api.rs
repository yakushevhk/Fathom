//! Replay timeline endpoint over explicitly recorded, redacted action rows.
use crate::{error, AppState};
use axum::{extract::{Query, State}, response::{IntoResponse, Response}, Json};
use pr_persistence::MAX_REPLAY_LIMIT;
use serde::Deserialize;
use std::sync::Arc;

const DEFAULT_LIMIT: usize = 50;
const MAX_FILTER_BYTES: usize = 256;

#[derive(Debug, Deserialize)]
pub struct ReplayQuery {
    pub session: Option<String>,
    pub agent: Option<String>,
    pub limit: Option<usize>,
}

fn filter(value: Option<String>, name: &str) -> Result<Option<String>, Response> {
    let Some(value) = value else { return Ok(None); };
    if value.is_empty() || value.len() > MAX_FILTER_BYTES {
        return Err(error(axum::http::StatusCode::BAD_REQUEST, format!("{name} is invalid")));
    }
    Ok(Some(value))
}

/// GET /api/v1/replay — newest recorded governed actions first.
pub async fn list_replay(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ReplayQuery>,
) -> Response {
    let session = match filter(query.session, "session") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let agent = match filter(query.agent, "agent") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_REPLAY_LIMIT);
    match state.db.list_replay_actions(session.as_deref(), agent.as_deref(), limit) {
        Ok(actions) => Json(serde_json::json!({ "actions": actions })).into_response(),
        Err(db_error) => error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, db_error.to_string()),
    }
}
