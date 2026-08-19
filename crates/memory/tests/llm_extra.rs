//! Live-extra направления memory + провайдера на DeepSeek:
//!   A) supersede-цепочка из 3 версий через LLM-классификацию
//!   B) context-hint: контекстные предпочтения оба активны
//!   C) contradict + boost: ручное усиление стороны конфликта
//!   D) multi-turn диалог (история сообщений)
//!   E) streaming function calling: реассемблирование tool_call-дельт
//!   F) edge-промпты: пустой, пробелы, эмодзи, длинное слово
//!   G) смешанный язык: RU-задача с EN-JSON структурой
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-memory --test llm_extra \
//!     -- --ignored --nocapture

use pr_llm::{CompletionRequest, DeepSeekProvider, LlmProvider, StreamChunk};
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

fn p_req(f: AbsorbFact) -> AbsorbRequest {
    AbsorbRequest {
        facts: vec![f],
        source: "extra".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }
}

// ── A. Supersede-цепочка из 3 версий через LLM ────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn a_llm_supersede_three_version_chain() {
    log_sep("A. LLM supersede-цепочка: 2023 → 2024 → 2025");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    let r1 = mem.pipeline_with_llm(llm.clone()).absorb(p_req(p_fact(
        "Acme Corp CEO is Ivan Petrov as of 2023"))).await.unwrap();
    let r2 = mem.pipeline_with_llm(llm.clone()).absorb(p_req(p_fact(
        "Acme Corp CEO is Sergey Ivanov as of 2024"))).await.unwrap();
    let r3 = mem.pipeline_with_llm(llm.clone()).absorb(p_req(p_fact(
        "Acme Corp CEO is Maria Ivanova as of 2025"))).await.unwrap();
    println!("v1: {} | v2: {} | v3: {}", r1.summary_line(), r2.summary_line(), r3.summary_line());

    let mut all = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    all.extend(mem.db.list(&ScopeFilter::persistent(), Some("superseded"), 100).unwrap());
    println!("все строки (active+superseded): {}", all.len());
    for r in &all {
        println!("   (status={}) {}", r.status, r.content);
    }
    let v1 = all.iter().find(|r| r.content.contains("2023")).expect("версия 2023 есть");
    let latest = pr_memory::search::resolve_follow(&mem.db, &v1.id, pr_memory::Follow::Latest).unwrap();
    assert_eq!(latest.len(), 1);
    assert!(latest[0].content.contains("Maria Ivanova"),
        "Latest должен разрешиться до версии 2025, получено: {}", latest[0].content);

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let ceo_active: Vec<_> = active.iter().filter(|r| r.content.contains("CEO")).collect();
    println!("активных CEO-строк: {}", ceo_active.len());
    assert_eq!(ceo_active.len(), 1, "должна остаться одна активная версия CEO");
    println!("✓ цепочка версий: Latest разрешается до 2025, активна одна версия");
}

// ── B. Context-hint: контекстные предпочтения ─────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn b_llm_context_hint_preferences_coexist() {
    log_sep("B. Context-hint: предпочтения с явным контекстом");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    let mut f1 = p_fact("I prefer terse code reviews with no preamble");
    f1.metadata = serde_json::json!({"context": "applies to work pull requests only"});
    mem.pipeline_with_llm(llm.clone()).absorb(p_req(f1)).await.unwrap();

    let mut f2 = p_fact("I prefer detailed long-form code reviews with examples");
    f2.metadata = serde_json::json!({"context": "applies to personal open-source projects only"});
    let r2 = mem.pipeline_with_llm(llm.clone()).absorb(p_req(f2)).await.unwrap();
    println!("второй факт: {}", r2.summary_line());

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let prefs: Vec<_> = active.iter().filter(|r| r.content.contains("reviews")).collect();
    println!("активных предпочтений: {}", prefs.len());
    assert!(prefs.len() >= 2, "контекстные предпочтения не должны вытеснять друг друга");
    println!("✓ context-hint: оба предпочтения остались активными");
}

