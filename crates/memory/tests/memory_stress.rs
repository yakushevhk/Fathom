//! Стресс и edge cases memory-подсистемы (без сети):
//!   - конкурентные absorb (разные факты, гонки записи)
//!   - конкурентные чтение+запись (поиск во время записи)
//!   - большой batch (50 фактов)
//!   - boost: кламп в 0, несуществующий id
//!   - merge_into / update_confidence на отсутствующем id
//!   - supersede-цепочка из 3: поиск находит только последнюю версию
//!   - теги и metadata сохраняются при absorb
//!   - keyword_search: подстрока/терм

use pr_memory::{
    content_hash, AbsorbFact, AbsorbRequest, Memory, MemoryRow, Scope, ScopeFilter,
};
use serde_json::json;
use std::sync::Arc;

fn in_memory() -> Memory {
    Memory::in_memory(pr_core::MemoryConfig::default()).unwrap()
}

fn clone_mem(mem: &Memory) -> Memory {
    Memory {
        db: mem.db.clone(),
        embedder: mem.embedder.clone(),
        config: mem.config.clone(),
    }
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

fn req_facts(facts: Vec<AbsorbFact>) -> AbsorbRequest {
    AbsorbRequest {
        facts,
        source: "stress".into(),
        scope: Scope::Agent,
        scope_key: String::new(),
        context: None,
        dry_run: false,
    }
}

// ── 1. Конкурентные absorb (16 потоков, разные факты) ─────────────────────

#[tokio::test]
async fn concurrent_absorbs_all_land() {
    let mem = in_memory();
    let mut handles = Vec::new();
    for i in 0..16 {
        let m = clone_mem(&mem);
        handles.push(tokio::spawn(async move {
            // Уникальные токены — иначе консолидация схлопнет похожие факты.
            let req = req_facts(vec![fact(&format!(
                "Observation {i}: zz{i}qw{i}er{i}ty{i} unique padding token"
            ))]);
            m.pipeline().absorb(req).await
        }));
    }
    let mut created = 0;
    for h in handles {
        created += h.await.unwrap().unwrap().created;
    }
    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    assert_eq!(created, 16, "все 16 конкурентных absorb должны создать факт");
    assert_eq!(active.len(), 16, "в сторе 16 активных строк");
}

// ── 2. Конкурентные чтение+запись ─────────────────────────────────────────

#[tokio::test]
async fn concurrent_mixed_reads_and_writes() {
    let mem = in_memory();
    // 8 писателей: 5 фактов каждый.
    let mut handles = Vec::new();
    for w in 0..8 {
        let m = clone_mem(&mem);
        handles.push(tokio::spawn(async move {
            for k in 0..5 {
                let req = req_facts(vec![fact(&format!(
                    "Writer {w} fact {k}: zz{w}kk{k} unique padding token"
                ))]);
                m.pipeline().absorb(req).await.unwrap();
            }
        }));
    }
    // 8 читателей: ищут в цикле, пока пишут.
    for _ in 0..8 {
        let m = clone_mem(&mem);
        handles.push(tokio::spawn(async move {
            for _ in 0..20 {
                let _ = m.search("unique padding", &ScopeFilter::persistent(), Some(3)).await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 100).unwrap();
    assert_eq!(active.len(), 40, "8×5 фактов, без потерь и повреждений");
}

// ── 3. Большой batch: 50 фактов ───────────────────────────────────────────

#[tokio::test]
async fn large_batch_of_50_facts_all_inserted() {
    let mem = in_memory();
    // Уникальные токены на каждый факт, чтобы консолидация их не схлопнула.
    let facts: Vec<AbsorbFact> = (0..50)
        .map(|i| fact(&format!(
            "Observation {i}: zz{i}qw{i}er{i}ty{i} unique padding token"
        )))
        .collect();
    let report = mem.pipeline().absorb(req_facts(facts)).await.unwrap();
    assert_eq!(report.created, 50, "все 50 фактов созданы: {}", report.summary_line());
    let active = mem.db.list(&ScopeFilter::persistent(), Some("active"), 200).unwrap();
    assert_eq!(active.len(), 50);
}

// ── 4. boost: кламп в ноль и несуществующий id ────────────────────────────

#[tokio::test]
async fn boost_clamps_at_zero_and_missing_id_is_false() {
    let mem = in_memory();
    let row = MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: "boost probe".into(),
        metadata: json!({}),
        tags: vec![],
        source: "stress".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: 0.8,
        importance: 0.2,
        access_count: 0,
        last_accessed: None,
        status: "active".into(),
        expires_at: None,
        content_hash: content_hash("boost probe"),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    mem.db.insert(&row).unwrap();

    // -100 при importance 0.2 → кламп в 0.0, не в отрицательные.
    assert!(mem.db.boost(&row.id, -100.0).unwrap());
    assert_eq!(mem.db.get(&row.id).unwrap().unwrap().importance, 0.0);

    // Несуществующий id → false.
    assert!(!mem.db.boost("no-such-id", 1.0).unwrap());
}

// ── 5. merge_into / update_confidence на отсутствующем id ─────────────────

#[tokio::test]
async fn merge_and_confidence_update_missing_id_are_false() {
    let mem = in_memory();
    assert!(!mem.db.merge_into("no-such-id", "x", &[], 0.9).unwrap());
    assert!(!mem.db.update_confidence("no-such-id", 0.5).unwrap());
}

// ── 6. Supersede-цепочка из 3: поиск видит только последнюю ───────────────

#[tokio::test]
async fn supersede_chain_of_three_search_finds_only_latest() {
    let mem = in_memory();
    // v3 идёт через pipeline (получает embedding + FTS); v1/v2 вставляем
    // напрямую как superseded — они не должны попадать в поиск.
    let v3_req = req_facts(vec![fact("The office moved again to 30 Leninsky prospect in Moscow")]);
    mem.pipeline().absorb(v3_req).await.unwrap();

    let now = chrono::Utc::now().to_rfc3339();
    let mk = |content: &str| MemoryRow {
        id: uuid::Uuid::now_v7().to_string(),
        content: content.to_string(),
        metadata: json!({}),
        tags: vec!["office".into()],
        source: "chain".into(),
        scope: "agent".into(),
        scope_key: String::new(),
        confidence: 0.8,
        importance: 1.0,
        access_count: 0,
        last_accessed: None,
        status: "superseded".to_string(),
        expires_at: None,
        content_hash: content_hash(content),
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    let v1 = mk("The office is at 12 Tverskaya street in Moscow");
    let v2 = mk("The office moved to 5 Arbat street in Moscow");
    mem.db.insert(&v1).unwrap();
    mem.db.insert(&v2).unwrap();
    let v3 = mem
        .db
        .list(&ScopeFilter::persistent(), Some("active"), 10)
        .unwrap()
        .remove(0);
    mem.db.add_edge(&v2.id, &v1.id, "supersedes", None).unwrap();
    mem.db.add_edge(&v3.id, &v2.id, "supersedes", None).unwrap();

    let hits = mem.search("office moscow street", &ScopeFilter::persistent(), Some(10)).await.unwrap();
    println!("хитов: {}", hits.len());
    for h in &hits {
        println!("   [{:.3}] {}", h.score, h.memory.content);
    }
    // Активный поиск исключает superseded — виден только v3.
    assert!(!hits.is_empty(), "последняя версия должна находиться");
    assert!(hits.iter().all(|h| h.memory.content.contains("Leninsky")),
        "поиск должен возвращать только последнюю версию офиса");
}

// ── 7. Теги и metadata сохраняются при absorb ─────────────────────────────

#[tokio::test]
async fn absorb_preserves_tags_and_metadata() {
    let mem = in_memory();
    let mut f = fact("The company uses Kafka for event streaming");
    f.tags = vec!["architecture".into(), "kafka".into()];
    f.metadata = json!({"source_url": "https://acme.dev/stack", "verified": true});
    let req = req_facts(vec![f]);
    let report = mem.pipeline().absorb(req).await.unwrap();
    assert_eq!(report.created, 1);

    let rows = mem.db.list(&ScopeFilter::persistent(), Some("active"), 10).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].tags.contains(&"architecture".to_string()));
    assert!(rows[0].tags.contains(&"kafka".to_string()));
    assert_eq!(rows[0].metadata["source_url"], "https://acme.dev/stack");
    assert_eq!(rows[0].metadata["verified"], true);
}

// ── 8. keyword_search: терм и подстрока ────────────────────────────────────

#[tokio::test]
async fn keyword_search_finds_terms_and_substrings() {
    let mem = in_memory();
    let req = req_facts(vec![
        fact("PostgreSQL 17 improves logical replication performance"),
        fact("Redis Streams provide durable message queues"),
    ]);
    mem.pipeline().absorb(req).await.unwrap();

    let hits = mem
        .db
        .keyword_search("postgresql replication", &ScopeFilter::persistent(), 10)
        .unwrap();
    assert!(!hits.is_empty(), "keyword_search должен найти postgresql факт");
    assert!(hits.iter().any(|(id, _)| {
        mem.db.get(id).unwrap().map(|r| r.content.contains("PostgreSQL")).unwrap_or(false)
    }));
}

// ── 9. Absorb-отказ: супер-короткий и супер-длинный факт ──────────────────

#[tokio::test]
async fn absorb_rejects_extreme_lengths() {
    let mem = in_memory();
    let report = mem.pipeline().absorb(req_facts(vec![
        fact("ab"),
        fact(&"x".repeat(6000)),
        fact("нормальный факт про PostgreSQL"),
    ])).await.unwrap();
    assert_eq!(report.rejected, 2, "короткий и слишком длинный отклонены");
    assert_eq!(report.created, 1, "нормальный создан");
}
