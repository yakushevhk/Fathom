//! Live-бенчмарк латентностей DeepSeek через штатный DeepSeekProvider.
//! Печатает markdown-таблицу для отчёта; ассерты минимальны (это bench).
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-llm --test llm_live_bench \
//!     -- --ignored --nocapture

use pr_llm::{CompletionRequest, DeepSeekProvider, LlmProvider};
use std::sync::Arc;
use std::time::{Duration, Instant};

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

fn msg_text(m: &pr_core::Message) -> String {
    match m {
        pr_core::Message::System { content } | pr_core::Message::User { content } => content.clone(),
        pr_core::Message::Assistant { content, .. } => content.clone().unwrap_or_default(),
        pr_core::Message::Tool { content, .. } => content.clone(),
    }
}

async fn timed(llm: &DeepSeekProvider, req: CompletionRequest) -> (Duration, String) {
    let start = Instant::now();
    let resp = llm.complete(&req).await.expect("bench call failed");
    (start.elapsed(), msg_text(&resp.message))
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

#[tokio::test]
#[ignore = "live API"]
async fn llm_latency_benchmark() {
    let key = api_key();
    let flash = DeepSeekProvider::new("https://api.deepseek.com", &key, "deepseek-v4-flash");

    println!("\n# LLM latency benchmark (DeepSeek, live)");
    println!("model: deepseek-v4-flash (reasoning)");

    // ── 1. Classify (маленький вызов, как в AbsorbPipeline::classify) ──────
    let classify_task = r#"You are a memory manager. New fact: "Acme Corp CEO is Maria Ivanova as of 2025".
Candidate: [c0] "Acme Corp CEO is Ivan Petrov as of 2024".
Choose EXACTLY ONE verdict: duplicate | supersede | contradict | coexist | related | new.
Respond with ONLY JSON: {"candidate":"c0","verdict":"...","reason":"short"}"#;
    let mut lat = Vec::new();
    for _ in 0..5 {
        let (d, text) = timed(&flash, CompletionRequest {
            messages: vec![pr_core::Message::user(classify_task)],
            tools: vec![],
            temperature: Some(0.1),
            max_tokens: Some(2048),
            stream: false,
        }).await;
        assert!(!text.is_empty());
        lat.push(d);
    }
    let avg = lat.iter().sum::<Duration>() / lat.len() as u32;
    println!("\n## classify (маленький вызов, n=5)");
    println!("| метрика | значение |");
    println!("|---|---|");
    println!("| avg | {:.2}s |", avg.as_secs_f64());
    println!("| median | {:.2}s |", median(lat.clone()).as_secs_f64());
    println!("| min | {:.2}s |", lat.iter().min().unwrap().as_secs_f64());
    println!("| max | {:.2}s |", lat.iter().max().unwrap().as_secs_f64());

    // ── 2. Plan (2048 токенов) ─────────────────────────────────────────────
    let plan_prompt = r#"You are planning a research task. Decompose the following query into 2-5 independent sub-tasks that can be researched in parallel.

Query: Compare Rust and Go for building web backends in 2026

Respond with ONLY a JSON array of strings."#;
    let (d_plan, plan) = timed(&flash, CompletionRequest {
        messages: vec![pr_core::Message::user(plan_prompt)],
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(2048),
        stream: false,
    }).await;
    let plan_tasks = serde_json::from_str::<Vec<String>>(plan.trim()).ok().map(|v| v.len()).unwrap_or(0);
    println!("\n## plan (2048 токенов, {} задач)", plan_tasks);
    println!("| метрика | значение |");
    println!("|---|---|");
    println!("| latency | {:.2}s |", d_plan.as_secs_f64());

    // ── 3. Synthesis (~1500 токенов вывода) ────────────────────────────────
    let synth_prompt = r#"You are synthesizing research findings into a final report.

Original query: Сравнить PostgreSQL и MySQL для OLTP

Findings from sub-agents:

### Finding 1
PostgreSQL 17 показывает 120k TPS в sysbench на 16 ядрах (percona.com, 2026-03).
### Finding 2
MySQL 8.4 достигает 95k TPS на том же железе, p99 latency ниже (mysql.com, 2026-02).

Write a comprehensive, well-structured markdown report that answers the query, notes contradictions, lists sources and identifies gaps."#;
    let (d_synth, synth) = timed(&flash, CompletionRequest {
        messages: vec![pr_core::Message::user(synth_prompt)],
        tools: vec![],
        temperature: Some(0.5),
        max_tokens: Some(2048),
        stream: false,
    }).await;
    println!("\n## synthesis ({} символов вывода)", synth.chars().count());
    println!("| метрика | значение |");
    println!("|---|---|");
    println!("| latency | {:.2}s |", d_synth.as_secs_f64());

    // ── 4. Streaming: TTFT (time to first token) ───────────────────────────
    let llm = Arc::new(flash);
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user("Напиши список из 30 столиц мира.")],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(2000),
        stream: true,
    };
    let start = Instant::now();
    let mut stream = llm.stream(&req).await.unwrap();
    use futures::StreamExt;
    let mut ttft = None;
    let mut chunks = 0u32;
    let mut total_chars = 0usize;
    while let Some(c) = stream.next().await {
        match c.unwrap() {
            pr_llm::StreamChunk::Text { delta } => {
                if ttft.is_none() {
                    ttft = Some(start.elapsed());
                }
                chunks += 1;
                total_chars += delta.chars().count();
            }
            _ => {}
        }
    }
    println!("\n## streaming (30 столиц)");
    println!("| метрика | значение |");
    println!("|---|---|");
    println!("| TTFT | {:.2}s |", ttft.unwrap_or_default().as_secs_f64());
    println!("| total | {:.2}s |", start.elapsed().as_secs_f64());
    println!("| chunks | {chunks} |");
    println!("| chars | {total_chars} |");

    // ── 5. Parallel fan-out: 10 одновременных ─────────────────────────────
    let mut handles = Vec::new();
    let start = Instant::now();
    for i in 0..10 {
        let llm = llm.clone();
        handles.push(tokio::spawn(async move {
            let req = CompletionRequest {
                messages: vec![pr_core::Message::user(format!("Reply with the two characters: a{i}"))],
                tools: vec![],
                temperature: Some(0.0),
                max_tokens: Some(10),
                stream: false,
            };
            llm.complete(&req).await.is_ok()
        }));
    }
    let mut ok = 0;
    for h in handles {
        if h.await.unwrap() {
            ok += 1;
        }
    }
    let parallel = start.elapsed();
    println!("\n## parallel fan-out (10 вызовов)");
    println!("| метрика | значение |");
    println!("|---|---|");
    println!("| total | {:.2}s |", parallel.as_secs_f64());
    println!("| per-call amortized | {:.2}s |", parallel.as_secs_f64() / 10.0);
    println!("| успешно | {ok}/10 |");

    println!("\n✓ benchmark завершён");
}
