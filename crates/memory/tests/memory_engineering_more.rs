//! Дополнительные юнит-тесты направлений memory-подсистемы (без сети):
//!   - batch-консолидация N→1 внутри одного absorb
//!   - digest: релевантное + TODO + недавнее
//!   - граф сущностей: ingest из metadata + multi-hop
//!   - кириллический поиск (TF-IDF токенизатор)
//!   - follow-режимы (latest / full_history) после supersede
//!   - edge cases: пустой стор, batch со смешанными вердиктами

use pr_memory::{
    content_hash, AbsorbFact, AbsorbRequest, Follow, Memory, MemoryRow, Scope, ScopeFilter,
};
use serde_json::json;

fn in_memory() -> Memory {
    Memory::in_memory(pr_core::MemoryConfig::default()).unwrap()
}

fn fact(content: &str) -> AbsorbFact {
    AbsorbFact {
        content: content.into(),
        metadata: json!({}),
        tags: vec![],
        confidence: None,
        memory_class: None,
    }
}

fn fact_with_meta(content: &str, metadata: serde_json::Value) -> AbsorbFact {
    AbsorbFact {
        content: content.into(),
        metadata,
        tags: vec![],
        confidence: None,
        memory_class: None,
    }
}

// ── 1. Batch-консолидация N→1 внутри одного absorb ────────────────────────

