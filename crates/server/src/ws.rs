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

    // Spawn task forwarding internal broadcast events to WebSocket client
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(json_val) = serde_json::to_value(&event) {
                let msg = WsServerMessage::Event { payload: json_val };
                if let Ok(serialized) = serde_json::to_string(&msg) {
                    if sender.send(Message::Text(serialized)).await.is_err() {
                        break;
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
                                // handle heartbeat
                            }
                            WsClientMessage::Subscribe { .. } => {
                                // handle session-filtered subscription
                            }
                        }
                    }
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
