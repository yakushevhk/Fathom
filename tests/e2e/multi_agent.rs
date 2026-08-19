//! End-to-end test: multi-agent fan-out research session.
//!
//! The planner decomposes the query into several sub-tasks which are executed
//! by parallel researcher agents, then synthesized into a final report.

#[path = "../support/mock_llm.rs"]
mod mock_llm;

use std::sync::Arc;

use mock_llm::MockLlm;
use pr_agent::Coordinator;
use pr_core::{AgentEvent, AppConfig, SessionId};
use pr_persistence::Persistence;
use pr_tools::ToolRegistry;
use tokio::sync::broadcast;

fn test_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.agent.max_iterations = 5;
    config
}

#[tokio::test]
async fn test_multi_agent_research() {
    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().to_path_buf();
    let query = "Analyze the impact of LLM agents on software engineering";
    let session_id = SessionId::new();
    let n_agents = 3usize;

    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, query).unwrap();

    let (event_tx, mut event_rx) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query.to_string(),
        Arc::new(MockLlm::multi_agent(n_agents)),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir.clone(),
        test_config(),
    );

    let output = coordinator
        .execute()
        .await
        .expect("multi-agent session should complete");

    // All agents were spawned and completed.
    assert_eq!(output.total_agents, n_agents as u32);
    assert!(output.total_tokens > 0);
    let (total, completed) = db.count_session_agents(&session_id).unwrap();
    assert_eq!(total, n_agents);
    assert_eq!(completed, n_agents);

    // Every agent produced a finding file.
    for i in 1..=n_agents {
        let path = output_dir.join(format!("findings/finding-{i}.md"));
        assert!(path.exists(), "{} should exist", path.display());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.trim().is_empty());
    }

    // Synthesis integrates all sub-tasks.
    assert!(!output.synthesis.trim().is_empty());
    assert!(output.synthesis.contains("Research Report"));

    // Session marked completed.
    let row = db.get_session(&session_id).unwrap().unwrap();
    assert_eq!(row.status, "completed");
    assert_eq!(row.total_agents, n_agents as i64);

    // Lifecycle events were emitted for every agent.
    let mut events = Vec::new();
    while let Ok(e) = event_rx.try_recv() {
        events.push(e);
    }
    let spawned = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AgentSpawned { .. }))
        .count();
    let completed_events = events
        .iter()
        .filter(|e| matches!(e, AgentEvent::AgentCompleted { .. }))
        .count();
    assert_eq!(spawned, n_agents);
    assert_eq!(completed_events, n_agents);
    assert!(events
        .iter()
        .any(|e| matches!(e, AgentEvent::SessionCompleted { .. })));
}

#[tokio::test]
async fn test_multi_agent_respects_max_agents_cap() {
    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().to_path_buf();
    let session_id = SessionId::new();

    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, "capped run").unwrap();

    let mut config = test_config();
    config.agent.max_agents = 2;

    let (event_tx, _) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        "capped run".to_string(),
        // Planner proposes 4 sub-tasks, but only 2 may run.
        Arc::new(MockLlm::multi_agent(4)),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir.clone(),
        config,
    );

    let output = coordinator.execute().await.expect("session should complete");
    assert_eq!(output.total_agents, 2);
    let (total, completed) = db.count_session_agents(&session_id).unwrap();
    assert_eq!(total, 2);
    assert_eq!(completed, 2);
}
