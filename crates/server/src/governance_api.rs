//! Governance policy and audit HTTP endpoints.
use crate::{error, AppState};
use axum::{extract::{Query, State}, response::{IntoResponse, Response}, Json};
use pr_governance::{ActionContext, Governance, PolicyConfig, PolicyEngine};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const MAX_LIMIT: usize = 200;
const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<usize>,
    pub decision: Option<String>,
    pub agent: Option<String>,
    pub session: Option<String>,
}

#[derive(Debug, Serialize)]
struct PolicyResponse { enabled: bool, policy: PolicyConfig }

/// GET /governance/policy
pub async fn get_policy(State(state): State<Arc<AppState>>) -> Response {
    let (enabled, policy) = state.governance_snapshot().await;
    Json(PolicyResponse { enabled, policy }).into_response()
}

/// PUT /governance/policy
pub async fn put_policy(
    State(state): State<Arc<AppState>>,
    Json(policy): Json<PolicyConfig>,
) -> Response {
    if policy.rules.len() > 1000 {
        return error(axum::http::StatusCode::BAD_REQUEST, "too many policy rules (maximum 1000)");
    }
    state.replace_governance(policy).await;
    get_policy(State(state)).await
}

/// POST /governance/decide
pub async fn decide(
    State(state): State<Arc<AppState>>,
    Json(context): Json<ActionContext>,
) -> Response {
    let result = state.governance_decide(&context).await;
    match result {
        Ok(decision) => Json(decision).into_response(),
        Err(message) => error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, message),
    }
}

/// GET /governance/audit
pub async fn audit(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let mut rows = match state.db.list_audit_events() {
        Ok(rows) => rows,
        Err(e) => return error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    rows.reverse();
    rows.retain(|row| query.decision.as_deref().map_or(true, |v| row.decision.eq_ignore_ascii_case(v))
        && query.agent.as_deref().map_or(true, |v| row.agent == v)
        && query.session.as_deref().map_or(true, |v| row.session == v));
    rows.truncate(limit);
    Json(rows).into_response()
}

// Kept private to avoid exposing computer-service coupling until the service contract is stable.
#[allow(dead_code)]
pub(crate) fn governance_from_policy(policy: PolicyConfig) -> Governance {
    Governance::new(PolicyEngine::new(policy))
}
