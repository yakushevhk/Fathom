//! Юнит-тесты фич memory-engineering (без сети, детерминированные):
//!   - cross-call consolidation (эвристический merge)
//!   - MemoryClass (ephemeral / durable / expiring)
//!   - скоринг поиска: confidence + reinforcement
//!   - GC: decay-сопротивление у часто читаемых, сохранение updated_at
//!   - merge_into: объединение контента/тегов/confidence

use pr_memory::{
    content_hash, AbsorbFact, AbsorbRequest, GcOptions, Memory, MemoryRow, Scope, ScopeFilter,
};
use serde_json::json;

fn in_memory() -> Memory {
    Memory::in_memory(pr_core::MemoryConfig::default()).unwrap()
}

fn fact(content: &str, memory_class: Option<&str>) -> AbsorbFact {
    AbsorbFact {
        content: content.into(),
        metadata: json!({}),
        tags: vec![],
        confidence: None,
        memory_class: memory_class.map(String::from),
    }
}

fn req(f: AbsorbFact) -> AbsorbRequest {
    AbsorbRequest {
        facts: vec![f],
        source: "unit".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }
}

fn raw_row(mem: &Memory, content: &str, conf: f64, access: i64) -> MemoryRow {
    let now = chrono::Utc::now().to_rfc3339();
    let row = MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_string(),
        metadata: json!({}),
        tags: vec![],
        source: "unit".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: conf,
        importance: 1.0,
        access_count: access,
        last_accessed: None,
        status: "active".into(),
        expires_at: None,
        content_hash: content_hash(content),
        created_at: now.clone(),
        updated_at: now,
    };
    mem.db.insert(&row).unwrap();
    row
}

// ── Cross-call consolidation ────────────────────────────────────────────────

#[tokio::test]
async fn cross_call_merge_combines_similar_fact_into_one_row() {
    // Эмпирически (TF-IDF): cosine этих двух фактов = 0.938 → диапазон
    // merge [0.85, 0.97), subject пересекается > 0.3.
    let mem = in_memory();
    let a = "The company Acme Corp is based in Moscow and develops CRM software";
    let b = "The company Acme Corp is based in Moscow and develops CRM and ERP software";

    let r1 = mem.pipeline().absorb(req(fact(a, None))).await.unwrap();
    assert_eq!(r1.created, 1);

    let r2 = mem.pipeline().absorb(req(fact(b, None))).await.unwrap();
    assert_eq!(r2.created, 0, "второй факт не должен создать новую строку: {}", r2.summary_line());
    assert!(r2.consolidated >= 1, "должен сработать cross-call merge: {}", r2.summary_line());

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    assert_eq!(active.len(), 1, "должна остаться одна объединённая строка");
    assert!(active[0].content.contains(a) && active[0].content.contains(b),
        "контент должен быть объединён: {}", active[0].content);
}

#[tokio::test]
async fn cross_call_merge_does_not_fire_below_threshold() {
    // cosine = 0.714 → ниже порога 0.85 → новый факт остаётся отдельной строкой.
    let mem = in_memory();
    let r1 = mem.pipeline().absorb(req(fact(
        "Redis is great for caching session data", None))).await.unwrap();
    assert_eq!(r1.created, 1);

    let r2 = mem.pipeline().absorb(req(fact(
        "Redis works well for caching session data", None))).await.unwrap();
    assert_eq!(r2.created, 1, "ниже порога merge факт должен быть отдельным: {}", r2.summary_line());
    assert_eq!(r2.consolidated, 0);

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    assert_eq!(active.len(), 2, "две строки, без объединения");
}

// ── MemoryClass ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn memory_class_ephemeral_forced_to_run_scope() {
    let mem = in_memory();
    let r = mem.pipeline().absorb(req(fact(
        "This bug is annoying right now", Some("ephemeral")))).await.unwrap();
    assert_eq!(r.created, 1);

    let agent_rows = mem.db.list(&ScopeFilter::new().add(Scope::Agent, ""), Some("active"), 100).unwrap();
    assert!(agent_rows.iter().all(|r| !r.content.contains("bug is annoying")),
        "ephemeral-факт не должен попасть в agent scope");

    let run_rows = mem.db.list(&ScopeFilter::new().add(Scope::Run, ""), Some("active"), 100).unwrap();
    assert!(run_rows.iter().any(|r| r.content.contains("bug is annoying")),
        "ephemeral-факт должен быть в run scope");
}

#[tokio::test]
async fn memory_class_expiring_gets_default_90_day_ttl() {
    let mem = in_memory();
    mem.pipeline().absorb(req(fact(
        "The quarterly report deadline is 2026-09-30", Some("expiring")))).await.unwrap();

    let rows = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let row = rows.iter().find(|r| r.content.contains("quarterly")).unwrap();
    assert!(row.expires_at.is_some(), "expiring-факт должен получить expires_at");

    let exp = chrono::DateTime::parse_from_rfc3339(row.expires_at.as_deref().unwrap()).unwrap();
    let now = chrono::Utc::now();
    let days = (exp.with_timezone(&chrono::Utc) - now).num_days();
    assert!((85..=95).contains(&days), "TTL по умолчанию ~90 дней, получено {days}");
}

#[tokio::test]
async fn memory_class_durable_is_default_no_ttl() {
    let mem = in_memory();
    mem.pipeline().absorb(req(fact(
        "The user prefers terse code reviews", None))).await.unwrap();

    let rows = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    let row = rows.iter().find(|r| r.content.contains("terse")).unwrap();
    assert!(row.expires_at.is_none(), "durable-факт не должен иметь TTL");
    assert_eq!(row.scope, "agent", "durable по умолчанию в agent scope");
}

