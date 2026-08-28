//! AG-UI compatibility bridge.
//!
//! This is intentionally a narrow transport adapter: it exposes the existing
//! agent event bus as SSE envelopes, but does not pretend to implement the
//! complete AG-UI command or state protocol. Event payloads are redacted at
//! the boundary before they leave the server.

use crate::AppState;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{sse::{Event, KeepAlive, Sse}, IntoResponse, Response},
    Json,
};
use futures::stream::Stream;
use pr_core::AgentEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::VecDeque,
    convert::Infallible,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;

/// Stable envelope emitted by the compatibility bridge. `event_type` is a
/// small, documented subset of AG-UI-like lifecycle names; `data` retains the
/// corresponding Fathom event shape so clients do not need unsupported
/// protocol features or speculative command handling.
#[derive(Debug, Clone, Serialize)]
pub struct AgUiEvent {
    pub protocol: &'static str,
    pub version: &'static str,
    pub event_type: &'static str,
    pub event_id: Option<String>,
    pub timestamp_ms: u128,
    pub data: Value,
}

impl AgUiEvent {
    fn from_agent_event(event: AgentEvent) -> Self {
        let (event_type, event_id) = match &event {
            AgentEvent::SessionStarted { id, .. } => ("RUN_STARTED", Some(id.0.clone())),
            AgentEvent::SessionCompleted { id, .. } => ("RUN_FINISHED", Some(id.0.clone())),
            AgentEvent::SessionFailed { id, .. } => ("RUN_ERROR", Some(id.0.clone())),
            AgentEvent::AgentSpawned { id, .. } => ("STEP_STARTED", Some(id.0.clone())),
            AgentEvent::AgentCompleted { id, .. } => ("STEP_FINISHED", Some(id.0.clone())),
            AgentEvent::AgentFailed { id, .. } => ("STEP_ERROR", Some(id.0.clone())),
            AgentEvent::LlmStreamChunk { agent_id, .. } => ("TEXT_MESSAGE_CONTENT", Some(agent_id.0.clone())),
            AgentEvent::ThinkingChunk { agent_id, .. } => ("THINKING_CONTENT", Some(agent_id.0.clone())),
            AgentEvent::ToolCallStarted { agent_id, .. } => ("TOOL_CALL_START", Some(agent_id.0.clone())),
            AgentEvent::ToolCallCompleted { agent_id, .. } => ("TOOL_CALL_END", Some(agent_id.0.clone())),
            AgentEvent::Finding { agent_id, .. } => ("STATE_DELTA", Some(agent_id.0.clone())),
            AgentEvent::AgentStateChanged { id, .. } => ("STATE_DELTA", Some(id.0.clone())),
            AgentEvent::QuestionAsked { agent_id, .. } => ("INTERRUPT", Some(agent_id.0.clone())),
            AgentEvent::ApprovalRequested { agent_id, .. } => ("INTERRUPT", Some(agent_id.0.clone())),
            AgentEvent::SessionForked { child_id, .. } => ("RUN_STARTED", Some(child_id.0.clone())),
            AgentEvent::FileChangeUndone { session_id, .. } => ("STATE_DELTA", Some(session_id.0.clone())),
            AgentEvent::TitleGenerated { session_id, .. } => ("STATE_DELTA", Some(session_id.0.clone())),
        };
        let mut data = serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({
            "type": "serialization_error",
            "error": "event could not be serialized",
        }));
        redact_value(&mut data);
        Self {
            protocol: "fathom.ag-ui",
            version: "1",
            event_type,
            event_id,
            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
            data,
        }
    }

    fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            protocol: "fathom.ag-ui",
            version: "1",
            event_type: "ERROR",
            event_id: None,
            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
            data: serde_json::json!({"error": {"code": code, "message": message.into()}}),
        }
    }
}

/// Redact values under common credential-bearing fields. This is deliberately
/// conservative: unknown fields remain available for compatibility, while
/// anything named like a credential is never sent over this bridge.
fn redact_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                let key_lower = key.to_ascii_lowercase();
                if ["api_key", "apikey", "authorization", "password", "secret", "token", "private_key", "credential"]
                    .iter().any(|needle| key_lower.contains(needle))
                {
                    *value = Value::String("[REDACTED]".to_string());
                } else {
                    redact_value(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_value),
        _ => {}
    }
}

type AgUiStream = Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>;

const EVENT_RING_CAPACITY: usize = 256;

#[derive(Clone)]
struct RecordedEvent {
    sequence: u64,
    envelope: AgUiEvent,
}

struct EventStore {
    ring: Mutex<VecDeque<RecordedEvent>>,
    next_sequence: std::sync::atomic::AtomicU64,
    tx: broadcast::Sender<RecordedEvent>,
}

static EVENT_STORE: OnceLock<Arc<EventStore>> = OnceLock::new();

