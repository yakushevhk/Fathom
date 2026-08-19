//! Integration tests: research pipeline -> export -> notification.
//!
//! Exercises crate interactions: `pr-agent` produces a `SessionOutput`,
//! `pr-core` exports it to files and delivers webhook notifications.

#[path = "../support/mock_llm.rs"]
mod mock_llm;

use std::sync::Arc;

use mock_llm::MockLlm;
use pr_agent::Coordinator;
use pr_core::{
    export::{pandoc_available, ExportFormat, Exporter},
    notify::{NotificationChannel, Notifier},
    AppConfig, SessionId, SessionOutput,
};
use pr_persistence::Persistence;
use pr_tools::ToolRegistry;
use tokio::sync::broadcast;

/// Run a small offline research session and return its output.
async fn run_session(
    output_dir: std::path::PathBuf,
    llm: Arc<dyn pr_llm::LlmProvider>,
) -> (SessionOutput, Arc<Persistence>) {
    let mut config = AppConfig::default();
    config.agent.max_iterations = 5;

    let session_id = SessionId::new();
    let db = Arc::new(Persistence::open(&output_dir.join(".research.db")).unwrap());
    db.create_session(&session_id, "integration test query").unwrap();

    let (event_tx, _) = broadcast::channel(1024);
    let mut coordinator = Coordinator::new(
        session_id,
        "integration test query".to_string(),
        llm,
        Arc::new(ToolRegistry::new()),
        event_tx,
        db.clone(),
        output_dir,
        config,
    );

    let output = coordinator.execute().await.expect("session should complete");
    (output, db)
}

#[tokio::test]
async fn test_research_then_export_html_and_json() {
    let tmp = tempfile::tempdir().unwrap();
    let (output, _db) = run_session(
        tmp.path().to_path_buf(),
        Arc::new(MockLlm::multi_agent(2)),
    )
    .await;

    let exporter = Exporter::new(tmp.path().to_path_buf());

    // HTML export
    let html_path = exporter.export(&output, ExportFormat::Html).await.unwrap();
    assert_eq!(html_path, tmp.path().join("report.html"));
    let html = std::fs::read_to_string(&html_path).unwrap();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("Research Report"));
    assert!(html.contains(&output.session_id.0));
    // Findings appendix is embedded.
    assert!(html.contains("Appendix: Individual Findings"));

    // JSON export
    let json_path = exporter.export(&output, ExportFormat::Json).await.unwrap();
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
    assert_eq!(value["session_id"], output.session_id.0);
    assert_eq!(value["total_agents"], 2);
    assert_eq!(value["findings"].as_array().unwrap().len(), 2);
    assert!(value["synthesis"]["markdown"]
        .as_str()
        .unwrap()
        .contains("Research Report"));
    // Sources extracted from the mock researchers' answers.
    assert!(value["findings"][0]["sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "https://example.org/reference"));
}

#[tokio::test]
async fn test_research_then_export_pdf_docx_via_pandoc() {
    if !pandoc_available().await {
        eprintln!("pandoc not installed; skipping PDF/DOCX export integration test");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let (output, _db) = run_session(
        tmp.path().to_path_buf(),
        Arc::new(MockLlm::single_agent()),
    )
    .await;

    let exporter = Exporter::new(tmp.path().to_path_buf());

    // DOCX never needs an external PDF engine, so it must succeed.
    let docx_path = exporter.export(&output, ExportFormat::Docx).await.unwrap();
    assert!(docx_path.exists());
    assert!(std::fs::metadata(&docx_path).unwrap().len() > 0);

    // PDF additionally needs a PDF engine (pdflatex/weasyprint/...); tolerate
    // environments that have pandoc but no engine.
    match exporter.export(&output, ExportFormat::Pdf).await {
        Ok(pdf_path) => {
            assert!(pdf_path.exists());
            assert!(std::fs::metadata(&pdf_path).unwrap().len() > 0);
        }
        Err(e) => {
            eprintln!("PDF export unavailable in this environment: {e}");
        }
    }
}

#[tokio::test]
async fn test_research_then_notify_webhook() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // One-shot local HTTP server capturing a single POST request.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let mut header_end = None;
        let mut content_length = None;
        loop {
            let n = socket.read(&mut chunk).await.unwrap();
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if header_end.is_none() {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos);
                    let headers = String::from_utf8_lossy(&buf[..pos]).to_lowercase();
                    content_length = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok());
                }
            }
            if let (Some(pos), Some(len)) = (header_end, content_length) {
                if buf.len() >= pos + 4 + len {
                    break;
                }
            }
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        String::from_utf8_lossy(&buf).to_string()
    });

    let tmp = tempfile::tempdir().unwrap();
    let (output, _db) = run_session(
        tmp.path().to_path_buf(),
        Arc::new(MockLlm::multi_agent(2)),
    )
    .await;

    let notifier = Notifier::new(vec![NotificationChannel::Webhook {
        url: format!("http://{addr}/research-hook"),
    }]);
    notifier
        .notify_completion(&output)
        .await
        .expect("webhook delivery should succeed");

    let request = server.await.unwrap();
    assert!(request.starts_with("POST /research-hook"));
    assert!(request.contains("session.completed"));
    assert!(request.contains(&output.session_id.0));
}
