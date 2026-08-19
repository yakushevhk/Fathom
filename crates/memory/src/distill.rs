//! Session distillation (openclaude `/dream` pattern).
//!
//! Run-scoped episode facts accumulate during sessions; most of them are
//! noise next week. Distillation promotes the durable ones into persistent
//! agent knowledge (re-running the full absorb pipeline, so dedup and
//! supersession apply) and archives the run-scoped originals. Nothing is
//! deleted — archived rows stay queryable by id/follow=full_history.
//!
//! Designed to run as a background job (`parallel-research memory distill`
//! or a scheduled durable job).

use crate::absorb::{AbsorbFact, AbsorbRequest};
use crate::db::{MemoryRow, Scope, ScopeFilter};
use crate::Memory;
use serde::{Deserialize, Serialize};

/// Outcome of one distillation pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DistillReport {
    /// Run facts promoted into agent scope (new knowledge).
    pub promoted: usize,
    /// Run facts already known in agent scope (absorb skipped them).
    pub skipped: usize,
    /// Run facts archived after processing (promoted or skipped).
    pub archived: usize,
    /// Facts that failed to process (kept as-is).
    pub errors: usize,
    pub dry_run: bool,
}

impl DistillReport {
    pub fn summary_line(&self) -> String {
        format!(
            "{} promoted, {} already known, {} archived, {} errors{}",
            self.promoted,
            self.skipped,
            self.archived,
            self.errors,
            if self.dry_run { " (dry run)" } else { "" }
        )
    }
}

impl Memory {
    /// Distill run-scoped facts into persistent agent knowledge.
    ///
    /// `session_key` limits the pass to one session (`run` scope_key);
    /// `None` distils every run-scoped fact. Facts are absorbed one by one
    /// so each outcome can be attributed to its source row (distillation is
    /// an offline job — the extra pipeline calls are acceptable).
    pub async fn distill(
        &self,
        session_key: Option<&str>,
        dry_run: bool,
    ) -> anyhow::Result<DistillReport> {
        let mut report = DistillReport {
            dry_run,
            ..Default::default()
        };

        let filter = ScopeFilter {
            pairs: vec![("run".to_string(), session_key.unwrap_or("").to_string())],
        };
        let candidates: Vec<MemoryRow> = self
            .db
            .list(&filter, Some("active"), usize::MAX)?
            .into_iter()
            .filter(|r| !r.is_expired())
            .collect();
        if candidates.is_empty() {
            return Ok(report);
        }

        for row in candidates {
            let fact = AbsorbFact {
                content: row.content.clone(),
                metadata: row.metadata.clone(),
                tags: row.tags.clone(),
                confidence: Some(row.confidence),
                memory_class: None,
            };
            let req = AbsorbRequest {
                facts: vec![fact],
                // Preserve original provenance in the promoted copy.
                source: if row.source.is_empty() {
                    format!("distilled:{}", short(&row.id))
                } else {
                    row.source.clone()
                },
                scope: Scope::Agent,
                scope_key: String::new(),
                context: Some("distillation of session facts".to_string()),
                dry_run,
            };

            match self.pipeline().absorb(req).await {
                Ok(rep) => {
                    let was_duplicate = rep.skipped > 0 && rep.created == 0 && rep.superseded == 0;
                    if was_duplicate {
                        report.skipped += 1;
                    } else {
                        report.promoted += 1;
                    }
                    // Archive the run-scoped original either way: the fact is
                    // either represented in agent scope now or intentionally
                    // dropped as a duplicate.
                    if !dry_run {
                        self.db.set_status(&row.id, "archived")?;
                        self.db.log_history(
                            &row.id,
                            "distilled",
                            Some("run"),
                            Some(if was_duplicate { "duplicate-in-agent-scope" } else { "promoted" }),
                        );
                        report.archived += 1;
                    }
                }
                Err(e) => {
                    tracing::warn!("distill: failed to absorb run fact {}: {e}", row.id);
                    report.errors += 1;
                }
            }
        }

        Ok(report)
    }
}

