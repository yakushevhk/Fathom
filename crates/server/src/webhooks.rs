use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

/// Inbound webhook payload triggering autonomous agent coworker execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundWebhookPayload {
    /// Webhook source: "github", "sentry", "email", "crm", "custom"
    pub source: String,
    /// Event type: e.g. "push", "pull_request", "issue", "error_alert", "lead_created"
    pub event: String,
    /// Optional Coworker ID or name to dispatch this task to
    #[serde(default)]
    pub coworker_id: Option<String>,
    /// Summary or prompt to trigger
    pub task: String,
    /// Extra metadata payload
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub status: String,
    pub session_id: String,
    pub message: String,
}

/// POST /api/v1/webhooks/inbound
/// Ingest external webhooks (GitHub, Sentry, Email, CRM) and trigger proactive Coworker runs.
pub async fn handle_inbound_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InboundWebhookPayload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let session_id = uuid::Uuid::now_v7().to_string();
    let prompt = format!(
        "[EVENT TRIGGER: {} / {}]\n{}\nMetadata: {}",
        payload.source,
        payload.event,
        payload.task,
        serde_json::to_string(&payload.metadata.unwrap_or_default()).unwrap_or_default()
    );

    tracing::info!(
        "Inbound webhook received from '{}' (event: '{}'), triggering session '{}'",
        payload.source,
        payload.event,
        session_id
    );

    // Record session creation in state database
    let pool = state.db.clone();
    let session_id_typed = pr_core::SessionId(session_id.clone());
    let query_task = prompt.clone();
    tokio::task::spawn_blocking(move || {
        let _ = pool.create_session(&session_id_typed, &query_task);
    });
    let resp = WebhookResponse {
        status: "accepted".to_string(),
        session_id,
        message: format!("Triggered autonomous session from webhook source '{}'", payload.source),
    };

    Ok((StatusCode::ACCEPTED, Json(resp)))
}