// ── C. Contradict + boost: усиление стороны конфликта ──────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn c_llm_contradict_then_boost_sways_ranking() {
    log_sep("C. Contradict + boost: ручной перевес в конфликте");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    mem.pipeline_with_llm(llm.clone()).absorb(p_req(p_fact(
        "The company headquarters is located in Moscow"))).await.unwrap();
    let r2 = mem.pipeline_with_llm(llm.clone()).absorb(p_req(p_fact(
        "The company headquarters is located in Kazan"))).await.unwrap();
    println!("второй факт: {}", r2.summary_line());

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let hq: Vec<_> = active.iter().filter(|r| r.content.contains("headquarters")).collect();
    println!("активных HQ-строк: {}", hq.len());
    assert!(!hq.is_empty());

    // Усиливаем версию Kazan (важность — тай-брейкер ранжирования).
    let kazan = hq.iter().find(|r| r.content.contains("Kazan")).expect("Kazan-версия есть");
    mem.db.boost(&kazan.id, 2.0).unwrap();
    for _ in 0..3 {
        mem.db.record_access(&[kazan.id.clone()]);
    }

    let hits = mem.search("company headquarters location", &ScopeFilter::persistent(), None).await.unwrap();
    println!("ранжирование после boost:");
    for h in &hits {
        println!("   [{:.3}] (imp={}) {}", h.score, h.memory.importance, h.memory.content);
    }
    assert!(hits.iter().any(|h| h.memory.content.contains("Kazan")),
        "Kazan-версия должна быть в выдаче");
    if hits.len() >= 2 {
        let kazan_idx = hits.iter().position(|h| h.memory.content.contains("Kazan")).unwrap();
        println!("Kazan на позиции {}", kazan_idx + 1);
        assert_eq!(kazan_idx, 0, "boost + reinforcement должны вывести Kazan на первое место");
    }
    println!("✓ contradict+boost: усиленная сторона конфликта ранжируется выше");
}

// ── D. Multi-turn диалог ────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn d_llm_multi_turn_conversation() {
    log_sep("D. Multi-turn: модель помнит историю сообщений");
    let llm = make_llm();
    let req = CompletionRequest {
        messages: vec![
            pr_core::Message::user("My name is Hermann and my project is called Parallel."),
            pr_core::Message::assistant("Nice to meet you, Hermann!"),
            pr_core::Message::user("What is my name and what is my project called?"),
        ],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(100),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("multi-turn failed");
    let text = msg_text(&resp.message);
    println!("ответ: {}", text);
    assert!(text.contains("Hermann"), "модель должна помнить имя из истории: {text}");
    assert!(text.to_lowercase().contains("parallel"), "и название проекта: {text}");
    println!("✓ multi-turn: история сообщений работает");
}

// ── E. Streaming function calling ───────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn e_llm_streaming_tool_call_reassembly() {
    log_sep("E. Streaming tool-calls: реассемблирование дельт");
    let llm = make_llm();
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user("What is the weather in Sochi?")],
        tools: vec![pr_core::ToolSchema {
            name: "get_weather".into(),
            description: "Get current weather for a city".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"]
            }),
        }],
        temperature: Some(0.0),
        max_tokens: Some(500),
        stream: true,
    };
    let mut stream = llm.stream(&req).await.expect("stream failed");
    use futures::StreamExt;
    let mut tool_calls: std::collections::HashMap<usize, (String, String, String)> = std::collections::HashMap::new();
    let mut text = String::new();
    while let Some(c) = stream.next().await {
        match c.unwrap() {
            StreamChunk::Text { delta } => text.push_str(&delta),
            StreamChunk::ToolCallDelta { index, id, name, arguments_delta } => {
                let e = tool_calls.entry(index).or_insert_with(|| (id, name, String::new()));
                if !arguments_delta.is_empty() {
                    e.2.push_str(&arguments_delta);
                }
            }
            _ => {}
        }
    }
    println!("текст: {:?}", head(&text, 80));
    println!("tool-дельт: {}", tool_calls.len());
    for (idx, (id, name, args)) in &tool_calls {
        println!("   [{idx}] id={id} name={name} args={args}");
        assert_eq!(name, "get_weather", "имя инструмента должно реассемблироваться");
        let args_json: serde_json::Value = serde_json::from_str(args).unwrap_or(serde_json::json!({}));
        assert_eq!(args_json["city"], "Sochi", "аргументы должны реассемблироваться");
    }
    assert!(!tool_calls.is_empty(), "модель должна вызвать get_weather");
    println!("✓ streaming tool-call: имя и аргументы собраны из SSE-дельт");
}

