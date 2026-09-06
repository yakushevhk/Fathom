//! Fathom Desktop — native GPUI desktop application for autonomous AI coworkers
//! with 1:1 OpenBot parity.

pub mod api;
pub mod app;
pub mod components;
pub mod daemon;
pub mod state;
pub mod theme;

use gpui::{
    App, Application, Bounds, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Launching Fathom Desktop (GPUI)...");

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1220.0), px(780.0)), cx);
        let app_state = Arc::new(state::AppState::new());

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| app::DesktopApp::new(app_state, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_initialization() {
        let state = state::AppState::new();
        assert_eq!(state.active_tab(), state::NavigationTab::Channels);
        assert!(!*state.is_engine_running.read());
        assert_eq!(state.messages.read().len(), 0);
    }

    #[test]
    fn test_add_and_clear_messages() {
        let state = state::AppState::new();
        state.add_message(state::ChatMessage {
            id: "msg-1".to_string(),
            role: "user".to_string(),
            content: "Hello Fathom".to_string(),
            thinking: None,
            tool_name: None,
            tool_status: None,
            tool_input: None,
            tool_output: None,
            question: None,
            request_id: None,
            timestamp: "12:00:00".to_string(),
            expanded: false,
        });

        assert_eq!(state.messages.read().len(), 1);
        assert_eq!(state.messages.read()[0].content, "Hello Fathom");
    }

    #[test]
    fn test_gallery_card_deserialization() {
        let json_data = r#"{
            "title": "Contract Signature",
            "subtitle": "Review vendor NDA",
            "status": "APPROVED",
            "status_tone": "positive",
            "fields": [
                {"label": "Party", "value": "Acme Corp"},
                {"label": "Risk", "value": "Low"}
            ]
        }"#;

        let res: Result<components::gallery::RecordCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.title, "Contract Signature");
        assert_eq!(card.fields.len(), 2);
    }

    #[test]
    fn test_confirm_action_card_deserialization() {
        let json_data = r#"{
            "request_id": "req-99",
            "title": "Execute Bash Command",
            "description": "Install dependencies in /workspace",
            "command_or_tool": "cargo build --release",
            "risk_level": "medium"
        }"#;

        let res: Result<components::gallery::ConfirmActionCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.request_id, "req-99");
        assert_eq!(card.risk_level, "medium");
    }

    #[test]
    fn test_skill_draft_card_deserialization() {
        let json_data = r#"{
            "slug": "review-pr",
            "name": "PR Reviewer",
            "description": "Examines pull requests for bugs and security defects",
            "tools": ["github_pr", "shell_exec"],
            "instructions": "Follow zero-trust review standards"
        }"#;

        let res: Result<components::gallery::SkillDraftCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.slug, "review-pr");
        assert_eq!(card.tools.len(), 2);
    }

    #[test]
    fn test_agent_handoff_card_deserialization() {
        let json_data = r#"{
            "from_agent": "general_assistant",
            "to_agent": "risk_analyst",
            "task": "Audit financial API key disclosure",
            "depth": 1,
            "max_depth": 3,
            "constraints": "Strict CEL deny boundary enforced",
            "status": "delegated"
        }"#;

        let res: Result<components::gallery::AgentHandoffCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.from_agent, "general_assistant");
        assert_eq!(card.to_agent, "risk_analyst");
        assert_eq!(card.depth, 1);
        assert_eq!(card.status, "delegated");
    }

    #[test]
    fn test_advisor_note_card_deserialization() {
        let json_data = r#"{
            "reviewer_model": "gpt-5.5-preview",
            "severity": "concern",
            "title": "Unbounded Memory Allocation",
            "message": "Potential memory leak detected in hot loop",
            "suggestion": "Use bounded LRU cache or ring buffer"
        }"#;

        let res: Result<components::gallery::AdvisorNoteCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.reviewer_model, "gpt-5.5-preview");
        assert_eq!(card.severity, "concern");
    }

    #[test]
    fn test_collab_session_card_deserialization() {
        let json_data = r#"{
            "session_id": "sess-404",
            "relay_url": "https://relay.fathom.internal/join#fth1",
            "role": "peer",
            "peer_count": 3,
            "is_active": true
        }"#;

        let res: Result<components::gallery::CollabSessionCardData, _> = serde_json::from_str(json_data);
        assert!(res.is_ok());
        let card = res.unwrap();
        assert_eq!(card.session_id, "sess-404");
        assert_eq!(card.peer_count, 3);
        assert!(card.is_active);
    }
}

