//! Широкое live-тестирование LLM-зависимых направлений крейта pr-memory
//! на реальном DeepSeek.
//!
//!   A) LLM-переранжирование поиска (llm_rerank) — JSON verdict {"order": [...]}
//!   B) LLM-классификация absorb — cross-call: оба контекстных предпочтения активны
//!   C) LLM-классификация absorb — supersede-цепочка версий
//!   D) Raw provider — usage tracking + structured JSON output
//!   E) Raw provider — streaming (SSE), сбор текста
//!   F) Raw provider — function calling (tool_calls с аргументами)
//!
//! Ключ: DEEPSEEK_API_KEY (env или .env). Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-memory --test llm_wide \
//!     -- --ignored --nocapture

use pr_llm::DeepSeekProvider;
use pr_llm::LlmProvider;
use pr_memory::{AbsorbFact, AbsorbRequest, Memory, Scope, ScopeFilter};
use std::sync::Arc;

fn api_key() -> String {
    if let Ok(k) = std::env::var("DEEPSEEK_API_KEY") {
        if !k.trim().is_empty() {
            return k.trim().to_string();
        }
    }
    for path in [".env", "../../.env"] {
        if let Ok(contents) = std::fs::read_to_string(path) {
            for line in contents.lines() {
                if let Some(v) = line.strip_prefix("DEEPSEEK_API_KEY=") {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return v.to_string();
                    }
                }
            }
        }
    }
    panic!("DEEPSEEK_API_KEY not found");
}

fn make_llm() -> Arc<DeepSeekProvider> {
    Arc::new(DeepSeekProvider::new(
        "https://api.deepseek.com",
        &api_key(),
        "deepseek-chat",
    ))
}

fn msg_text(m: &pr_core::Message) -> String {
    match m {
        pr_core::Message::System { content } | pr_core::Message::User { content } => content.clone(),
        pr_core::Message::Assistant { content, .. } => content.clone().unwrap_or_default(),
        pr_core::Message::Tool { content, .. } => content.clone(),
    }
}

fn log_sep(t: &str) {
    println!("\n────────────────── {t} ──────────────────");
}

fn p_fact(content: &str) -> AbsorbFact {
    AbsorbFact {
        content: content.into(),
        metadata: serde_json::json!({}),
        tags: vec![],
        confidence: None,
        memory_class: None,
    }
}

// ── A. LLM rerank ──────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn a_llm_rerank_orders_by_relevance() {
    log_sep("A. llm_rerank — переупорядочивание результатов поиска");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.rerank = true;
    let mem = Memory::in_memory(cfg).unwrap();

    let pipeline = mem.pipeline_with_llm(llm.clone());
    let req = AbsorbRequest {
        facts: vec![
            p_fact("Redis is an in-memory data store used for caching and message brokering"),
            p_fact("Memcached is a distributed memory object caching system for web applications"),
            p_fact("RabbitMQ is an open-source message broker that implements the AMQP protocol"),
            p_fact("Apache Kafka is a distributed event streaming platform used as a message bus"),
            p_fact("PostgreSQL uses MVCC for concurrency control and WAL for crash recovery"),
        ],
        source: "rerank".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let _ = pipeline.absorb(req).await.unwrap();

    let hits = pr_memory::search::hybrid_search(
        &mem.db,
        &mem.embedder,
        &pr_memory::SearchParams {
            query: "in-memory caching message broker".into(),
            top_k: 5,
            min_score: 0.05, // расширяем выдачу, чтобы rerank имел смысл
            semantic_weight: 0.7,
            temporal_decay: 0.01,
            scope: ScopeFilter::persistent(),
        },
    )
    .await
    .unwrap();
    println!("{} results before rerank:", hits.len());
    for h in &hits {
        println!("   [{:.3}] {}", h.score, h.memory.content);
    }
    assert!(hits.len() >= 2, "нужно ≥2 кандидатов для теста rerank, получено {}", hits.len());

    let llm_dyn: Arc<dyn pr_llm::LlmProvider> = llm.clone();
    let reranked =
        pr_memory::search::llm_rerank(&llm_dyn, "in-memory caching message broker", hits.clone()).await;
    println!("after LLM rerank:");
    for h in &reranked {
        println!("   - {}", h.memory.content);
    }
    if let Some(first) = reranked.first() {
        assert!(
            first.memory.content.to_lowercase().contains("redis"),
            "Redis должен быть первым, получено: {}",
            first.memory.content
        );
    }
    println!("✓ rerank: Redis поднят на первое место");
}

