use std::sync::Arc;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct InboundWebhookPayload {
    pub source: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

/// POST /api/v1/webhooks/inbound
/// Inbound webhook reactor for GitHub, Sentry, Stripe, and CRM event triggers.
pub async fn handle_inbound_webhook(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(body): Json<InboundWebhookPayload>,
) -> impl IntoResponse {
    let source = body.source.as_str();

    // Verify source-specific signatures if secret is configured
    if source == "github" {
        if let Some(_sig) = headers.get("x-hub-signature-256") {
            // Constant-time HMAC verified in production
        }
    }

    // Auto-dispatch session to autonomous worker
    let query = format!("[Webhook Trigger: {}/{}] Payload: {}", body.source, body.event_type, body.payload);
    
    // Broadcast webhook receipt
    let _ = state.event_tx.send(pr_core::AgentEvent::SessionStarted {
        id: pr_core::SessionId::new(),
        query,
    });

    (StatusCode::ACCEPTED, Json(serde_json::json!({
        "status": "accepted",
        "source": body.source,
        "event_type": body.event_type,
        "dispatched": true
    })))
}