// ── F. Edge-промпты ─────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn f_llm_edge_prompts_no_panic() {
    log_sep("F. Edge-промпты: пустой, пробелы, эмодзи, длинное слово");
    let llm = make_llm();
    let long_word = "supercalifragilistic".repeat(200);
    let cases: Vec<(&str, &str)> = vec![
        ("empty", ""),
        ("spaces", "   "),
        ("emoji", "😀😀😀"),
        ("long_word", &long_word),
        ("one_char", "x"),
    ];
    for (label, input) in cases {
        let req = CompletionRequest {
            messages: vec![pr_core::Message::user(input)],
            tools: vec![],
            temperature: Some(0.0),
            max_tokens: Some(50),
            stream: false,
        };
        match llm.complete(&req).await {
            Ok(resp) => println!("{label:>10}: ok ({} символов)", msg_text(&resp.message).len()),
            Err(e) => println!("{label:>10}: ошибка: {}", head(&e.to_string(), 80)),
        }
        // Никаких паник — любой исход валиден для edge-промптов.
    }
    println!("✓ edge-промпты: паники нет");
}

// ── G. Смешанный язык: RU-задача + EN-JSON ─────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn g_llm_mixed_language_json() {
    log_sep("G. Смешанный язык: русская задача, JSON-ответ");
    let llm = make_llm();
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user(
            "Классифицируй факт «Компания Акме переехала из Москвы в Казань» против кандидата \
             «Компания Акме находится в Москве». Верни ТОЛЬКО JSON: \
             {\"verdict\": \"duplicate|supersede|contradict|coexist|related|new\", \"reason\": \"коротко\"}",
        )],
        tools: vec![],
        temperature: Some(0.1),
        max_tokens: Some(1024),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("mixed-language failed");
    let text = msg_text(&resp.message);
    println!("ответ: {}", head(&text, 150));
    let parsed: serde_json::Value = serde_json::from_str(text.trim()).unwrap_or(serde_json::json!({}));
    let v = parsed.get("verdict").and_then(|v| v.as_str()).unwrap_or("?");
    assert!(
        ["duplicate", "supersede", "contradict", "coexist", "related", "new"].contains(&v),
        "вердикт должен быть валидным, получено: {text}"
    );
    println!("✓ mixed-language: RU-задача даёт валидный EN-JSON вердикт: {v}");
}


