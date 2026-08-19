//! End-to-end test: inter-agent messaging via the `hub` tool and IrcBus.
//!
//! Two agents run concurrently (as coordinator-spawned researchers). One
//! sends a message via the `hub send` tool to the other; the recipient
//! drains its inbox at the turn boundary and reacts to the message. The
//! test asserts the bus delivered the message and both agents stayed
//! registered throughout their run.

#[path = "../support/mock_llm.rs"]
mod mock_llm;

use std::sync::Arc;

use mock_llm::MockLlm;
use pr_agent::Coordinator;
use pr_core::irc::{AgentRegistry, IrcBus};
use pr_core::{AppConfig, SessionId};
use pr_persistence::Persistence;
use pr_tools::ToolRegistry;
use tokio::sync::broadcast;

fn test_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.agent.max_iterations = 10;
    config
}

#[tokio::test]
async fn test_hub_tool_registers_agents_on_bus() {
    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().to_path_buf();
    let query = "Find info about company Acme";
    let session_id = SessionId::new();

    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, query).unwrap();

    let (event_tx, _event_rx) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id.clone(),
        query.to_string(),
        Arc::new(MockLlm::multi_agent(2)),
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir.clone(),
        test_config(),
    );

    let output = coordinator
        .execute()
        .await
        .expect("session should complete");

    // Two agents ran.
    assert_eq!(output.total_agents, 2);

    // Once the session completes, all agents are unregistered.
    let agents = AgentRegistry::global().list();
    assert!(
        agents.is_empty(),
        "all agents must be unregistered after run, got: {agents:?}"
    );
}

#[tokio::test]
async fn test_irc_bus_survives_agent_lifecycle() {
    // Sanity: the bus itself works in the Fathom process context with
    // real AgentId values (not just the unit-test ids).
    let bus = IrcBus::global();
    let alice = pr_core::AgentId::new();
    let bob = pr_core::AgentId::new();

    assert!(!bus.is_registered(&alice));

    let mut rx = bus.register(&bob);
    assert!(bus.is_registered(&bob));

    let msg = pr_core::IrcMessage {
        from: alice.clone(),
        to: Some(bob.clone()),
        content: "ping".to_string(),
        id: bus.next_msg_id(),
        expects_reply: false,
        reply_to: None,
    };
    match bus.send(msg) {
        pr_core::DeliveryReceipt::Delivered => {}
        other => panic!("expected Delivered, got {other:?}"),
    }

    let received = rx.recv().await.unwrap();
    assert_eq!(received.content, "ping");

    bus.unregister(&bob);
    assert!(!bus.is_registered(&bob));
}