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
