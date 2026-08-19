//! Live-тесты OSINT-тулов на реальном DeepSeek:
//!   A) extract_contacts + enrich_entities (LLM-извлечение персон/компаний)
//!   B) enrich_person — обогащение профиля человека через LLM
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-tools --test llm_live_tools \
//!     -- --ignored --nocapture

use pr_llm::{DeepSeekProvider, LlmProvider};
use pr_tools::{Tool, ToolContext};
use std::path::PathBuf;
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

fn log_sep(t: &str) {
    println!("\n────────────────── {t} ──────────────────");
}

fn ctx_with_llm() -> ToolContext {
    let llm: Arc<dyn LlmProvider> = Arc::new(DeepSeekProvider::new(
        "https://api.deepseek.com",
        &api_key(),
        "deepseek-chat",
    ));
    ToolContext::new(PathBuf::from("/tmp"), pr_core::SearchConfig::default())
        .with_llm(llm)
}

// ── A. extract_contacts + LLM-enrichment ────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn a_extract_contacts_llm_enrichment() {
    log_sep("A. extract_contacts + enrich_entities (LLM)");
    let ctx = ctx_with_llm();
    let tool = pr_tools::ContactExtractor;

    let text = r#"
Contact page of Acme Software:
CEO: Maria Ivanova, email maria.ivanova@acme-soft.ru, phone +7 495 123 45 67
CTO: Ivan Petrov, ivan.petrov@acme-soft.ru
Sales: sales@acme-soft.ru
Acme Software is a Moscow-based CRM vendor with 120 employees.
"#;

    let out = tool
        .execute(
            serde_json::json!({"text": text, "enrich_entities": true}),
            &ctx,
        )
        .await
        .expect("extract_contacts failed");
    println!("success: {}", out.success);
    println!("content: {}", out.content);
    assert!(out.success, "extract должен пройти: {}", out.content);
    // Вывод тула — человекочитаемый отчёт (не JSON).
    assert!(out.content.contains("3 email(s)"), "три email в тексте: {}", out.content);
    assert!(out.content.contains("Maria Ivanova — CEO"), "CEO-персона извлечена LLM");
    assert!(out.content.contains("Ivan Petrov — CTO"), "CTO-персона извлечена LLM");
    assert!(out.content.contains("Acme Software"), "компания извлечена LLM");
    assert!(out.content.contains("industry: CRM vendor"), "атрибуты компании обогащены LLM");
    assert!(out.content.contains("+74951234567"), "телефон нормализован");
    println!("✓ extract: 3 email + 2 персоны + компания с атрибутами — LLM-enrichment работает");
}

// ── B. enrich_person через LLM ──────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn b_enrich_person_llm() {
    log_sep("B. enrich_person — обогащение профиля через LLM");
    let ctx = ctx_with_llm();
    let tool = pr_tools::enrich_person::PersonEnricher;

    let out = tool
        .execute(
            serde_json::json!({
                "name": "Maria Ivanova",
                "company": "Acme Software",
                "role": "CEO"
            }),
            &ctx,
        )
        .await
        .expect("enrich_person failed");
    println!("success: {}", out.success);
    println!("content: {}", head(&out.content, 400));
    assert!(out.success, "enrich должен пройти: {}", out.content);
    // Обогащение возвращает непустой результат (или честное примечание).
    assert!(!out.content.trim().is_empty());
    println!("✓ enrich_person: LLM-ответ получен");
}

fn head(s: &str, n: usize) -> &str {
    let mut end = s.len().min(n);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
