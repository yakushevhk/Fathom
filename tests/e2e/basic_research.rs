//! End-to-end test: a basic single-agent research session.
//!
//! Runs the full coordinator pipeline (plan -> research -> synthesize ->
//! write output -> persist) against a deterministic mock LLM, so no network
//! access or API key is required.

#[path = "../support/mock_llm.rs"]
mod mock_llm;

use std::sync::Arc;

use mock_llm::MockLlm;
use pr_agent::Coordinator;
use pr_core::{AppConfig, SessionId};
use pr_persistence::Persistence;
use pr_tools::ToolRegistry;
use tokio::sync::broadcast;

fn test_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.agent.max_iterations = 5;
    config
}

#[tokio::test]
async fn test_basic_research() {
    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().to_path_buf();
    let query = "What are the benefits of Rust for CLI tooling?";
    let session_id = SessionId::new();

    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, query).unwrap();

    let (event_tx, _event_rx) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query.to_string(),
        Arc::new(MockLlm::single_agent()),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir.clone(),
        test_config(),
    );

    let output = coordinator
        .execute()
        .await
        .expect("research session should complete");

    // Summary is non-empty and is the synthesized report.
    assert!(!output.synthesis.trim().is_empty());
    assert!(output.synthesis.contains("Research Report"));
    assert_eq!(output.session_id, session_id);
    assert_eq!(output.total_agents, 1);
    assert!(output.total_tokens > 0);

    // Output files are created.
    for file in ["summary.md", "index.md", "sources.md"] {
        assert!(output_dir.join(file).exists(), "{file} should exist");
    }
    assert!(output_dir.join("findings/finding-1.md").exists());

    let summary = std::fs::read_to_string(output_dir.join("summary.md")).unwrap();
    assert!(!summary.trim().is_empty());
    assert_eq!(summary, output.synthesis);

    let finding = std::fs::read_to_string(output_dir.join("findings/finding-1.md")).unwrap();
    assert!(finding.contains("example.com"));

    // Session marked completed in the database.
    let row = db.get_session(&session_id).unwrap().unwrap();
    assert_eq!(row.status, "completed");
    assert_eq!(row.total_agents, 1);
    assert!(row.total_tokens > 0);
}

/// Live-API variant of the basic E2E test. Disabled by default because it
/// costs tokens and needs network access. Run with:
///   cargo test --test e2e_basic_research -- --ignored
#[tokio::test]
#[ignore]
async fn test_basic_research_live_api() {
    let config = AppConfig::load().expect("config should load");
    if config.llm.api_key.is_empty() {
        eprintln!("skipping live E2E test: no API key configured");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().to_path_buf();
    let session_id = SessionId::new();
    let query = "Summarize the current state of the Rust async ecosystem in two paragraphs.";

    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, query).unwrap();

    let llm: Arc<dyn pr_llm::LlmProvider> = Arc::new(pr_llm::DeepSeekProvider::new(
        &config.llm.base_url,
        &config.llm.api_key,
        &config.llm.model,
    ));

    let (event_tx, _) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query.to_string(),
        llm,
        Arc::new(ToolRegistry::with_builtins()),
        event_tx,
        db.clone(),
        output_dir.clone(),
        config,
    );

    let output = coordinator
        .execute()
        .await
        .expect("live research session should complete");

    assert!(!output.synthesis.trim().is_empty());
    assert!(output_dir.join("summary.md").exists());
}
