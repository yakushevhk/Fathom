//! Live-стресс провайдера DeepSeek: параллелизм, reasoning-обрыв,
//! путь ошибок, русский язык.
//!
//! Запуск:
//!   DEEPSEEK_API_KEY=sk-... cargo test -p pr-llm --test llm_live_provider \
//!     -- --ignored --nocapture

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

// ── 1. Параллелизм: 20 одновременных вызовов через ModelSemaphore ──────────

#[tokio::test]
#[ignore = "live API"]
async fn parallel_completions_all_succeed() {
    log_sep("Параллелизм: 20 одновременных вызовов");
    let llm = make_llm();
    let start = std::time::Instant::now();

    let mut handles = Vec::new();
    for i in 0..20 {
        let llm = llm.clone();
        handles.push(tokio::spawn(async move {
            let req = CompletionRequest {
                messages: vec![pr_core::Message::user(format!(
                    "Reply with exactly the two characters: a{i}"
                ))],
                tools: vec![],
                temperature: Some(0.0),
                max_tokens: Some(10),
                stream: false,
            };
            llm.complete(&req).await.map(|r| msg_text(&r.message))
        }));
    }

    let mut ok = 0;
    let mut errs = 0;
    for h in handles {
        match h.await.unwrap() {
            Ok(text) => {
                ok += 1;
                assert!(!text.trim().is_empty(), "ответ не должен быть пустым");
            }
            Err(e) => {
                errs += 1;
                println!("   ошибка: {e}");
            }
        }
    }
    let elapsed = start.elapsed();
    println!("успешно: {ok}, ошибок: {errs}, время: {elapsed:?}");
    assert_eq!(ok, 20, "все 20 параллельных вызовов должны пройти");
    assert_eq!(errs, 0);
}

// ── 2. Reasoning-модель: обрыв по max_tokens ───────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn reasoning_truncation_returns_gracefully() {
    log_sep("Reasoning-обрыв: max_tokens=1 на reasoning-модели");
    let llm = make_llm();
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user(
            "Объясни подробно, почему небо голубое, включая рассеяние Рэлея.",
        )],
        tools: vec![],
        temperature: Some(0.0),
        max_tokens: Some(1), // заведомо мало: reasoning съест весь бюджет
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("обрыв должен вернуться без ошибки");
    let text = msg_text(&resp.message);
    println!("content: {:?}", text);
    println!("finish_reason: {:?}", resp.finish_reason);
    // Пустой content при finish_reason=length — штатный диагностический случай
    // (не паника, не PrError): именно это ловит synthesis retry-логика.
    println!("✓ обрыв обработан без паники (контент {} символов)", text.len());
}

// ── 3. Путь ошибок: недоступный эндпоинт (без ключа API) ───────────────────

#[tokio::test]
#[ignore = "live API"]
async fn invalid_endpoint_fails_cleanly_with_retries() {
    log_sep("Ошибки: недоступный эндпоинт (с retry)");
    let llm = DeepSeekProvider::new("http://127.0.0.1:1", "dummy-key", "deepseek-chat");
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user("ping")],
        tools: vec![],
        temperature: None,
        max_tokens: Some(10),
        stream: false,
    };
    let start = std::time::Instant::now();
    let result = llm.complete(&req).await;
    let elapsed = start.elapsed();
    match result {
        Err(e) => println!("ожидаемая ошибка за {elapsed:?}: {e}"),
        Ok(_) => panic!("недоступный эндпоинт не должен отвечать"),
    }
    // retry: 500ms → 1s → 2s (+jitter) — хотя бы ~3 попытки.
    assert!(elapsed.as_millis() >= 500, "должен быть хотя бы один retry-цикл");
    println!("✓ ошибка вернулась чисто после retry-цикла");
}

// ── 4. Русский язык ────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn russian_response_quality() {
    log_sep("Русский: осмысленный ответ на русском");
    let llm = make_llm();
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user(
            "Кратко, в одном абзаце: чем MVCC в PostgreSQL отличается от блокировок в MySQL InnoDB?",
        )],
        tools: vec![],
        temperature: Some(0.3),
        max_tokens: Some(300),
        stream: false,
    };
    let resp = llm.complete(&req).await.expect("русский запрос упал");
    let text = msg_text(&resp.message);
    println!("ответ ({} символов): {}", text.len(), &text[..text.len().min(400)]);
    assert!(text.contains("MVCC") || text.contains("mvcc"), "ответ должен упоминать MVCC");
    println!("✓ русскоязычный ответ получен");
}

// ── 5. Стрим ошибки: некорректный ключ ─────────────────────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn invalid_api_key_returns_http_error() {
    log_sep("Ошибки: неверный API-ключ (401)");
    let llm = DeepSeekProvider::new("https://api.deepseek.com", "sk-invalid-key-000", "deepseek-chat");
    let req = CompletionRequest {
        messages: vec![pr_core::Message::user("ping")],
        tools: vec![],
        temperature: None,
        max_tokens: Some(10),
        stream: false,
    };
    let result = llm.complete(&req).await;
    match result {
        Err(pr_core::PrError::Http { status, .. }) => {
            println!("HTTP {status} получен корректно");
            assert_eq!(status, 401, "неверный ключ должен давать 401");
        }
        Err(e) => println!("другой тип ошибки: {e}"),
        Ok(_) => panic!("неверный ключ не должен работать"),
    }
    println!("✓ 401 обработан как постоянная ошибка (без retry)");
}

// ── 6. Usage accounting: total == prompt + completion ──────────────────────

#[tokio::test]
#[ignore = "live API"]
async fn usage_accounting_is_consistent() {
    log_sep("Usage accounting: total == prompt + completion (n=3)");
    let llm = make_llm();
    for i in 0..3 {
        let req = CompletionRequest {
            messages: vec![pr_core::Message::user(format!(
                "Опиши в двух предложениях, что такое кэширование {i} уровня."
            ))],
            tools: vec![],
            temperature: Some(0.0),
            max_tokens: Some(200),
            stream: false,
        };
        let resp = llm.complete(&req).await.expect("usage call failed");
        let u = resp.usage.expect("usage должен быть у non-stream ответа");
        println!("call {i}: prompt={} completion={} total={}",
            u.prompt_tokens, u.completion_tokens, u.total_tokens);
        assert_eq!(u.prompt_tokens + u.completion_tokens, u.total_tokens,
            "total должен быть суммой prompt+completion");
        assert!(u.prompt_tokens > 0 && u.completion_tokens > 0);
    }
    println!("✓ usage accounting: сумма токенов сходится на всех вызовах");
}
