//! Live-тесты LLM-зависимых направлений агентского крейта на реальном DeepSeek.
//!
//!   A) Компакция контекста — полный `CompactionEngine::compact()` с реальным LLM
//!   B) Планирование — декомпозиция research-запроса (тот же промпт, что в Coordinator::plan)
//!   C) Планирование — LeadGen-декомпозиция с квотами
//!   D) Goal-judge — вердикт "полно/неполно" с подзадачами дозаполнения
//!
//! Ключ: DEEPSEEK_API_KEY (env или .env/.env на уровне workspace). Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-agent --test llm_live_agent \
//!     -- --ignored --nocapture

use pr_core::{ContextConfig, Message};
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
    panic!("DEEPSEEK_API_KEY not found in env or .env");
}

fn make_llm() -> Arc<DeepSeekProvider> {
    Arc::new(DeepSeekProvider::new(
        "https://api.deepseek.com",
        &api_key(),
        "deepseek-chat",
    ))
}

fn msg_text(m: &Message) -> String {
    match m {
        Message::System { content } | Message::User { content } => content.clone(),
        Message::Assistant { content, .. } => content.clone().unwrap_or_default(),
        Message::Tool { content, .. } => content.clone(),
    }
}

fn log_sep(t: &str) {
    println!("\n────────────────── {t} ──────────────────");
}

async fn complete(llm: &Arc<DeepSeekProvider>, messages: Vec<Message>, max_tokens: u32) -> String {
    let req = CompletionRequest {
        messages,
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(max_tokens),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("LLM call failed");
    msg_text(&resp.message)
}

// ── A. Compaction: полный pipeline с реальным LLM ──────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn a_llm_compaction_full_pipeline() {
    log_sep("A. CompactionEngine::compact() с реальным DeepSeek");
    let llm = make_llm();

    // Много уникальных сообщений, чтобы перешагнуть порог 50% от
    // fail-closed окна (min 32K токенов → порог ~16K токенов).
    let mut messages = vec![
        Message::system("You are a research assistant. Work methodically, cite sources."),
        Message::user("Research the state of superconducting quantum computing and write a report."),
    ];
    for i in 0..400 {
        messages.push(Message::user(format!(
            "Observation {i}: measured qubit coherence time of {:.1} microseconds in dilution \
             refrigerator run at 15 millikelvin; error rate per two-qubit gate was {:.2}%; \
             surface code distance 5 corrected {:.2}% of injected faults; architecturally, \
             the trapped-ion platform showed connectivity {i}/40 while superconducting chips \
             limited to nearest neighbours; open question {i}: how to scale calibration \
             overhead linearly with qubit count.",
            i as f32 * 1.37 % 180.0,
            i as f32 * 0.13 % 3.0,
            i as f32 * 0.71 % 99.0,
        )));
    }
    messages.push(Message::user("Synthesize the final report from all observations."));

    let before = pr_core::estimate_messages_tokens(&messages);
    println!("estimated tokens before: {before}");

    let mut engine = pr_agent::compaction::CompactionEngine::new(ContextConfig {
        context_window: 2000, // explicit ≠ default → Confirmed, but safety floor clamps to 32K
        ..Default::default()
    });

    let llm2 = llm.clone();
    let result = engine
        .compact(&mut messages, move |prompt| {
            let llm = llm2.clone();
            async move {
                let req = CompletionRequest {
                    messages: prompt,
                    tools: vec![],
                    temperature: Some(0.3),
                    max_tokens: Some(600),
                    stream: false,
                };
                let resp = llm.complete(&req).await?;
                Ok(msg_text(&resp.message))
            }
        })
        .await
        .expect("compaction failed");

    println!("used_llm: {}", result.used_llm);
    println!("tokens_before: {}  tokens_after: {}", result.tokens_before, result.tokens_after);
    println!("micro_pruned: {}  cooldown: {}", result.micro_pruned, result.cooldown_triggered);

    let summary = result
        .messages
        .iter()
        .find(|m| matches!(m, Message::System { .. }))
        .map(msg_text)
        .unwrap_or_default();
    println!("summary length: {} chars", summary.len());
    println!("summary head: {}", &summary[..summary.len().min(220)]);

    assert!(result.used_llm, "LLM summarization must have run");
    assert!(!summary.is_empty(), "summary must be non-empty");
    assert!(result.tokens_after < result.tokens_before, "compaction must reduce tokens");
    println!("✓ compaction: LLM-саммаризация выполнена, токены сокращены");
}

// ── B. Planning: research-декомпозиция (промпт Coordinator::plan) ─────────

#[tokio::test]
#[ignore = "live API"]
async fn b_llm_plan_research_decompose() {
    log_sep("B. Planning (research) — JSON array из 2-5 подзадач");
    let llm = make_llm();
    let query = "Compare PostgreSQL and MySQL for high-throughput OLTP workloads in 2026";

    let prompt = format!(
        r#"You are planning a research task. Decompose the following query into 2-5 independent sub-tasks that can be researched in parallel.

Query: {query}

Respond with ONLY a JSON array of strings, where each string is a self-contained research task description. Each task should be specific enough for a researcher agent to complete independently.

Example format:
["Research the history of X", "Find current applications of Y", "Analyze the limitations of Z"]

Do NOT include any explanation, just the JSON array."#
    );

    let text = complete(&llm, vec![Message::user(prompt)], 2048).await;
    println!("raw: {}", &text[..text.len().min(200)]);

    let tasks: Vec<String> = serde_json::from_str(text.trim()).unwrap_or_else(|_| {
        // fallback: extract [...] block (mirrors Coordinator::plan)
        let s = text.find('[').unwrap();
        let e = text.rfind(']').unwrap();
        serde_json::from_str(&text[s..=e]).expect("plan JSON array unparsable")
    });

    println!("parsed {} sub-tasks:", tasks.len());
    for t in &tasks {
        println!("   - {t}");
    }
    assert!((2..=5).contains(&tasks.len()), "plan must contain 2-5 subtasks, got {}", tasks.len());
    assert!(tasks.iter().any(|t| t.to_lowercase().contains("postgres") || t.to_lowercase().contains("mysql")),
        "subtasks must cover the query subject");
    println!("✓ planning: research-запрос декомпозирован на {} задач", tasks.len());
}

