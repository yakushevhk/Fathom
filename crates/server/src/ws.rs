use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { session_id: Option<String> },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsServerMessage {
    #[serde(rename = "pong")]
    Pong,
    #[serde(rename = "event")]
    Event { payload: serde_json::Value },
}

/// GET /api/v1/ws
/// Bidirectional multiplexed WebSocket connection for live event streaming and agent coordination.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.subscribe();
    let (pong_tx, mut pong_rx) = tokio::sync::mpsc::channel::<WsServerMessage>(16);

    // Spawn task forwarding internal broadcast events and pongs to WebSocket client
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(server_msg) = pong_rx.recv() => {
                    if let Ok(serialized) = serde_json::to_string(&server_msg) {
                        if sender.send(Message::Text(serialized)).await.is_err() {
                            break;
                        }
                    }
                }
                recv_res = event_rx.recv() => {
                    match recv_res {
                        Ok(event) => {
                            if let Ok(json_val) = serde_json::to_value(&event) {
                                let json_val = pr_governance::redact_secrets(&json_val);
                                let msg = WsServerMessage::Event { payload: json_val };
                                if let Ok(serialized) = serde_json::to_string(&msg) {
                                    if sender.send(Message::Text(serialized)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                            tracing::warn!("WebSocket subscriber lagged by {count} events, sending sync_required frame");
                            let sync_msg = serde_json::json!({
                                "type": "sync_required",
                                "skipped_events": count
                            });
                            if let Ok(serialized) = serde_json::to_string(&sync_msg) {
                                if sender.send(Message::Text(serialized)).await.is_err() {
                                    break;
                                }
                            }
                            continue;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    });

    // Task consuming client incoming messages
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<WsClientMessage>(&text) {
                        match client_msg {
                            WsClientMessage::Ping => {
                                let _ = pong_tx.send(WsServerMessage::Pong).await;
                            }
                            WsClientMessage::Subscribe { .. } => {
                                // Subscription tracking can be filtered per session
                            }
                        }
                    }
                }
                Message::Ping(_) => {
                    let _ = pong_tx.send(WsServerMessage::Pong).await;
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });
    // Terminate when either direction closes
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}
