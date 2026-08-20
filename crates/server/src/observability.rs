//! Bounded operational observability endpoints.
use crate::{error, json, AppState};
use axum::{extract::State, http::StatusCode, response::Response};
use serde::Serialize;
use std::sync::Arc;

const MAX_AUDIT_ROWS: usize = 10_000;

#[derive(Debug, Serialize)]
pub struct ObservabilitySummary {
    pub active_sessions: usize,
    pub sessions_total: u64,
    pub agents_spawned: u64,
    pub tool_calls: u64,
    pub tokens_used: u64,
    pub audit_events: usize,
    pub audit_denials: usize,
    /// True when the bounded audit sample reached its hard limit.
    pub audit_counts_truncated: bool,
}

/// GET /api/v1/observability/summary
///
/// Returns live process metrics and bounded audit counts. Audit rows are
/// loaded with a hard limit so a damaged/very old database cannot make this
/// endpoint unbounded.
pub async fn summary(State(state): State<Arc<AppState>>) -> Response {
    let active_sessions = match state.active_sessions.lock() {
        Ok(sessions) => sessions.len(),
        Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "active session state unavailable"),
    };
    let rows = match state.db.list_audit_events_limited(Some(MAX_AUDIT_ROWS)) {
        Ok(rows) => rows,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to read audit counts: {e}")),
    };
    let audit_denials = rows.iter().filter(|row| row.decision.eq_ignore_ascii_case("deny")).count();
    json(StatusCode::OK, serde_json::json!(ObservabilitySummary {
        active_sessions,
        sessions_total: state.metrics.sessions_total.get(),
        agents_spawned: state.metrics.agents_spawned.get(),
        tool_calls: state.metrics.tool_calls.get(),
        tokens_used: state.metrics.tokens_used.get(),
        audit_events: rows.len(),
        audit_denials,
        audit_counts_truncated: rows.len() == MAX_AUDIT_ROWS,
    }))
}
