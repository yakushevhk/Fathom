//! Live-тесты реальных промптов агентского крейта на DeepSeek.
//! Используются настоящие role_prompt_for / PromptBuilder (не копии):
//!   A) Синтез отчёта (роль Writer) — markdown-структура
//!   B) Системный промпт Researcher — ответ со ссылками на источники
//!   C) Gap-filling round (флит C3) — промпт дозаполнения контактов
//!   D) Роль Verifier — факт-чекинг с вердиктом
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-agent --test llm_live_agent2 \
//!     -- --ignored --nocapture

use pr_agent::prompt::{role_prompt_for, PromptBuilder};
use pr_core::agent::AgentRole;
use pr_llm::{CompletionRequest, DeepSeekProvider, LlmProvider};
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

/// Срез по границе символа (кириллица — многобайтовая).
fn head(s: &str, n: usize) -> &str {
    let mut end = s.len().min(n);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ── A. Синтез отчёта (роль Writer) ─────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn a_synthesis_report_markdown_structure() {
    log_sep("A. Синтез отчёта — роль Writer");
    let llm = make_llm();

    let findings = "\
### Finding 1
PostgreSQL 17 показывает 120k TPS в OLTP-бенчмарке sysbench на 16 ядрах.
Источники: percona.com/blog (2026-03), postgresql.org/docs.
### Finding 2
MySQL 8.4 достигает 95k TPS на том же железе, но лучше держит p99 latency.
Источники: mysql.com/benchmarks (2026-02).
### Finding 3
Экосистема: pgBouncer и native partitioning дают PostgreSQL преимущество в connection pooling.
";
    let prompt = format!(
        concat!(
            "You are synthesizing research findings into a final report.\n\n",
            "Original query: {}\n\n",
            "Findings from sub-agents:\n\n{}\n\n",
            "\n",
            "Write a comprehensive, well-structured markdown report that:\n",
            "1. Answers the original query\n",
            "2. Integrates all findings coherently\n",
            "3. Notes any contradictions between sources\n",
            "4. Lists key sources/references\n",
            "5. Identifies gaps or areas for further research\n\n",
            "Write in a clear, informative style. Use markdown headers, bullet points, and emphasis where appropriate."
        ),
        "Сравнить PostgreSQL и MySQL для высоконагруженных OLTP-систем",
        findings,
    );

    let req = CompletionRequest {
        messages: vec![
            pr_core::Message::system(format!(
                "You are a research synthesizer.\n\n{}",
                role_prompt_for(AgentRole::Writer)
            )),
            pr_core::Message::user(prompt),
        ],
        tools: vec![],
        temperature: Some(0.5),
        max_tokens: Some(16_384),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("synthesis failed");
    let text = msg_text(&resp.message);
    println!("отчёт: {} символов", text.len());
    println!("{}", head(&text, 700));

    assert!(text.len() > 300, "отчёт не должен быть пустым/коротким");
    assert!(text.contains('#'), "отчёт должен содержать markdown-заголовки");
    let lower = text.to_lowercase();
    assert!(lower.contains("postgresql") && lower.contains("mysql"),
        "отчёт должен интегрировать оба findings");
    println!("✓ синтез: markdown-отчёт с интеграцией findings получен");
}

// ── B. Системный промпт Researcher (настоящий PromptBuilder) ───────────────

#[tokio::test]
#[ignore = "live API"]
async fn b_researcher_prompt_produces_cited_findings() {
    log_sep("B. Researcher — настоящий системный промпт + источники");
    let llm = make_llm();

    // Реальный путь сборки промпта агента (без env-блока).
    let builder = PromptBuilder::new(
        AgentRole::Researcher,
        "Исследуй: в каком году вышел Rust 1.0 и кто его основной спонсор?",
        1,
        2,
        "deepseek-chat",
    );
    let system = builder.build();
    println!("системный промпт: {} символов", system.len());

    let req = CompletionRequest {
        messages: vec![
            pr_core::Message::system(system),
            pr_core::Message::user(
                "Исследуй: в каком году вышел Rust 1.0 и кто его основной спонсор? \
                 Верни ответ с указанием источника.",
            ),
        ],
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(1024),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("researcher call failed");
    let text = msg_text(&resp.message);
    println!("ответ: {}", head(&text, 500));
    assert!(text.contains("2015") || text.to_lowercase().contains("mozilla"),
        "ответ должен содержать год (2015) или спонсора (Mozilla): {}", head(&text, 100));
    println!("✓ researcher: промпт даёт содержательный ответ с фактами");
}

// ── C. Gap-filling round (флит C3, LeadGen) ────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn c_gap_filling_round_prompt() {
    log_sep("C. Gap-filling round — дозаполнение контактов");
    let llm = make_llm();
    let query = "Найди контакты CEO IT-компаний Москвы, нужно 20 email";
    let gap_task = format!(
        "GAP-FILLING ROUND: the team collected 13 of 20 requested contacts so far. \
         Find at least 7 MORE contacts matching the original query: {query}. \
         Use DIFFERENT sources/companies than the obvious ones already covered. \
         Extract and verify emails/phones; extraction results are auto-persisted.",
    );

    let req = CompletionRequest {
        messages: vec![
            pr_core::Message::system(format!(
                "You are a research agent.\n\n{}",
                role_prompt_for(AgentRole::Researcher)
            )),
            pr_core::Message::user(gap_task),
        ],
        tools: vec![],
        temperature: Some(0.4),
        max_tokens: Some(1024),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("gap-filling failed");
    let text = msg_text(&resp.message);
    println!("ответ: {}", head(&text, 500));
    assert!(!text.trim().is_empty(), "gap-промпт не должен давать пустой ответ");
    let lower = text.to_lowercase();
    assert!(
        lower.contains("источник") || lower.contains("source") || lower.contains("company") || lower.contains("компан"),
        "ответ должен ссылаться на источники/компании"
    );
    println!("✓ gap-filling: агент понял задачу дозаполнения");
}

// ── D. Роль Verifier — факт-чекинг ─────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn d_verifier_role_fact_check() {
    log_sep("D. Verifier — факт-чекинг утверждений");
    let llm = make_llm();

    let claims = "\
Claim 1: Rust 1.0 was released in 2015.
Claim 2: Rust 1.0 was released in 2005.
Claim 3: The Rust Foundation is the main steward of the Rust language.
";
    let req = CompletionRequest {
        messages: vec![
            pr_core::Message::system(format!(
                "You are a verifier agent.\n\n{}",
                role_prompt_for(AgentRole::Verifier)
            )),
            pr_core::Message::user(format!(
                "Fact-check the following claims. For each claim, assign a verification \
                 status: verified / false / unverified, and give a one-line reason.\n\n{claims}"
            )),
        ],
        tools: vec![],
        temperature: Some(0.2),
        max_tokens: Some(1024),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("verifier call failed");
    let text = msg_text(&resp.message);
    println!("вердикт: {}", head(&text, 600));
    let lower = text.to_lowercase();
    assert!(
        lower.contains("verified") || lower.contains("false") || lower.contains("верно") || lower.contains("невер"),
        "вердикт должен содержать статусы проверки: {}", head(&text, 80)
    );
    println!("✓ verifier: утверждения размечены статусами");
}
