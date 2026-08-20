//! Safe, operator-triggered notification delivery.
use crate::{error, json, AppState};
use axum::{extract::State, http::StatusCode, response::Response, Json};
use pr_core::{NotificationChannel, Notifier, SessionId, SessionOutput};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

const DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestNotificationRequest {
    /// Selects a channel already configured in `[notifications]`. This is a
    /// symbolic name, never a destination URL or arbitrary address.
    pub channel: String,
}

/// POST /api/v1/notifications/test
///
/// Sends a bounded test message through exactly one configured channel. The
/// endpoint deliberately does not accept destination details, credentials,
/// or message content from the caller.
pub async fn test(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TestNotificationRequest>,
) -> Response {
    let channel = body.channel.trim().to_ascii_lowercase();
    let configured = &state.config.notifications;
    let selected = match channel.as_str() {
        "webhook" if !configured.webhook_url.trim().is_empty() => {
            NotificationChannel::Webhook { url: configured.webhook_url.trim().to_string() }
        }
        "email" if !configured.email_to.trim().is_empty() => {
            let smtp_host = if configured.smtp_host.trim().is_empty() { "localhost".to_string() } else { configured.smtp_host.trim().to_string() };
            let from = if configured.email_from.trim().is_empty() { "fathom@localhost".to_string() } else { configured.email_from.trim().to_string() };
            NotificationChannel::Email {
                smtp_host,
                smtp_port: configured.smtp_port,
                from,
                to: configured.email_to.trim().to_string(),
                username: configured.smtp_username.clone(),
                password: configured.smtp_password.clone(),
            }
        }
        "telegram" if !configured.telegram_bot_token.trim().is_empty() && !configured.telegram_chat_id.trim().is_empty() => {
            NotificationChannel::Telegram {
                bot_token: configured.telegram_bot_token.trim().to_string(),
                chat_id: configured.telegram_chat_id.trim().to_string(),
            }
        }
        "webhook" | "email" | "telegram" => {
            return error(StatusCode::CONFLICT, format!("notification channel '{channel}' is not configured"));
        }
        _ => return error(StatusCode::BAD_REQUEST, "channel must be one of: webhook, email, telegram"),
    };

    let notifier = Notifier::new(vec![selected]);
    let session = SessionOutput {
        session_id: SessionId("operator-test".to_string()),
        output_dir: std::path::PathBuf::from("."),
        synthesis: "Fathom notification test".to_string(),
        total_tokens: 0,
        total_agents: 0,
    };
    let delivery = tokio::time::timeout(DELIVERY_TIMEOUT, notifier.notify_completion(&session)).await;
    match delivery {
        Err(_) => error(StatusCode::GATEWAY_TIMEOUT, "notification delivery timed out"),
        Ok(Err(_)) => {
            // Avoid echoing configured addresses or transport details to the
            // caller; the symbolic channel is the only safe diagnostic.
            tracing::warn!(channel = %channel, "configured notification delivery failed");
            error(StatusCode::BAD_GATEWAY, "notification delivery failed")
        }
        Ok(Ok(())) => json(StatusCode::OK, serde_json::json!({ "channel": channel, "status": "sent" })),
    }
}