// ── Скоринг поиска: confidence + reinforcement ─────────────────────────────

async fn seed_twin_rows(mem: &Memory, content: &str, conf_a: f64, conf_b: f64) {
    for conf in [conf_a, conf_b] {
        let row = raw_row(mem, content, conf, 0);
        let vec = mem.embedder.embed(&[content.to_string()]).await.unwrap().remove(0);
        mem.db.put_embedding(&row.id, mem.embedder.model_name(), &vec).unwrap();
    }
}

#[tokio::test]
async fn scoring_higher_confidence_ranks_higher() {
    let mem = in_memory();
    let content = "Acme Corp uses Kubernetes for production deployments";
    seed_twin_rows(&mem, content, 0.9, 0.4).await;

    let hits = mem.search("kubernetes production", &ScopeFilter::persistent(), None).await.unwrap();
    assert_eq!(hits.len(), 2, "оба близнеца должны попасть в выдачу");
    assert!(hits[0].memory.confidence > hits[1].memory.confidence,
        "высокая confidence должна ранжироваться выше: [{:.3}/{:.3}]",
        hits[0].score, hits[1].score);
}

#[tokio::test]
async fn scoring_more_accesses_ranks_higher() {
    let mem = in_memory();
    let content = "Redis is used as a cache in the product";
    let a = raw_row(&mem, content, 0.8, 12); // reinforcement = min(12/10, 1) = 1.0
    let _b = raw_row(&mem, content, 0.8, 0); // reinforcement = 0
    let vec = mem.embedder.embed(&[content.to_string()]).await.unwrap().remove(0);
    mem.db.put_embedding(&a.id, mem.embedder.model_name(), &vec).unwrap();
    for row in mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap() {
        mem.db.put_embedding(&row.id, mem.embedder.model_name(), &vec).unwrap();
    }

    let hits = mem.search("redis cache", &ScopeFilter::persistent(), None).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].memory.id, a.id,
        "часто читаемая память (access_count=12) должна быть первой");
}

// ── GC: decay-сопротивление и updated_at ───────────────────────────────────

#[tokio::test]
async fn gc_decay_resistance_keeps_frequently_accessed() {
    let mem = in_memory();
    let old = (chrono::Utc::now() - chrono::Duration::days(200)).to_rfc3339();

    // Оба факта: 200 дней, confidence 0.2. Разница — число обращений:
    // 16 обращений → resistance = min(16·0.05, 0.8) = 0.8 →
    // effective = 0.02·(1−0.8)·(200/30) ≈ 0.027 → 0.173 > 0.15 → выживает;
    // 0 обращений → effective = 0.02·1·(200/30) ≈ 0.133 → 0.067 → архивируется.
    let mk = |content: &str, access: i64| MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_string(),
        metadata: json!({}),
        tags: vec![],
        source: "unit".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: 0.2,
        importance: 1.0,
        access_count: access,
        last_accessed: None,
        status: "active".into(),
        expires_at: None,
        content_hash: content_hash(content),
        created_at: old.clone(),
        updated_at: old.clone(),
    };
    let accessed = mk("frequently recalled fact", 16);
    let untouched = mk("never recalled fact", 0);
    mem.db.insert(&accessed).unwrap();
    mem.db.insert(&untouched).unwrap();

    let report = mem.gc(&GcOptions::default()).await.unwrap();
    println!("archived={} decayed={}", report.confidence_archived, report.confidence_decayed);

    assert_eq!(mem.db.get(&untouched.id).unwrap().unwrap().status, "archived",
        "нетронутый факт (0 обращений) должен быть архивирован");
    assert_eq!(mem.db.get(&accessed.id).unwrap().unwrap().status, "active",
        "часто читаемый факт должен пережить decay");
    let conf = mem.db.get(&accessed.id).unwrap().unwrap().confidence;
    assert!(conf > 0.15 && conf < 0.2, "confidence должен снизиться, но не ниже порога: {conf}");
}

#[tokio::test]
async fn gc_update_confidence_preserves_updated_at() {
    let mem = in_memory();
    let row = raw_row(&mem, "timestamp stability probe", 0.8, 0);
    let before = mem.db.get(&row.id).unwrap().unwrap().updated_at;
    mem.db.update_confidence(&row.id, 0.5).unwrap();
    let after = mem.db.get(&row.id).unwrap().unwrap().updated_at;
    assert_eq!(before, after,
        "update_confidence не должен трогать updated_at (иначе decay сбросит idle-счётчик)");
}

// ── merge_into ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn merge_into_unions_tags_and_takes_max_confidence() {
    let mem = in_memory();
    let mut row = raw_row(&mem, "original content", 0.7, 0);
    // Пересоздаём с тегом (вставка по тому же id заменяет строку).
    mem.db.delete(&row.id).unwrap();
    row.tags = vec!["alpha".into()];
    mem.db.insert(&row).unwrap();

    let ok = mem.db
        .merge_into(&row.id, "additional content", &["beta".to_string()], 0.9)
        .unwrap();
    assert!(ok);

    let merged = mem.db.get(&row.id).unwrap().unwrap();
    assert!(merged.content.contains("original") && merged.content.contains("additional"),
        "контент объединён: {}", merged.content);
    assert!(merged.tags.contains(&"alpha".to_string()) && merged.tags.contains(&"beta".to_string()),
        "теги объединены: {:?}", merged.tags);
    assert_eq!(merged.confidence, 0.9, "confidence берётся по максимуму");
}