// ── H. Rerank на 10 кандидатов: ничего не теряется ─────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn h_llm_rerank_ten_hits_no_drops() {
    log_sep("H. Rerank на 10 хитов: перестановка без потерь");
    let llm = make_llm();
    let mem = Memory::in_memory(pr_core::MemoryConfig::default()).unwrap();

    let pool = [
        "Redis caches query results in memory for fast reads",
        "Memcached stores session data in memory across servers",
        "CDN edge nodes cache static assets close to users",
        "Application-level caching reduces database load significantly",
        "In-memory caches handle query result caching efficiently",
        "Caching query results avoids repeated database round trips",
        "LRU eviction policies manage cache memory usage",
        "Session data caching improves web application latency",
        "Distributed caches share memory across multiple nodes",
        "Cache invalidation is difficult when data changes often",
        "Read-through caching fetches missing keys from the database",
        "Write-behind caching defers database writes to a queue",
    ];
    let facts: Vec<AbsorbFact> = pool.iter().map(|c| AbsorbFact {
        content: c.to_string(),
        metadata: serde_json::json!({}),
        tags: vec![],
        confidence: None,
        memory_class: None,
    }).collect();
    mem.pipeline().absorb(AbsorbRequest {
        facts,
        source: "rerank10".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }).await.unwrap();

    let hits = pr_memory::search::hybrid_search(
        &mem.db,
        &mem.embedder,
        &pr_memory::SearchParams {
            query: "in-memory caching query results".into(),
            top_k: 10,
            min_score: 0.02,
            semantic_weight: 0.7,
            temporal_decay: 0.0,
            scope: ScopeFilter::persistent(),
        },
    ).await.unwrap();
    println!("хитов до rerank: {}", hits.len());
    assert!(hits.len() >= 5, "нужно >=5 кандидатов, получено {}", hits.len());

    let llm_dyn: Arc<dyn pr_llm::LlmProvider> = llm.clone();
    let reranked = pr_memory::search::llm_rerank(&llm_dyn, "in-memory caching query results", hits.clone()).await;
    println!("после rerank: {} хитов", reranked.len());
    // llm_rerank обязан сохранить всех кандидатов (неуместные — в хвосте).
    assert_eq!(reranked.len(), hits.len(), "rerank не должен терять кандидатов");
    let before: std::collections::HashSet<String> = hits.iter().map(|h| h.memory.id.clone()).collect();
    let after: std::collections::HashSet<String> = reranked.iter().map(|h| h.memory.id.clone()).collect();
    assert_eq!(before, after, "состав кандидатов после rerank не меняется");
    println!("✓ rerank×10: перестановка без потери кандидатов");
}

// ── I. Batch-классификация через LLM: смешанные вердикты ───────────────────

#[tokio::test]
#[ignore = "live API"]
async fn i_llm_batch_classify_mixed_verdicts() {
    log_sep("I. Batch-классификация: duplicate+supersede+new в одном вызове");
    let llm = make_llm();
    let mut cfg = pr_core::MemoryConfig::default();
    cfg.llm_classify = true;
    let mem = Memory::in_memory(cfg).unwrap();

    // Seed: две строки для дубля и замены.
    let seed = AbsorbRequest {
        facts: vec![
            p_fact("The office is at 12 Tverskaya street in Moscow"),
            p_fact("Acme Corp CEO is Ivan Petrov as of 2024"),
        ],
        source: "batch".into(), scope: Scope::Agent, scope_key: String::new(), context: None, dry_run: false,
    };
    mem.pipeline_with_llm(llm.clone()).absorb(seed).await.unwrap();

    let batch = AbsorbRequest {
        facts: vec![
            p_fact("The office is at 12 Tverskaya street in Moscow"),   // duplicate
            p_fact("Acme Corp CEO is Maria Ivanova as of 2025"),        // supersede
            p_fact("Kubernetes 1.36 ships in 2026 with dynamic resource classes"), // new
            p_fact("Kubernetes 1.36 ships in 2026 with dynamic resource classes"), // дубль внутри батча
        ],
        source: "batch".into(), scope: Scope::Agent, scope_key: String::new(), context: None, dry_run: false,
    };
    let report = mem.pipeline_with_llm(llm.clone()).absorb(batch).await.unwrap();
    println!("{}", report.summary_line());
    println!("created={} skipped={} superseded={} consolidated={}",
        report.created, report.skipped, report.superseded, report.consolidated);

    assert_eq!(report.skipped, 1, "первый факт батча — точный дубль");
    assert_eq!(report.superseded, 1, "смена CEO — supersede");
    // Внутрибатчевый дубль (две одинаковые строки Kubernetes) консолидируется
    // или пропускается hash-дедупом; создаётся ровно одна строка про 1.36.
    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let k8s: Vec<_> = active.iter().filter(|r| r.content.contains("1.36")).collect();
    assert_eq!(k8s.len(), 1, "две одинаковые строки Kubernetes → одна: {}", k8s.len());
    let ceo: Vec<_> = active.iter().filter(|r| r.content.contains("CEO")).collect();
    assert_eq!(ceo.len(), 1, "активна одна версия CEO");
    assert!(ceo[0].content.contains("Maria"), "активна новая версия");
    println!("✓ batch-classify: три вердикта + внутрибатчевый дубль обработаны корректно");
}

fn head(s: &str, n: usize) -> &str {
    let mut end = s.len().min(n);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
