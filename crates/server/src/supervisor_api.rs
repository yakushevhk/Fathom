//! Optional Docker computer supervisor endpoints.
use crate::AppState;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;

pub async fn list(State(state): State<Arc<AppState>>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"computer supervisor is not configured"})),
        )
            .into_response();
    };
    match supervisor.list().await {
        Ok(containers) => Json(containers).into_response(),
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

pub async fn ensure(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"computer supervisor is not configured"})),
        )
            .into_response();
    };
    match supervisor.ensure(&agent_id).await {
        Ok(container) => Json(container).into_response(),
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

pub async fn stop(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"computer supervisor is not configured"})),
        )
            .into_response();
    };
    match supervisor.stop(&agent_id).await {
        Ok(()) => Json(json!({"agent_id": agent_id, "stopped": true})).into_response(),
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

pub async fn reset(State(state): State<Arc<AppState>>, Path(agent_id): Path<String>) -> Response {
    let Some(supervisor) = state.supervisor.as_ref() else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"computer supervisor is not configured"})),
        )
            .into_response();
    };
    match supervisor.reset(&agent_id).await {
        Ok(()) => Json(json!({"agent_id": agent_id, "reset": true})).into_response(),
        Err(error) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(json!({"error": error.to_string()})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use pr_core::AppConfig;
    use pr_persistence::{JobsDb, Persistence};
    use serde_json::Value;

    fn test_state() -> Arc<AppState> {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let jobs = Arc::new(JobsDb::in_memory().unwrap());
        let mut config = AppConfig::default();
        config.memory.enabled = false;
        AppState::with_db_and_jobs(config, db, jobs)
    }

    #[tokio::test]
    async fn list_without_supervisor_returns_503() {
        let resp = list(State(test_state())).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["error"].as_str().unwrap().contains("not configured"));
    }
}
