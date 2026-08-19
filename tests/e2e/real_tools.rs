//! E2E over the REAL tool stack (fleet backlog #14).
//!
//! Previous integration tests ran the coordinator against an empty tool
//! registry. This suite drives the actual tools end-to-end against a local
//! mock HTTP site:
//!
//! ```text
//! mock site (tokio TCP) ──> web_fetch / extract_contacts(url)
//!     ──> deterministic autosave into ContactDb
//!     ──> semantic memory absorb (agent scope)
//! ```
//!
//! The loopback SSRF block is lifted for this process via
//! `PR_SSRF_ALLOW_LOOPBACK=1` (test-only escape hatch in `guard.rs`).

use async_trait::async_trait;
use pr_agent::AgentRuntime;
use pr_core::{AgentId, AgentRole, AppConfig, Message, PrResult, SessionId, ToolCall};
use pr_llm::{CompletionRequest, CompletionResponse, LlmProvider, StreamChunk, Usage};
use pr_memory::Memory;
use pr_persistence::{ContactDb, ContactStore, Persistence};
use pr_tools::ToolRegistry;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;

const TEAM_PAGE: &str = r#"<!DOCTYPE html>
<html>
<head><title>Acme LLC — Team</title></head>
<body>
<h1>Our Team</h1>
<div class="person">
  <h2>Ivan Petrov</h2>
  <p>CTO</p>
  <a href="mailto:ivan.petrov@acme-e2e.example">ivan.petrov@acme-e2e.example</a>
  <span>+7 916 123-45-67</span>
</div>
<div class="person">
  <h2>Maria Sidorova</h2>
  <p>CEO</p>
  <a href="mailto:maria.sidorova@acme-e2e.example">maria.sidorova@acme-e2e.example</a>
</div>
<p>General inbox: info@acme-e2e.example</p>
</body>
</html>"#;

/// Minimal HTTP/1.1 server: any request gets the team page.
async fn spawn_mock_site() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock site");
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let _ = sock.read(&mut buf).await; // consume the request
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    TEAM_PAGE.len(),
                    TEAM_PAGE
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (format!("http://{addr}/team"), handle)
}

/// Scripted LLM: first completion issues `extract_contacts` against the mock
/// site, second completion ends the run.
struct ScriptedLlm {
    url: String,
    calls: std::sync::Mutex<u32>,
}

#[async_trait]
impl LlmProvider for ScriptedLlm {
    fn name(&self) -> &str {
        "scripted"
    }
    fn model(&self) -> &str {
        "scripted-model"
    }
    async fn complete(&self, _req: &CompletionRequest) -> PrResult<CompletionResponse> {
        let mut calls = self.calls.lock().unwrap();
        *calls += 1;
        let resp = if *calls == 1 {
            CompletionResponse {
                message: Message::assistant_with_tools(
                    Some("Extracting contacts from the team page.".to_string()),
                    vec![ToolCall::new(
                        "call_extract",
                        "extract_contacts",
                        serde_json::json!({ "url": self.url }),
                    )],
                ),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 10,
                    total_tokens: 20,
                }),
                finish_reason: Some("tool_calls".into()),
            }
        } else {
            CompletionResponse {
                message: Message::assistant("Harvesting finished."),
                usage: Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                }),
                finish_reason: Some("stop".into()),
            }
        };
        Ok(resp)
    }
    async fn stream(
        &self,
        _req: &CompletionRequest,
    ) -> PrResult<Box<dyn futures::Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        // Force the runtime's non-streaming fallback path.
        Err(pr_core::PrError::Llm("stream disabled in e2e".into()))
    }
}

fn enable_loopback_for_tests() {
    std::env::set_var("PR_SSRF_ALLOW_LOOPBACK", "1");
}

#[tokio::test]
async fn web_fetch_reads_local_mock_site() {
    enable_loopback_for_tests();
    let (url, server) = spawn_mock_site().await;

    let registry = ToolRegistry::with_builtins();
    let ctx = pr_tools::ToolContext::new(
        std::env::temp_dir(),
        pr_core::SearchConfig::default(),
    );
    let out = registry
        .execute("web_fetch", serde_json::json!({ "url": url }), &ctx)
        .await
        .expect("web_fetch executes");
    assert!(out.success, "web_fetch failed: {}", out.content);
    assert!(out.content.contains("Our Team"), "page text missing: {}", out.content);
    assert!(out.content.contains("Ivan Petrov"));

    server.abort();
}

#[tokio::test]
async fn full_pipeline_extract_save_memory() {
    enable_loopback_for_tests();
    let (url, server) = spawn_mock_site().await;

    // Real subsystems, all in-memory.
    let contact_db: Arc<dyn ContactStore> = Arc::new(ContactDb::in_memory().unwrap());
    let memory = Arc::new(Memory::in_memory(AppConfig::default().memory).unwrap());
    let session_db = Arc::new(Persistence::in_memory().unwrap());
    let session_id = SessionId::new();
    session_db.create_session(&session_id, "harvest acme team").unwrap();

    let llm = Arc::new(ScriptedLlm {
        url: url.clone(),
        calls: std::sync::Mutex::new(0),
    });

    let (event_tx, _event_rx) = broadcast::channel(128);
    let agent_id = AgentId::new();
    // The runtime expects its agent row to exist (the coordinator normally
    // creates it before spawning).
    session_db
        .create_agent(&pr_core::AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: format!("Harvest contacts from {url}"),
            status: pr_core::AgentStatus::Spawned,
            depth: 0,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        })
        .unwrap();
    let mut agent = AgentRuntime::new(
        agent_id,
        session_id,
        None,
        AgentRole::Researcher,
        format!("Harvest contacts from {url}"),
        0,
        llm,
        Arc::new(ToolRegistry::with_builtins()),
        event_tx,
        session_db,
        std::env::temp_dir(),
        AppConfig::default(),
    );
    agent.contact_db = Some(contact_db.clone());
    agent.memory = Some(memory.clone());

    let output = agent.run().await.expect("agent run");
    assert_eq!(output.summary, "Harvesting finished.");

    // ── Contacts reached the database via deterministic autosave ──
    let stored = contact_db.list_all(100, 0).await.expect("list contacts");
    assert!(
        stored.len() >= 2,
        "expected at least 2 saved contacts, got {}: {:?}",
        stored.len(),
        stored.iter().map(|c| c.email.clone()).collect::<Vec<_>>()
    );
    assert!(stored.iter().any(|c| c.email.as_deref() == Some("ivan.petrov@acme-e2e.example")));
    assert!(stored.iter().any(|c| c.email.as_deref() == Some("maria.sidorova@acme-e2e.example")));

    // ── Semantic memory absorbed the harvested contacts ──
    let memories = memory
        .db
        .list(&pr_memory::ScopeFilter::persistent(), Some("active"), 100)
        .unwrap();
    assert!(
        !memories.is_empty(),
        "autosave must absorb contacts into long-term memory"
    );
    assert!(
        memories.iter().any(|m| m.content.contains("acme-e2e.example")),
        "memory facts should mention the harvested emails: {:?}",
        memories.iter().map(|m| m.content.clone()).collect::<Vec<_>>()
    );
    // Hybrid search finds them.
    let hits = memory
        .search("ivan petrov email acme", &pr_memory::ScopeFilter::persistent(), Some(5))
        .await
        .unwrap();
    assert!(!hits.is_empty(), "search must find absorbed contacts");

    server.abort();
}