#[tokio::test]
async fn batch_absorb_consolidates_near_duplicates() {
    let mem = in_memory();
    let req = AbsorbRequest {
        facts: vec![
            fact("Acme Corp is based in Moscow and develops CRM software"),
            fact("Acme Corp is based in Moscow and develops CRM and ERP software"),
            fact("Acme Corp is based in Moscow and develops CRM software for banks"),
        ],
        source: "batch".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let report = mem.pipeline().absorb(req).await.unwrap();
    println!("created={} consolidated={} skipped={}", report.created, report.consolidated, report.skipped);
    assert!(report.consolidated >= 2, "близкие факты батча должны консолидироваться N→1");

    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    assert!(active.len() < 3, "после консолидации строк меньше, чем фактов: {}", active.len());
    assert!(!active.is_empty());
}

// ── 2. Digest: релевантное + TODO + недавнее ───────────────────────────────

#[tokio::test]
async fn digest_collects_todos_recent_and_relevant() {
    let mem = in_memory();
    let mut todo = fact("Review pull request #42 about auth middleware");
    todo.metadata = json!({"type": "todo"});
    let mut done = fact("Ship the v1.3 release");
    done.metadata = json!({"type": "todo", "status": "done"});
    let req = AbsorbRequest {
        facts: vec![
            todo,
            done,
            fact("Auth middleware uses JWT with 15-minute expiry"),
            fact("Team uses Rust for backend services"),
        ],
        source: "digest".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    let digest = mem
        .digest("auth middleware jwt", &ScopeFilter::persistent())
        .await
        .unwrap();
    println!("relevant={} todos={} recent={}", digest.relevant.len(), digest.open_todos.len(), digest.recent.len());

    assert!(!digest.relevant.is_empty(), "должно быть релевантное по запросу");
    assert_eq!(digest.open_todos.len(), 1, "только открытый TODO (done — исключён)");
    assert!(digest.open_todos[0].content.contains("Review pull request"),
        "TODO должен быть про review PR #42");
    assert!(digest.recent.len() >= 3, "recent собирает последние записи");
}

// ── 3. Граф сущностей: ingest + multi-hop ─────────────────────────────────

#[tokio::test]
async fn entity_graph_multihop_from_absorbed_facts() {
    let mem = in_memory();
    let f = fact_with_meta(
        "Ivan Petrov works at Acme Corp which is based in Moscow",
        json!({
            "entities": [
                {"name": "Ivan Petrov", "type": "person"},
                {"name": "Acme Corp", "type": "company"},
                {"name": "Moscow", "type": "location"}
            ],
            "relations": [
                {"from": "Ivan Petrov", "to": "Acme Corp", "relation": "works_at"},
                {"from": "Acme Corp", "to": "Moscow", "relation": "located_in"}
            ]
        }),
    );
    let req = AbsorbRequest {
        facts: vec![f],
        source: "graph".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    let start = mem.db.entity_by_name("Ivan Petrov", "person").unwrap().expect("person exists");
    let paths = pr_memory::graph::multi_hop(&mem.db, &start, 2).unwrap();
    println!("paths from Ivan Petrov: {}", paths.len());
    for p in &paths {
        let names: Vec<&str> = p.nodes.iter().map(|n| n.name.as_str()).collect();
        println!("   {} via {:?}", names.join(" → "), p.relations);
    }
    assert!(
        paths.iter().any(|p| p.nodes.iter().any(|n| n.name == "Moscow")),
        "multi-hop должен дойти от Ivan Petrov до Moscow через Acme Corp"
    );
}

// ── 4. Кириллический поиск ─────────────────────────────────────────────────

#[tokio::test]
async fn cyrillic_facts_are_searchable() {
    let mem = in_memory();
    let req = AbsorbRequest {
        facts: vec![
            fact("Компания Акме разрабатывает CRM-системы для банков в Москве"),
            fact("Ruby on Rails используется для быстрой разработки веб-приложений"),
        ],
        source: "cyr".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    let hits = mem.search("московская компания CRM банки", &ScopeFilter::persistent(), None).await.unwrap();
    println!("hits: {}", hits.len());
    assert!(!hits.is_empty(), "кириллический запрос должен находить русский факт");
    assert!(hits[0].memory.content.contains("Акме"),
        "первым должен быть русский факт про Акме, получено: {}", hits[0].memory.content);
}

// ── 5. Edge case: пустой стор ──────────────────────────────────────────────

#[tokio::test]
async fn empty_store_search_and_digest_are_safe() {
    let mem = in_memory();
    let hits = mem.search("anything", &ScopeFilter::persistent(), None).await.unwrap();
    assert!(hits.is_empty(), "пустой стор → пустая выдача");

    let digest = mem.digest("anything", &ScopeFilter::persistent()).await.unwrap();
    assert!(digest.relevant.is_empty() && digest.open_todos.is_empty() && digest.recent.is_empty());

    let block = mem.digest_block("anything", &ScopeFilter::persistent(), 1000).await;
    assert!(block.is_empty(), "digest_block пустого стора возвращает пустую строку");
}

// ── 6. Follow-режимы после supersede-цепочки ───────────────────────────────

#[tokio::test]
async fn follow_latest_and_full_history_resolve_chains() {
    let mem = in_memory();
    let now = chrono::Utc::now().to_rfc3339();
    let mk = |content: &str, status: &str| MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_string(),
        metadata: json!({}),
        tags: vec![],
        source: "follow".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: 0.8,
        importance: 1.0,
        access_count: 0,
        last_accessed: None,
        status: status.to_string(),
        expires_at: None,
        content_hash: content_hash(content),
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let v1 = mk("CEO is Alice", "superseded");
    let v2 = mk("CEO is Bob", "superseded");
    let v3 = mk("CEO is Carol", "active");
    mem.db.insert(&v1).unwrap();
    mem.db.insert(&v2).unwrap();
    mem.db.insert(&v3).unwrap();
    // v3 supersedes v2, v2 supersedes v1.
    mem.db.add_edge(&v3.id, &v2.id, "supersedes", None).unwrap();
    mem.db.add_edge(&v2.id, &v1.id, "supersedes", None).unwrap();

    let latest = pr_memory::search::resolve_follow(&mem.db, &v1.id, Follow::Latest).unwrap();
    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].id, v3.id, "Latest должен дойти до конца цепочки");

    let history = pr_memory::search::resolve_follow(&mem.db, &v3.id, Follow::FullHistory).unwrap();
    let ids: Vec<&str> = history.iter().map(|r| r.id.as_str()).collect();
    println!("history: {ids:?}");
    assert!(ids.contains(&v1.id.as_str()) && ids.contains(&v2.id.as_str()) && ids.contains(&v3.id.as_str()),
        "FullHistory должен вернуть все три версии");

    let active = pr_memory::search::resolve_follow(&mem.db, &v1.id, Follow::Active).unwrap();
    assert!(active.is_empty(), "Active не возвращает superseded-строку");
}

// ── 7. Batch со смешанными вердиктами (эвристика, без LLM) ─────────────────

#[tokio::test]
async fn batch_mixed_verdicts_reported_separately() {
    let mem = in_memory();
    let seed = AbsorbRequest {
        facts: vec![fact("The office is at 12 Tverskaya street in Moscow")],
        source: "mix".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(seed).await.unwrap();

    let req = AbsorbRequest {
        facts: vec![
            fact("The office is at 12 Tverskaya street in Moscow"), // точный дубль → skip
            fact("PostgreSQL 17 is faster than v16 for analytics"), // новое → create
        ],
        source: "mix".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    let report = mem.pipeline().absorb(req).await.unwrap();
    println!("created={} skipped={}", report.created, report.skipped);
    assert_eq!(report.skipped, 1, "точный дубль должен быть пропущен");
    assert_eq!(report.created, 1, "новый факт создан");
}

// ── 8. expires_at напрямую из metadata (не через memory_class) ────────────

#[tokio::test]
async fn absorb_metadata_expires_at_directly() {
    let mem = in_memory();
    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let mut f = fact("temporary fact already expired");
    f.metadata = json!({"expires_at": past});
    let req = AbsorbRequest {
        facts: vec![f],
        source: "exp".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    // Поиск исключает истёкший факт.
    let hits = mem.search("temporary fact", &ScopeFilter::persistent(), None).await.unwrap();
    assert!(hits.is_empty(), "истёкший факт не должен находиться поиском");

    // GC архивирует его.
    let report = mem.gc(&pr_memory::GcOptions::default()).await.unwrap();
    assert_eq!(report.expired_archived, 1, "GC должен заархивировать истёкший: {}", report.summary_line());
}

// ── 9. Digest исключает истёкшие факты ─────────────────────────────────────

#[tokio::test]
async fn digest_excludes_expired_facts() {
    let mem = in_memory();
    let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    let mut expired = fact("expired detail about quarterly goals");
    expired.metadata = json!({"expires_at": past});
    let req = AbsorbRequest {
        facts: vec![expired, fact("current detail about quarterly goals")],
        source: "dig".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    };
    mem.pipeline().absorb(req).await.unwrap();

    let digest = mem.digest("quarterly goals", &ScopeFilter::persistent()).await.unwrap();
    let contents: Vec<&str> = digest.relevant.iter().map(|h| h.memory.content.as_str()).collect();
    println!("relevant: {contents:?}");
    assert!(contents.iter().any(|c| c.contains("current detail")), "свежий факт в дайджесте");
    assert!(!contents.iter().any(|c| c.contains("expired detail")), "истёкший факт исключён");
}

// ── 10. Stale-embedding после merge: rebuild чинит поиск ───────────────────

#[tokio::test]
async fn rebuild_embeddings_after_merge_fixes_stale_vector() {
    let mem = in_memory();
    // A: факт без слова ERP. B: merge-дополнение с новым словом ERP.
    mem.pipeline().absorb(AbsorbRequest {
        facts: vec![fact("The company Acme Corp is based in Moscow and develops CRM software")],
        source: "stale".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }).await.unwrap();
    let r2 = mem.pipeline().absorb(AbsorbRequest {
        facts: vec![fact("The company Acme Corp is based in Moscow and develops CRM and ERP software")],
        source: "stale".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }).await.unwrap();
    println!("merge result: {}", r2.summary_line());

    // До rebuild: эмбеддинг строки всё ещё от старого контента (без ERP).
    let before = mem.search("ERP software", &ScopeFilter::persistent(), None).await.unwrap();
    println!("поиск ERP до rebuild: {} хитов", before.len());

    // После rebuild: эмбеддинг пересчитан с объединённым контентом.
    let rebuilt = mem.rebuild_embeddings().await.unwrap();
    println!("rebuilt: {rebuilt}");
    let after = mem.search("ERP software", &ScopeFilter::persistent(), None).await.unwrap();
    println!("поиск ERP после rebuild: {} хитов", after.len());
    assert!(!after.is_empty(), "после rebuild поиск ERP должен находить объединённую строку");
    assert!(after[0].memory.content.contains("ERP"), "найдена строка с ERP");
}

// ── 11. Importance — тай-брейкер при равных скорах ─────────────────────────

#[tokio::test]
async fn search_importance_breaks_ties() {
    let mem = in_memory();
    let content = "The team uses Rust for backend services";
    // Две идентичные строки с разной важностью.
    let now = chrono::Utc::now().to_rfc3339();
    let mk = |importance: f64| pr_memory::MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_string(),
        metadata: json!({}),
        tags: vec![],
        source: "tie".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: 0.8,
        importance,
        access_count: 0,
        last_accessed: None,
        status: "active".into(),
        expires_at: None,
        content_hash: content_hash(content),
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let low = mk(0.5);
    let high = mk(2.0);
    mem.db.insert(&low).unwrap();
    mem.db.insert(&high).unwrap();
    let vec = mem.embedder.embed(&[content.to_string()]).await.unwrap().remove(0);
    for row in mem.db.list(&ScopeFilter::persistent(), Some("active"), 10).unwrap() {
        mem.db.put_embedding(&row.id, mem.embedder.model_name(), &vec).unwrap();
    }

    let hits = mem.search("Rust backend", &ScopeFilter::persistent(), None).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].memory.id, high.id,
        "при равных скорах важность решает: первым должен быть importance=2.0");
}
