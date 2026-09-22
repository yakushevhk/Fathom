use axum::{response::IntoResponse, Json};
use serde_json::json;

/// GET /api/v1/openapi.json
/// Returns the full OpenAPI 3.1 schema specification for Fathom HTTP API.
pub async fn openapi_spec() -> impl IntoResponse {
    let spec = json!({
        "openapi": "3.1.0",
        "info": {
            "title": "Fathom Autonomous Workforce & Coding Harness API",
            "version": "0.3.0",
            "description": "Enterprise-grade Rust runtime for autonomous AI workers, coding harnesses, computer use, and OSINT pipelines."
        },
        "paths": {
            "/api/v1/sessions": {
                "post": {
                    "summary": "Create and execute an autonomous session",
                    "responses": {
                        "200": { "description": "Session initialized" }
                    }
                },
                "get": {
                    "summary": "List all active and past sessions",
                    "responses": {
                        "200": { "description": "Array of sessions" }
                    }
                }
            },
            "/api/v1/webhooks/inbound": {
                "post": {
                    "summary": "Trigger proactive coworker session from external webhook",
                    "responses": {
                        "202": { "description": "Webhook accepted and queued" }
                    }
                }
            },
            "/api/v1/ws": {
                "get": {
                    "summary": "Multiplexed WebSocket connection for live telemetry and agent control",
                    "responses": {
                        "101": { "description": "Switching protocols to WebSocket" }
                    }
                }
            },
            "/health": {
                "get": {
                    "summary": "Health check endpoint",
                    "responses": {
                        "200": { "description": "Server healthy" }
                    }
                }
            },
            "/metrics": {
                "get": {
                    "summary": "Prometheus metrics exposition format",
                    "responses": {
                        "200": { "description": "Text metrics" }
                    }
                }
            }
        }
    });

    Json(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn spec_is_valid_openapi_31() {
        let resp = openapi_spec().await.into_response();
        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let spec: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(spec["openapi"], "3.1.0");
        assert!(spec["info"]["title"].as_str().unwrap().contains("Fathom"));
        assert!(spec["paths"].is_object());
        // Every declared path must be under /api/v1 or be /health|/metrics.
        for path in spec["paths"].as_object().unwrap().keys() {
            assert!(
                path.starts_with("/api/v1") || path == "/health" || path == "/metrics",
                "unexpected path {path}"
            );
        }
        // Every path must declare at least one response.
        for (path, ops) in spec["paths"].as_object().unwrap() {
            for (method, op) in ops.as_object().unwrap() {
                assert!(
                    op["responses"]
                        .as_object()
                        .map(|r| !r.is_empty())
                        .unwrap_or(false),
                    "{method} {path} declares no responses"
                );
            }
        }
    }
}
