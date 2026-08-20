//! Optional Docker computer supervisor endpoints.
use crate::AppState;
use axum::{extract::{Path, State}, response::{IntoResponse, Response}, Json};
use serde_json::json;
use std::sync::Arc;

pub async fn list(State(state): State<Arc<AppState>>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"computer supervisor is not configured"}))).into_response();
    };
    match supervisor.list().await {
        Ok(containers) => Json(containers).into_response(),
        Err(error) => (axum::http::StatusCode::BAD_GATEWAY, Json(json!({"error": error.to_string()}))).into_response(),
    }
}

pub async fn ensure(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"computer supervisor is not configured"}))).into_response();
    };
    match supervisor.ensure(&agent_id).await {
        Ok(container) => Json(container).into_response(),
        Err(error) => (axum::http::StatusCode::BAD_GATEWAY, Json(json!({"error": error.to_string()}))).into_response(),
    }
}

pub async fn stop(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"computer supervisor is not configured"}))).into_response();
    };
    match supervisor.stop(&agent_id).await {
        Ok(()) => Json(json!({"agent_id": agent_id, "stopped": true})).into_response(),
        Err(error) => (axum::http::StatusCode::BAD_GATEWAY, Json(json!({"error": error.to_string()}))).into_response(),
    }
}

pub async fn reset(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({"error":"computer supervisor is not configured"}))).into_response();
    };
    match supervisor.reset(&agent_id).await {
        Ok(()) => Json(json!({"agent_id": agent_id, "reset": true})).into_response(),
        Err(error) => (axum::http::StatusCode::BAD_GATEWAY, Json(json!({"error": error.to_string()}))).into_response(),
    }
}