fn short(id: &str) -> String {
    id.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::absorb::AbsorbFact;
    use pr_core::MemoryConfig;

    async fn store_with_run_fact(fact: &str) -> Memory {
        let mem = Memory::in_memory(MemoryConfig::default()).unwrap();
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: fact.to_string(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.8),
                    memory_class: None,
                }],
                source: "session:s1".into(),
                scope: Scope::Run,
                scope_key: "s1".into(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        mem
    }

    #[tokio::test]
    async fn distill_promotes_and_archives() {
        let mem = store_with_run_fact(
            "verified that the Kazan office employs forty engineers in research",
        )
        .await;

        let report = mem.distill(None, false).await.unwrap();
        assert_eq!(report.promoted, 1);
        assert_eq!(report.archived, 1);
        assert_eq!(report.skipped, 0);

        // The fact is now in agent scope...
        let agent_rows = mem
            .db
            .list(&ScopeFilter::persistent(), Some("active"), 10)
            .unwrap();
        assert_eq!(agent_rows.len(), 1);
        assert!(agent_rows[0].content.contains("Kazan office"));
        // ...and the run-scoped original is archived.
        let run_rows = mem
            .db
            .list(
                &ScopeFilter {
                    pairs: vec![("run".into(), String::new())],
                },
                Some("archived"),
                10,
            )
            .unwrap();
        assert_eq!(run_rows.len(), 1);
    }

    #[tokio::test]
    async fn distill_twice_is_idempotent() {
        let mem = store_with_run_fact(
            "acme llc signed a distribution agreement with beta partners group",
        )
        .await;
        let first = mem.distill(None, false).await.unwrap();
        assert_eq!(first.promoted, 1);
        let second = mem.distill(None, false).await.unwrap();
        assert_eq!(second.promoted + second.skipped, 0, "nothing left to distill");
    }

    #[tokio::test]
    async fn distill_dry_run_touches_nothing() {
        let mem = store_with_run_fact(
            "the pilot project finished ahead of schedule in q3 according to the pm",
        )
        .await;
        let report = mem.distill(None, true).await.unwrap();
        assert_eq!(report.promoted, 1);
        assert!(report.dry_run);
        assert_eq!(report.archived, 0);
        // Run fact still active, agent scope still empty.
        let run_rows = mem
            .db
            .list(
                &ScopeFilter {
                    pairs: vec![("run".into(), String::new())],
                },
                Some("active"),
                10,
            )
            .unwrap();
        assert_eq!(run_rows.len(), 1);
        assert!(mem
            .db
            .list(&ScopeFilter::persistent(), Some("active"), 10)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn distill_respects_session_key() {
        let mem = Memory::in_memory(MemoryConfig::default()).unwrap();
        for (sess, fact) in [
            ("s1", "first session learned about the alpha vendor pricing model"),
            ("s2", "second session mapped the beta supplier delivery schedule"),
        ] {
            mem.pipeline()
                .absorb(AbsorbRequest {
                    facts: vec![AbsorbFact {
                        content: fact.to_string(),
                        metadata: serde_json::json!({}),
                        tags: vec![],
                        confidence: Some(0.8),
                        memory_class: None,
                    }],
                    source: format!("session:{sess}"),
                    scope: Scope::Run,
                    scope_key: sess.into(),
                    context: None,
                    dry_run: false,
                })
                .await
                .unwrap();
        }

        let report = mem.distill(Some("s1"), false).await.unwrap();
        assert_eq!(report.archived, 1);
        // s2 fact is still active.
        let s2_rows = mem
            .db
            .list(
                &ScopeFilter {
                    pairs: vec![("run".into(), "s2".into())],
                },
                Some("active"),
                10,
            )
            .unwrap();
        assert_eq!(s2_rows.len(), 1);
    }

    #[tokio::test]
    async fn distill_skips_duplicates_already_in_agent_scope() {
        let mem = Memory::in_memory(MemoryConfig::default()).unwrap();
        let fact = "the warehouse relocation to novosibirsk completed in march";
        // Same fact already promoted earlier (agent scope)...
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: fact.to_string(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.9),
                    memory_class: None,
                }],
                source: "old-session".into(),
                scope: Scope::Agent,
                scope_key: String::new(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();
        // ...and present again as a run fact.
        mem.pipeline()
            .absorb(AbsorbRequest {
                facts: vec![AbsorbFact {
                    content: fact.to_string(),
                    metadata: serde_json::json!({}),
                    tags: vec![],
                    confidence: Some(0.8),
                    memory_class: None,
                }],
                source: "session:s9".into(),
                scope: Scope::Run,
                scope_key: "s9".into(),
                context: None,
                dry_run: false,
            })
            .await
            .unwrap();

        let report = mem.distill(None, false).await.unwrap();
        assert_eq!(report.skipped, 1, "hash dedup must flag the known fact");
        assert_eq!(report.promoted, 0);
        assert_eq!(report.archived, 1, "duplicate run fact is archived too");
    }
}