fn event_store(source: &broadcast::Sender<AgentEvent>) -> Arc<EventStore> {
    EVENT_STORE
        .get_or_init(|| {
            let (tx, _) = broadcast::channel(EVENT_RING_CAPACITY * 2);
            let store = Arc::new(EventStore {
                ring: Mutex::new(VecDeque::with_capacity(EVENT_RING_CAPACITY)),
                next_sequence: std::sync::atomic::AtomicU64::new(0),
                tx,
            });
            let mut rx = source.subscribe();
            let collector = Arc::clone(&store);
            tokio::spawn(async move {
                loop {
                    let agent_event = match rx.recv().await {
                        Ok(event) => event,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                    };
                    let sequence = collector
                        .next_sequence
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        .saturating_add(1);
                    let recorded = RecordedEvent {
                        sequence,
                        envelope: AgUiEvent::from_agent_event(agent_event),
                    };
                    if let Ok(mut ring) = collector.ring.lock() {
                        ring.push_back(recorded.clone());
                        while ring.len() > EVENT_RING_CAPACITY {
                            ring.pop_front();
                        }
                    }
                    let _ = collector.tx.send(recorded);
                }
            });
            store
        })
        .clone()
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct EventCursor {
    #[serde(default, alias = "lastEventId")]
    last_event_id: Option<String>,
}

fn requested_cursor(headers: &HeaderMap, query: &EventCursor) -> Option<u64> {
    headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .or(query.last_event_id.as_deref())
        .and_then(|value| value.trim().parse::<u64>().ok())
}

/// `GET /api/v1/ag-ui/events` — AG-UI-like SSE compatibility stream.
pub(crate) async fn events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventCursor>,
) -> AgUiStream {
    let store = event_store(&state.event_tx);
    Sse::new(event_stream(store, requested_cursor(&headers, &query))).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

/// `GET /api/v1/ag-ui/health` — reports bridge capabilities without claiming
/// support for commands, tool execution, or arbitrary component protocols.
pub(crate) async fn health() -> Response {
    Json(serde_json::json!({
        "ok": true,
        "protocol": "fathom.ag-ui",
        "version": "1",
        "transport": "sse",
        "capabilities": {"events": true, "commands": false, "state_mutation": false},
    })).into_response()
}

fn sse_event(recorded: RecordedEvent) -> Result<Event, Infallible> {
    let event_type = recorded.envelope.event_type;
    let data = serde_json::to_string(&recorded.envelope).unwrap_or_else(|_| {
        serde_json::to_string(&AgUiEvent::error("serialization_error", "event envelope could not be serialized"))
            .unwrap_or_else(|_| "{\"event_type\":\"ERROR\"}".to_string())
    });
    Ok(Event::default()
        .id(recorded.sequence.to_string())
        .event(event_type)
        .data(data))
}

fn reset_event(cursor: u64, oldest: u64) -> RecordedEvent {
    RecordedEvent {
        // Keep the oldest replayable event in the stream after RESET. The
        // reset marker itself represents the cursor immediately before it.
        sequence: oldest.saturating_sub(1),
        envelope: AgUiEvent {
            protocol: "fathom.ag-ui",
            version: "1",
            event_type: "RESET",
            event_id: None,
            timestamp_ms: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis(),
            data: serde_json::json!({"error": {"code": "cursor_too_old", "message": format!("event cursor {cursor} is no longer available"), "oldest_event_id": oldest}}),
        },
    }
}

fn event_stream(
    store: Arc<EventStore>,
    cursor: Option<u64>,
) -> Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> {
    // Subscribe before taking the snapshot so events emitted during replay are
    // queued for the live phase rather than falling through the gap.
    let rx = store.tx.subscribe();
    let replay = store.ring.lock().map(|ring| {
        let oldest = ring.front().map(|event| event.sequence);
        let mut events = Vec::new();
        if let Some(cursor) = cursor {
            match oldest {
                Some(oldest) if cursor.saturating_add(1) < oldest => events.push(reset_event(cursor, oldest)),
                _ => {}
            }
            events.extend(ring.iter().filter(|event| event.sequence > cursor).cloned());
        } else {
            events.extend(ring.iter().cloned());
        }
        events
    }).unwrap_or_default();
    Box::pin(futures::stream::unfold(
        (replay.into_iter(), rx),
        |(mut replay, mut rx)| async move {
            if let Some(event) = replay.next() {
                return Some((sse_event(event), (replay, rx)));
            }
            loop {
                match rx.recv().await {
                    Ok(event) => return Some((sse_event(event), (replay, rx))),
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        let envelope = AgUiEvent::error("event_lagged", format!("{count} events were skipped"));
                        let data = serde_json::to_string(&envelope).unwrap_or_else(|_| "{\"event_type\":\"ERROR\"}".to_string());
                        return Some((Ok(Event::default().event("ERROR").data(data)), (replay, rx)));
                    }
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_nested_credentials_without_dropping_safe_data() {
        let mut value = serde_json::json!({"args": {"api_key": "secret", "query": "safe"}, "items": [{"password": "x"}]});
        redact_value(&mut value);
        assert_eq!(value["args"]["api_key"], "[REDACTED]");
        assert_eq!(value["args"]["query"], "safe");
        assert_eq!(value["items"][0]["password"], "[REDACTED]");
    }
}