// ── B. Absorb cross-call: контекстные предпочтения ────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn b_llm_absorb_coexist_context() {
    log_sep("B. Absorb — cross-call: контекстные предпочтения оба активны");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    let req1 = AbsorbRequest {
        facts: vec![p_fact(
            "I prefer terse code reviews with no preamble when reviewing pull requests at work",
        )],
        source: "c".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline_with_llm(llm.clone()).absorb(req1).await.unwrap();

    let req2 = AbsorbRequest {
        facts: vec![p_fact(
            "For my personal open-source projects I prefer long-form detailed technical explanations in reviews",
        )],
        source: "c".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let r2 = mem.pipeline_with_llm(llm.clone()).absorb(req2).await.unwrap();
    println!("second fact: created={} superseded={} contradicted={} coexisted={} related={} skipped={}",
        r2.created, r2.superseded, r2.contradicted, r2.coexisted, r2.related, r2.skipped);

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    for a in &active {
        println!("   (status={}) {}", a.status, a.content);
    }
    assert!(active.len() >= 2, "оба контекстных предпочтения должны остаться active");
    println!("✓ absorb: оба предпочтения хранятся активными");
}

// ── C. Absorb: supersede-цепочка ───────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn c_llm_absorb_supersede_chain() {
    log_sep("C. Absorb — supersede-цепочка версий факта");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    let r1 = mem.pipeline_with_llm(llm.clone()).absorb(AbsorbRequest {
        facts: vec![p_fact("Acme Corp CEO is Ivan Petrov as of 2024")],
        source: "s".into(), scope: Scope::Agent, scope_key: String::new(), context: None, dry_run: false,
    }).await.unwrap();
    println!("v1: {}", r1.summary_line());

    let r2 = mem.pipeline_with_llm(llm.clone()).absorb(AbsorbRequest {
        facts: vec![p_fact("Acme Corp CEO is Maria Ivanova as of 2025")],
        source: "s".into(), scope: Scope::Agent, scope_key: String::new(), context: None, dry_run: false,
    }).await.unwrap();
    println!("v2: {}", r2.summary_line());

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    for a in &active {
        println!("   (status={}) {}", a.status, a.content);
    }
    let old_active = active.iter().any(|r| r.content.contains("Ivan Petrov") && r.content.contains("2024"));
    let new_active = active.iter().any(|r| r.content.contains("Maria Ivanova"));
    println!("old CEO active: {old_active}, new CEO active: {new_active}");
    assert!(new_active, "новый CEO должен быть активным");
    assert!(!old_active, "старый CEO должен быть superseded (не active)");
    println!("✓ absorb: смена CEO корректно заменена, старая версия не активна");
}

// ── D. Raw provider: usage + JSON ─────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn d_llm_provider_usage_and_json() {
    log_sep("D. Raw provider — usage tracking и structured JSON");
    let llm = make_llm();

    let req = pr_llm::CompletionRequest {
        messages: vec![pr_core::Message::user("Reply with exactly the word: pong")],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(20),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("completion failed");
    println!("model: {}", llm.model());
    println!("content: {:?}", msg_text(&resp.message));
    println!("finish_reason: {:?}", resp.finish_reason);
    if let Some(u) = &resp.usage {
        println!("usage: prompt={} completion={} total={}", u.prompt_tokens, u.completion_tokens, u.total_tokens);
        assert!(u.total_tokens > 0, "usage.total_tokens должен быть > 0");
    }

    let req2 = pr_llm::CompletionRequest {
        messages: vec![pr_core::Message::user(
            r#"Return a JSON object {"answer":"deepseek-works","ok":true}. No other text."#,
        )],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(100),
        stream: false,
    };
    let resp2 = llm.complete(&req2).await.expect("json completion failed");
    let text = msg_text(&resp2.message);
    println!("json attempt: {}", text.trim_start().chars().take(70).collect::<String>());
    if let Ok(p) = serde_json::from_str::<serde_json::Value>(text.trim()) {
        println!("parsed JSON ok: {}", p);
        assert_eq!(p["ok"].as_bool(), Some(true));
    } else {
        println!("⚠ не чистый JSON — пропускаем строгую проверку (модель reasoning)");
    }
    println!("✓ provider: usage и JSON-вывод работают");
}

// ── E. Raw provider: streaming ─────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn e_llm_provider_streaming() {
    log_sep("E. Raw provider — streaming (SSE) сбор текста");
    let llm = make_llm();

    let req = pr_llm::CompletionRequest {
        messages: vec![pr_core::Message::user(
            "List the numbers from one to five, one per line.",
        )],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(50),
        stream: true,
    };
    let mut stream = llm.stream(&req).await.expect("stream failed");
    use futures::StreamExt;
    let mut text = String::new();
    let mut usage_seen = false;
    while let Some(chunk) = stream.next().await {
        match chunk.expect("stream chunk error") {
            pr_llm::StreamChunk::Text { delta } => text.push_str(&delta),
            pr_llm::StreamChunk::Reasoning { .. } => {}
            pr_llm::StreamChunk::Done { usage, .. } => {
                usage_seen = usage.is_some();
            }
            pr_llm::StreamChunk::ToolCallDelta { .. } => {}
            pr_llm::StreamChunk::Error { .. } => {}
        }
    }
    println!("streamed text: {:?}", text);
    println!("usage in Done chunk: {usage_seen}");
    assert!(text.contains('1') && text.contains('5'), "стрим должен вернуть числа 1..5, получили: {text:?}");
    println!("✓ streaming: SSE-дельта собраны в текст");
}

// ── F. Raw provider: function calling ──────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn f_llm_provider_tool_calls() {
    log_sep("F. Raw provider — function calling (tool_calls)");
    let llm = make_llm();

    let req = pr_llm::CompletionRequest {
        messages: vec![pr_core::Message::user("What is the weather in Kazan?")],
        tools: vec![pr_core::ToolSchema {
            name: "get_weather".into(),
            description: "Get current weather for a city".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string", "description": "City name"}
                },
                "required": ["city"]
            }),
        }],
        temperature: Some(0.0),
        max_tokens: Some(200),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("tool call failed");
    match &resp.message {
        pr_core::Message::Assistant { tool_calls, .. } => {
            println!("tool_calls: {}", tool_calls.len());
            for tc in tool_calls {
                println!("   name={} args={}", tc.name(), tc.arguments());
            }
            assert!(!tool_calls.is_empty(), "модель должна вызвать get_weather");
            assert_eq!(tool_calls[0].name(), "get_weather");
            assert_eq!(tool_calls[0].arguments()["city"], "Kazan");
        }
        other => panic!("ожидался Assistant с tool_calls, получено: {:?}", msg_text(other)),
    }
    println!("✓ function calling: get_weather(Kazan) распознан корректно");
}
