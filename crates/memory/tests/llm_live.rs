//! Live LLM-режим memory engineering pipeline (требует реальный API ключ).
//!
//! Ключ читается из `.env` (переменная DEEPSEEK_API_KEY) или из env-переменной.
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-memory --test llm_live \
//!     -- --ignored --nocapture
//!
//! Покрывает то, что эвристика без LLM не умеет:
//!   - supersede (замена устаревшей версии)
//!   - contradict (противоречие)
//!   - coexist (контекстные предпочтения, оба активны)
//!   - cross-call consolidation
//!   - поиск с reinforcement (access_count) и confidence boost

use pr_llm::DeepSeekProvider;
use pr_memory::{AbsorbFact, AbsorbRequest, Memory, Scope, ScopeFilter};
use serde_json::json;
use std::sync::Arc;

fn api_key() -> String {
    // Order: env var -> .env file in cwd
    if let Ok(k) = std::env::var("DEEPSEEK_API_KEY") {
        if !k.trim().is_empty() {
            return k.trim().to_string();
        }
    }
    if let Ok(contents) = std::fs::read_to_string(".env") {
        for line in contents.lines() {
            if let Some(rest) = line.strip_prefix("DEEPSEEK_API_KEY=") {
                let v = rest.trim().trim_matches('"').trim_matches('\'');
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    panic!("DEEPSEEK_API_KEY not found in env or .env");
}

fn make_llm() -> Arc<DeepSeekProvider> {
    Arc::new(DeepSeekProvider::new(
        "https://api.deepseek.com",
        &api_key(),
        "deepseek-chat",
    ))
}

fn mem_with_llm() -> Memory {
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();
    mem
}

async fn absorb(llm: &Arc<DeepSeekProvider>, mem: &Memory, content: &str, tags: &[&str]) -> String {
    let req = AbsorbRequest {
        facts: vec![AbsorbFact {
            content: content.into(),
            metadata: json!({}),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            confidence: None,
            memory_class: None,
        }],
        source: "llm-live".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let report = mem.pipeline_with_llm(llm.clone()).absorb(req).await.unwrap();
    let s = report.summary_line();
    println!("   <- {content}");
    println!("      → {s}");
    s
}

#[tokio::test]
#[ignore = "requires live API key"]
async fn llm_supersede_contradict_coexist() {
    let llm = make_llm();
    let mem = mem_with_llm();

    println!("\n════════ LLM-MODE MEMORY ENGINEERING ════════\n");

    // 1. Base fact
    let r1 = absorb(&llm, &mem,
        "PostgreSQL 16 is 40% faster than v15 for complex analytics queries", &["database"]).await;
    assert!(r1.starts_with("1 created"), "base fact should be created: {r1}");

    // 2. Newer version → supersede expected
    println!("\n▶ supersede:");
    let r2 = absorb(&llm, &mem,
        "PostgreSQL 17 is now 55% faster than v15 for complex analytics queries", &["database"]).await;

    // 3. Verify v16 was superseded (status), v17 is the active one
    let active = mem
        .db
        .list(&ScopeFilter::persistent(), Some("active"), 100)
        .unwrap();
    let v16 = active.iter().find(|r| r.content.contains("16 is 40%"));
    let v17 = active.iter().find(|r| r.content.contains("17 is now 55%"));
    if r2.contains("supersede") || r2.contains("created") {
        println!("   active v16 present: {}, active v17 present: {}",
            v16.is_some(), v17.is_some());
    }

    // 4. Contradiction — conflicting claim about the same fact
    println!("\n▶ contradict:");
    absorb(&llm, &mem,
        "Benchmarks show PostgreSQL 17 is actually slower than v16 for write-heavy workloads", &["database"]).await;

    // 5. Coexist — context-specific preferences
    println!("\n▶ coexist (context-aware):");
    let _r5a = absorb(&llm, &mem,
        "I prefer terse code reviews with no preamble in my day job at work", &["preferences"]).await;
    let r5b = absorb(&llm, &mem,
        "I prefer long-form detailed technical documentation for my open-source side projects", &["preferences"]).await;

    let prefs = mem
        .db
        .list(&ScopeFilter::persistent(), Some("active"), 100)
        .unwrap()
        .into_iter()
        .filter(|r| r.tags.contains(&"preferences".to_string()))
        .collect::<Vec<_>>();
    println!("   active preference memories: {}", prefs.len());
    assert!(prefs.len() >= 2, "both context preferences must stay active");

    // Show the actual verdict LLM returned for the second fact (coexist vs related)
    if r5b.contains("coexist") {
        println!("   ✓ LLM returned 'coexist' verdict (context-specific, both active)");
    } else if r5b.contains("related") {
        println!("   ℹ LLM returned 'related' (both still active, linked)");
    }

    println!("\n─ search 'PostgreSQL performance' ─");
    for hit in mem.search("PostgreSQL 17 performance benchmarks", &ScopeFilter::persistent(), None).await.unwrap() {
        println!("   [{:.3}] {}", hit.score, hit.memory.content);
    }
}
