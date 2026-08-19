//! Integration tests: persistence + session history over real research runs.
//!
//! Two sessions are executed through the coordinator and then inspected via
//! `SessionHistory` (list / search / details).

#[path = "../support/mock_llm.rs"]
mod mock_llm;

use std::sync::Arc;

use mock_llm::MockLlm;
use pr_agent::Coordinator;
use pr_core::{AppConfig, SessionId};
use pr_persistence::{Persistence, SessionHistory};
use pr_tools::ToolRegistry;
use tokio::sync::broadcast;

async fn run_session(db: Arc<Persistence>, output_dir: std::path::PathBuf, query: &str, tasks: usize) -> SessionId {
    let mut config = AppConfig::default();
    config.agent.max_iterations = 5;

    let session_id = SessionId::new();
    db.create_session(&session_id, query).unwrap();

    let (event_tx, _) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query.to_string(),
        Arc::new(MockLlm::multi_agent(tasks)),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db,
        output_dir,
        config,
    );

    coordinator.execute().await.expect("session should complete");
    session_id
}

#[tokio::test]
async fn test_history_over_completed_sessions() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Arc::new(Persistence::open(&tmp.path().join(".research.db")).unwrap());

    let s1 = run_session(
        db.clone(),
        tmp.path().join("run-1"),
        "Research quantum computing trends",
        2,
    )
    .await;
    let s2 = run_session(
        db.clone(),
        tmp.path().join("run-2"),
        "Analyze coffee market prices",
        1,
    )
    .await;

    let history = SessionHistory::new(db.clone());

    // Listing: newest first, both sessions completed.
    let sessions = history.list_sessions(10);
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, s2);
    assert_eq!(sessions[1].id, s1);
    assert!(sessions.iter().all(|s| s.status == "completed"));

    // Limiting works.
    assert_eq!(history.list_sessions(1).len(), 1);

    // Search by substring of the query.
    let quantum = history.search_sessions("quantum");
    assert_eq!(quantum.len(), 1);
    assert_eq!(quantum[0].id, s1);
    assert!(history.search_sessions("nonexistent-topic").is_empty());

    // Details include agents and per-agent summaries.
    let details = history.get_session_details(&s1).expect("session exists");
    assert_eq!(details.session.id, s1);
    assert_eq!(details.session.status, "completed");
    assert_eq!(details.agents.len(), 2);
    assert!(details.agents.iter().all(|a| a.status == "completed"));
    assert!(details.agents.iter().all(|a| a.summary.is_some()));
    assert!(details.agents.iter().all(|a| a.tokens_used > 0));
    assert_eq!(details.session.total_agents, 2);

    let details2 = history.get_session_details(&s2).expect("session exists");
    assert_eq!(details2.agents.len(), 1);

    // Unknown session returns None.
    assert!(history
        .get_session_details(&SessionId("missing".to_string()))
        .is_none());
}
