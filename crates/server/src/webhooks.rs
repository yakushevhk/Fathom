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
    let _source = body.source.as_str();

    // Verify source-specific signatures if secret is configured
    if let Ok(secret) = std::env::var("FATHOM_WEBHOOK_SECRET") {
        if !secret.is_empty() {
            let sig = headers.get("x-fathom-signature")
                .or_else(|| headers.get("x-hub-signature-256"))
                .and_then(|v| v.to_str().ok());
            
            let Some(sig_str) = sig else {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Missing webhook signature header"
                }))).into_response();
            };

            // Clean sha256= prefix if present
            let hex_sig = sig_str.strip_prefix("sha256=").unwrap_or(sig_str);
            let payload_bytes = serde_json::to_vec(&body.payload).unwrap_or_default();

            use ring::hmac;
            let s_key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
            let tag = hmac::sign(&s_key, &payload_bytes);
            let expected_sig = tag.as_ref().iter().map(|b| format!("{:02x}", b)).collect::<String>();

            if hex_sig.to_lowercase() != expected_sig.to_lowercase() {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
                    "error": "Invalid webhook signature"
                }))).into_response();
            }
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
    }))).into_response()
}