// ── C. Planning: LeadGen-декомпозиция с квотами ────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn c_llm_plan_leadgen_decompose() {
    log_sep("C. Planning (LeadGen) — подзадачи с квотами");
    let llm = make_llm();
    let query = "Найди контакты CEO IT-компаний Москвы, нужно 20 верифицированных email";

    let prompt = format!(
        r#"You are planning an OSINT / lead-generation task: harvesting contacts (emails, phones, persons, companies).

Query: {query}

Decompose it into 2-5 NON-OVERLAPPING collection sub-tasks that can run in parallel. Partition by a dimension that avoids duplicate work: company industry, company name range, city district, source type (directories vs social vs corporate sites), or role.

Each sub-task MUST be self-contained and include: the exact target description, the preferred tools (search_business_directory, find_leads, parse_corporate_site, extract_contacts, search_social), and — if the query states an overall target — a per-task quota (roughly total / number of tasks).

Respond with ONLY a JSON array of strings. Example:
["Find CEO/CTO contacts of Moscow IT companies A-M via search_business_directory + parse_corporate_site; quota: 5 verified emails", "Find CEO/CTO contacts of Moscow IT companies N-Z via search_business_directory + parse_corporate_site; quota: 5 verified emails"]

Do NOT include any explanation, just the JSON array."#
    );

    let text = complete(&llm, vec![Message::user(prompt)], 2048).await;
    println!("raw: {}", &text[..text.len().min(250)]);

    let tasks: Vec<String> = serde_json::from_str(text.trim()).unwrap_or_else(|_| {
        let s = text.find('[').unwrap();
        let e = text.rfind(']').unwrap();
        serde_json::from_str(&text[s..=e]).expect("leadgen JSON array unparsable")
    });

    println!("parsed {} sub-tasks:", tasks.len());
    for t in &tasks {
        println!("   - {t}");
    }
    assert!((2..=5).contains(&tasks.len()), "leadgen plan must have 2-5 subtasks, got {}", tasks.len());
    let with_quota = tasks.iter().filter(|t| t.to_lowercase().contains("quota") || t.contains("квот")).count();
    println!("subtasks with quota: {with_quota}/{}", tasks.len());
    assert!(with_quota >= 1, "at least one subtask should carry a quota");
    println!("✓ planning: LeadGen-запрос декомпозирован на {} задач с квотами", tasks.len());
}

// ── D. Goal-judge: полный/неполный вердикт ─────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn d_llm_goal_judge_complete_and_gap() {
    log_sep("D. Goal-judge — complete=true и complete=false сценар­ии");
    let llm = make_llm();

    let judge_prompt = |goal: &str, results: &str| {
        format!(
            r#"You are the goal-checker for a research session.

ORIGINAL GOAL:
{goal}

RESULTS COLLECTED SO FAR:
{results}

Decide whether the collected results FULLY satisfy the original goal. Be strict but fair: only flag a gap if something concrete and important is missing — not nice-to-haves.

Respond with ONLY JSON:
{{"complete": true, "new_subtasks": []}}
or, if concrete gaps remain:
{{"complete": false, "new_subtasks": ["<self-contained gap-filling task>", ...]}}

Rules: at most 3 new_subtasks; each must be independently executable by a researcher and target a specific gap; if the goal is met return complete=true with an empty array. No explanation outside the JSON."#
        )
    };

    // Scenario 1: goal fully met.
    let goal = "Сравнить PostgreSQL и MySQL по производительности OLTP";
    let results = "Найдено 5 бенчмарков 2025-2026: PostgreSQL 17 — 120k TPS, MySQL 8.4 — 95k TPS на одинаковом железе; \
                   проанализированы индексы, репликация, изоляция транзакций; выводы подкреплены источниками.";
    let text = complete(&llm, vec![Message::user(judge_prompt(goal, results))], 1024).await;
    let verdict: serde_json::Value = serde_json::from_str(text.trim())
        .unwrap_or_else(|_| {
            let s = text.find('{').unwrap();
            let e = text.rfind('}').unwrap();
            serde_json::from_str(&text[s..=e]).expect("judge JSON unparsable")
        });
    println!("scenario 1 verdict: {}", verdict);
    assert_eq!(verdict["complete"].as_bool(), Some(true), "full results → complete=true");

    // Scenario 2: concrete gap remains.
    let results2 = "Найдено 2 бенчмарка PostgreSQL (2024). Данных по MySQL нет, сравнения нет.";
    let text2 = complete(&llm, vec![Message::user(judge_prompt(goal, results2))], 1024).await;
    let verdict2: serde_json::Value = serde_json::from_str(text2.trim())
        .unwrap_or_else(|_| {
            let s = text2.find('{').unwrap();
            let e = text2.rfind('}').unwrap();
            serde_json::from_str(&text2[s..=e]).expect("judge JSON unparsable")
        });
    println!("scenario 2 verdict: {}", verdict2);
    assert_eq!(verdict2["complete"].as_bool(), Some(false), "missing MySQL → complete=false");
    assert!(verdict2["new_subtasks"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "gap scenario must propose new subtasks");
    println!("✓ goal-judge: оба сценария (полный / неполный) распознаны корректно");
}
