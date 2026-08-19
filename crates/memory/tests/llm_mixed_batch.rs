//! Live-тесты: batch-вердикты, параллельный поиск, обе модели DeepSeek,
//! температурная вариативность планирования.
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-memory --test llm_mixed_batch \
//!     -- --ignored --nocapture

use pr_llm::{CompletionRequest, DeepSeekProvider, LlmProvider};
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

// ── 1. Batch со смешанными вердиктами через LLM ────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn mixed_batch_verdicts_with_llm() {
    log_sep("Смешанный batch: duplicate + supersede + new");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    // Seed: факт, который потом будет дублем и факт, который заменят.
    let seed = AbsorbRequest {
        facts: vec![
            p_fact("The office is at 12 Tverskaya street in Moscow"),
            p_fact("Acme Corp CEO is Ivan Petrov as of 2024"),
        ],
        source: "mix".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline_with_llm(llm.clone()).absorb(seed).await.unwrap();

    // Batch: [точный дубль, supersede-кандидат, совсем новое].
    let req = AbsorbRequest {
        facts: vec![
            p_fact("The office is at 12 Tverskaya street in Moscow"), // duplicate
            p_fact("Acme Corp CEO is Maria Ivanova as of 2025"),      // supersede
            p_fact("Kubernetes 1.35 ships in 2026 with in-place pod resize"), // new
        ],
        source: "mix".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let report = mem.pipeline_with_llm(llm.clone()).absorb(req).await.unwrap();
    println!("{}", report.summary_line());
    println!("created={} skipped={} superseded={}", report.created, report.skipped, report.superseded);

    assert_eq!(report.skipped, 1, "точный дубль должен быть пропущен (hash-дедуп)");
    assert_eq!(report.superseded, 1, "смена CEO должна быть supersede");
    assert_eq!(report.created, 1, "новый факт Kubernetes создан");
    println!("✓ batch: три разных вердикта в одном вызове");
}

// ── 2. Параллельный поиск по сторe ─────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn parallel_searches_on_shared_store() {
    log_sep("Параллельный поиск: 12 одновременных hybrid_search");
    let llm = make_llm();
    let mem = Memory::in_memory(pr_core::MemoryConfig::default()).unwrap();

    let req = AbsorbRequest {
        facts: (0..30)
            .map(|i| p_fact(&format!("Technology fact number {i} about distributed systems and caching")))
            .collect(),
        source: "par".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for i in 0..12 {
        let mem = Memory {
            db: mem.db.clone(),
            embedder: mem.embedder.clone(),
            config: mem.config.clone(),
        };
        handles.push(tokio::spawn(async move {
            let q = format!("distributed caching systems topic {i}");
            mem.search(&q, &ScopeFilter::persistent(), Some(5)).await
        }));
    }
    let mut ok = 0;
    let mut total_hits = 0;
    for h in handles {
        match h.await.unwrap() {
            Ok(hits) => {
                ok += 1;
                total_hits += hits.len();
            }
            Err(e) => println!("   ошибка: {e}"),
        }
    }
    println!("успешно: {ok}/12, суммарно хитов: {total_hits}, время: {:?}", start.elapsed());
    assert_eq!(ok, 12, "все параллельные поиски должны пройти");
    println!("✓ параллельный поиск: гонок нет, SQLite в Mutex держит нагрузку");
}

// ── 3. Обе модели DeepSeek: flash и pro ────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn flash_and_pro_models_both_classify() {
    log_sep("Обе модели: deepseek-v4-flash и deepseek-v4-pro");
    let key = api_key();
    // Однозначная замена факта (тот же субъект, новое значение + новая дата).
    let classify_task = r#"You are a memory manager. A new fact: "Acme Corp CEO is Maria Ivanova as of 2025".
Candidate: [c0] "Acme Corp CEO is Ivan Petrov as of 2024".
Choose EXACTLY ONE verdict: duplicate | supersede | contradict | coexist | related | new.
Respond with ONLY JSON: {"candidate":"c0","verdict":"...","reason":"short"}"#;

    let mut seen = Vec::new();
    for model in ["deepseek-v4-flash", "deepseek-v4-pro"] {
        let llm = Arc::new(DeepSeekProvider::new("https://api.deepseek.com", &key, model));
        let start = std::time::Instant::now();
        let req = CompletionRequest {
            messages: vec![pr_core::Message::user(classify_task)],
            tools: vec![],
            temperature: Some(0.1),
            max_tokens: Some(2048), // запас для reasoning-моделей (v4-pro)
            stream: false,
        };
        let resp = llm.complete(&req).await.expect(&format!("{model} failed"));
        let text = msg_text(&resp.message);
        let verdict: serde_json::Value = serde_json::from_str(text.trim()).unwrap_or(serde_json::json!({}));
        let v = verdict.get("verdict").and_then(|v| v.as_str()).unwrap_or("?");
        println!("{model}: {v} за {:?}", start.elapsed());
        // Валидный вердикт обязателен; конкретный выбор у reasoning-модели
        // может зависеть от формата промпта (см. бенчмарк-документ).
        assert!(
            ["duplicate", "supersede", "contradict", "coexist", "related", "new"].contains(&v),
            "{model} вернул невалидный вердикт: {text}"
        );
        seen.push((model, v.to_string()));
    }
    println!("наблюдение: {:?}", seen);
    println!("✓ обе модели возвращают валидные классификационные вердикты");
}

// ── 4. Температурная вариативность: план дважды при t=0.7 ──────────────────

#[tokio::test]
#[ignore = "live API"]
async fn temperature_variance_still_parseable() {
    log_sep("Вариативность: один план-промпт дважды при t=0.7");
    let llm = make_llm();
    let prompt = r#"You are planning a research task. Decompose the following query into 2-5 independent sub-tasks that can be researched in parallel.

Query: Compare Rust and Go for building web backends in 2026

Respond with ONLY a JSON array of strings, where each string is a self-contained research task description. Each task should be specific enough for a researcher agent to complete independently.

Example format:
["Research the history of X", "Find current applications of Y", "Analyze the limitations of Z"]

Do NOT include any explanation, just the JSON array."#;

    let mut plans = Vec::new();
    for round in 1..=2 {
        let req = CompletionRequest {
            messages: vec![pr_core::Message::user(prompt)],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(2048),
            stream: false,
        };
        let resp = llm.complete(&req).await.expect("plan failed");
        let text = msg_text(&resp.message);
        let tasks: Vec<String> = serde_json::from_str(text.trim()).unwrap_or_else(|_| {
            let s = text.find('[').unwrap();
            let e = text.rfind(']').unwrap();
            serde_json::from_str(&text[s..=e]).expect("plan JSON unparsable")
        });
        println!("round {round}: {} задач — {}", tasks.len(), tasks.first().map(|t| head(t, 70)).unwrap_or_default());
        assert!((2..=5).contains(&tasks.len()), "план должен содержать 2-5 задач");
        plans.push(tasks);
    }
    let same = plans[0] == plans[1];
    println!("планы идентичны: {same}");
    println!("✓ оба плана парсятся{}", if same { " (детерминированно)" } else { " (с вариативностью)" });
}

fn head(s: &str, n: usize) -> &str {
    let mut end = s.len().min(n);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
