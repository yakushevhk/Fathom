//! Memory garbage collection: TTL archiving and N→1 compaction.
//!
//! Long-lived stores accumulate noise: expired facts, run-scoped episode
//! details nobody ever recalled, and scope groups that grow past a useful
//! size. GC is a conservative offline pass (like [`crate::distill`]) that
//! only *archives* — nothing is deleted, so every row stays queryable via
//! `follow=full_history`.
//!
//! Three stages:
//! 1. **expired** — active rows whose `expires_at` is in the past are
//!    archived;
//! 2. **stale** — `run`-scoped active rows older than the TTL that were
//!    never accessed and never boosted are archived (distillation is the
//!    intended path for valuable run facts; whatever is left is noise);
//! 3. **compaction** — a `(scope, scope_key)` group with more than
//!    `compact_above` active rows has its oldest/least-important surplus
//!    merged into one consolidated memory (bulleted summary), and the
//!    originals are archived. The store shrinks N→1 without losing content,
//!    keeping digests and searches fast.

use crate::db::{content_hash, MemoryRow, Scope, ScopeFilter};
use crate::Memory;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Knobs for one GC pass.
#[derive(Debug, Clone)]
pub struct GcOptions {
    /// Archive untouched run-scoped facts older than this many days.
    pub ttl_days: u32,
    /// Compact a scope group when it holds more than this many active rows.
    pub compact_above: usize,
    /// Maximum rows merged into one consolidated memory per group per pass.
    pub compact_batch: usize,
    /// Daily confidence decay rate (0.02 = 2% per 30 idle days).
    pub confidence_decay_rate: f64,
    /// Archive memories whose confidence drops below this threshold.
    pub confidence_threshold: f64,
    pub dry_run: bool,
}

impl Default for GcOptions {
    fn default() -> Self {
        Self {
            ttl_days: 30,
            compact_above: 200,
            compact_batch: 100,
            confidence_decay_rate: 0.02,
            confidence_threshold: 0.15,
            dry_run: false,
        }
    }
}

/// Outcome of one GC pass.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GcReport {
    /// Active rows archived because their `expires_at` passed.
    pub expired_archived: usize,
    /// Run-scoped rows archived because they are older than the TTL and
    /// were never accessed/boosted.
    pub stale_archived: usize,
    /// Active rows archived because confidence decayed below threshold.
    pub confidence_archived: usize,
    /// Active rows whose confidence was reduced by decay.
    pub confidence_decayed: usize,
    /// Scope groups that were compacted N→1.
    pub groups_compacted: usize,
    /// Original rows archived by compaction (across all groups).
    pub facts_compacted: usize,
    pub errors: usize,
    pub dry_run: bool,
}

impl GcReport {
    pub fn summary_line(&self) -> String {
        format!(
            "{} expired, {} stale, {} confidence archived ({} decayed); {} group(s) compacted ({} facts merged){}",
            self.expired_archived,
            self.stale_archived,
            self.confidence_archived,
            self.confidence_decayed,
            self.groups_compacted,
            self.facts_compacted,
            if self.dry_run { " (dry run)" } else { "" }
        )
    }

    pub fn touched_anything(&self) -> bool {
        self.expired_archived + self.stale_archived + self.confidence_archived + self.facts_compacted > 0
    }
}

impl Memory {
    /// Run a GC pass with the configured defaults overridden by `opts`.
    pub async fn gc(&self, opts: &GcOptions) -> anyhow::Result<GcReport> {
        let mut report = GcReport {
            dry_run: opts.dry_run,
            ..Default::default()
        };
        let now = Utc::now();
        let ttl_cutoff = now - chrono::Duration::days(opts.ttl_days as i64);

        // Collect all active rows once; stages below only archive, so the
        // snapshot stays consistent enough for an offline maintenance pass.
        let mut active: Vec<MemoryRow> = Vec::new();
        for scope in [Scope::User, Scope::Agent, Scope::Run] {
            let filter = ScopeFilter::new().add(scope, "");
            active.extend(self.db.list(&filter, Some("active"), usize::MAX)?);
        }

        // ── Stage 1: expired ────────────────────────────────────────────────
        let mut surviving: Vec<MemoryRow> = Vec::with_capacity(active.len());
        for row in active {
            if row.is_expired() {
                self.archive(&row.id, "gc-expired", &mut report.errors, opts.dry_run)?;
                report.expired_archived += 1;
            } else {
                surviving.push(row);
            }
        }

        // ── Stage 2: stale run facts ────────────────────────────────────────
        let mut kept: Vec<MemoryRow> = Vec::with_capacity(surviving.len());
        for row in surviving {
            let stale = row.scope == "run"
                && row.access_count == 0
                && row.importance < 0.75
                && parsed_ts(&row.updated_at).map(|t| t < ttl_cutoff).unwrap_or(false);
            if stale {
                self.archive(&row.id, "gc-stale", &mut report.errors, opts.dry_run)?;
                report.stale_archived += 1;
            } else {
                kept.push(row);
            }
        }

        // ── Stage 2.5: confidence decay ────────────────────────────────────
        // Memories that haven't been accessed lose confidence over time.
        // Frequently-accessed memories resist decay (the "reinforcement"
        // concept from memory engineering).  Low-confidence survivors
        // are archived — they stop competing for retrieval space.
        let mut post_decay: Vec<MemoryRow> = Vec::with_capacity(kept.len());
        for row in kept {
            let last_str = row.last_accessed.as_deref().unwrap_or(&row.updated_at);
            let days_idle = parsed_ts(last_str)
                .map(|t| (now - t).num_seconds().max(0) as f64 / 86_400.0)
                .unwrap_or(0.0);

            // Frequently-accessed memories resist decay (max 80% resistance).
            let resistance = (row.access_count as f64 * 0.05).min(0.8);
            let effective = opts.confidence_decay_rate * (1.0 - resistance) * (days_idle / 30.0);
            let new_conf = (row.confidence - effective).max(0.0);

            if new_conf < opts.confidence_threshold {
                self.archive(&row.id, "gc-confidence-decay", &mut report.errors, opts.dry_run)?;
                report.confidence_archived += 1;
            } else if (new_conf - row.confidence).abs() > 1e-9 {
                if !opts.dry_run {
                    self.db.update_confidence(&row.id, new_conf)?;
                }
                report.confidence_decayed += 1;
                post_decay.push(MemoryRow { confidence: new_conf, ..row });
            } else {
                post_decay.push(row);
            }
        }

        // ── Stage 3: N→1 compaction per scope group ─────────────────────────
        let mut groups: std::collections::HashMap<(String, String), Vec<MemoryRow>> =
            std::collections::HashMap::new();
        for row in post_decay {
            groups
                .entry((row.scope.clone(), row.scope_key.clone()))
                .or_default()
                .push(row);
        }
        for ((scope, scope_key), mut rows) in groups {
            if rows.len() <= opts.compact_above {
                continue;
            }
            // Merge the cheapest surplus: low importance first, oldest first.
            rows.sort_by(|a, b| {
                a.importance
                    .partial_cmp(&b.importance)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.updated_at.cmp(&b.updated_at))
            });
            let surplus = (rows.len() - opts.compact_above).min(opts.compact_batch);
            if surplus < 2 {
                continue;
            }
            let victims: Vec<MemoryRow> = rows.drain(..surplus).collect();
            match self.compact_group(&scope, &scope_key, &victims, opts.dry_run) {
                Ok(consolidated_id) => {
                    report.groups_compacted += 1;
                    report.facts_compacted += victims.len();
                    if let Some(id) = consolidated_id {
                        // Provenance edges back to the archived originals
                        // (best effort — edges are decorative here).
                        for v in &victims {
                            let _ = self.db.add_edge(&id, &v.id, "references", Some("gc compaction"));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("gc: compaction of {scope}:{scope_key} failed: {e}");
                    report.errors += 1;
                }
            }
        }

        Ok(report)
    }

    fn archive(&self, id: &str, event: &str, errors: &mut usize, dry_run: bool) -> anyhow::Result<()> {
        if dry_run {
            return Ok(());
        }
        match self.db.set_status(id, "archived") {
            Ok(()) => self.db.log_history(id, event, Some("active"), Some("archived")),
            Err(e) => {
                *errors += 1;
                tracing::warn!("gc: failed to archive {id}: {e}");
            }
        }
        Ok(())
    }

    /// Merge `victims` into one consolidated memory; archive the originals.
    /// Returns the new row's id (None in dry-run mode).
    fn compact_group(
        &self,
        scope: &str,
        scope_key: &str,
        victims: &[MemoryRow],
        dry_run: bool,
    ) -> anyhow::Result<Option<String>> {
        let mut lines: Vec<String> = Vec::with_capacity(victims.len());
        let mut total = 0usize;
        for v in victims {
            let trimmed = v.content.trim();
            if trimmed.is_empty() {
                continue;
            }
            let one = truncate_chars(trimmed, 200);
            // Keep consolidated rows bounded even for huge batches.
            if total + one.len() > 8_000 {
                lines.push(format!("… and {} more", victims.len() - lines.len()));
                break;
            }
            total += one.len();
            lines.push(format!("- {one}"));
        }
        let content = format!(
            "Consolidated {} stale facts ({}):\n{}",
            victims.len(),
            Utc::now().to_rfc3339(),
            lines.join("\n")
        );
        if dry_run {
            return Ok(None);
        }

        let confidence = victims
            .iter()
            .map(|v| v.confidence)
            .fold(1.0_f64, f64::min);
        let now = Utc::now().to_rfc3339();
        let row = MemoryRow {
            id: uuid::Uuid::now_v7().to_string(),
            content: content.clone(),
            metadata: serde_json::json!({ "kind": "consolidated", "count": victims.len(), "gc": true }),
            tags: vec!["consolidated".to_string()],
            source: "gc".to_string(),
            scope: scope.to_string(),
            scope_key: scope_key.to_string(),
            confidence,
            importance: 0.5,
            access_count: 0,
            last_accessed: None,
            status: "active".to_string(),
            expires_at: None,
            content_hash: content_hash(&content),
            created_at: now.clone(),
            updated_at: now,
        };
        self.db.insert(&row)?;
        self.db.log_history(&row.id, "gc-consolidated", None, Some(&format!("{}", victims.len())));
        let mut errs = 0usize;
        for v in victims {
            self.archive(&v.id, "gc-compacted", &mut errs, false)?;
        }
        if errs > 0 {
            anyhow::bail!("failed to archive {errs} original row(s) after compaction");
        }
        Ok(Some(row.id))
    }
}

fn parsed_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::MemoryConfig;

    async fn store() -> Memory {
        Memory::in_memory(MemoryConfig::default()).unwrap()
    }

    fn raw_row(mem: &Memory, scope: &str, content: &str, age_days: i64, access: i64, importance: f64) -> MemoryRow {
        let ts = (Utc::now() - chrono::Duration::days(age_days)).to_rfc3339();
        let row = MemoryRow {
            id: uuid::Uuid::now_v7().to_string(),
            content: content.to_string(),
            metadata: serde_json::json!({}),
            tags: vec![],
            source: "test".into(),
            scope: scope.into(),
            scope_key: String::new(),
            confidence: 0.8,
            importance,
            access_count: access,
            last_accessed: None,
            status: "active".into(),
            expires_at: None,
            content_hash: content_hash(content),
            created_at: ts.clone(),
            updated_at: ts,
        };
        mem.db.insert(&row).unwrap();
        row
    }

    #[tokio::test]
    async fn gc_archives_expired_rows() {
        let mem = store().await;
        let mut row = raw_row(&mem, "agent", "expires soon", 0, 0, 1.0);
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        mem.db.delete(&row.id).unwrap();
        row.expires_at = Some(past);
        mem.db.insert(&row).unwrap();

        let report = mem.gc(&GcOptions::default()).await.unwrap();
        assert_eq!(report.expired_archived, 1);
        assert_eq!(mem.db.get(&row.id).unwrap().unwrap().status, "archived");
    }

    #[tokio::test]
    async fn gc_archives_stale_run_facts_only() {
        let mem = store().await;
        let stale_run = raw_row(&mem, "run", "old untouched run fact", 45, 0, 0.5);
        let accessed_run = raw_row(&mem, "run", "old but accessed run fact", 45, 3, 0.5);
        let boosted_run = raw_row(&mem, "run", "old but boosted run fact", 45, 0, 1.0);
        let old_agent = raw_row(&mem, "agent", "old agent knowledge", 90, 0, 0.5);
        let fresh_run = raw_row(&mem, "run", "fresh run fact", 2, 0, 0.5);

        let report = mem.gc(&GcOptions { ttl_days: 30, ..Default::default() }).await.unwrap();
        assert_eq!(report.stale_archived, 1, "only the untouched old run fact goes");
        assert_eq!(mem.db.get(&stale_run.id).unwrap().unwrap().status, "archived");
        for kept in [&accessed_run, &boosted_run, &old_agent, &fresh_run] {
            assert_eq!(mem.db.get(&kept.id).unwrap().unwrap().status, "active");
        }
    }

    #[tokio::test]
    async fn gc_compacts_oversized_group_n_to_1() {
        let mem = store().await;
        for i in 0..25 {
            raw_row(&mem, "run", &format!("bulk run fact number {i}"), 10, 0, 0.4 + (i as f64) * 0.001);
        }
        let opts = GcOptions {
            ttl_days: 30,
            compact_above: 10,
            compact_batch: 100,
            confidence_decay_rate: 0.02,
            confidence_threshold: 0.15,
            dry_run: false,
        };
        let report = mem.gc(&opts).await.unwrap();
        assert_eq!(report.groups_compacted, 1);
        // 25 rows, keep 10 -> merge the 15 cheapest (age 10d < ttl, but they
        // are accessed=0 & low importance; stage 2 archives them as stale?
        // no: age 10 < ttl 30, so they survive to compaction).
        assert_eq!(report.facts_compacted, 15);

        let filter = ScopeFilter::new().add(Scope::Run, "");
        let active = mem.db.list(&filter, Some("active"), usize::MAX).unwrap();
        assert_eq!(active.len(), 11, "10 survivors + 1 consolidated");
        let consolidated = active.iter().find(|r| r.tags.iter().any(|t| t == "consolidated")).unwrap();
        assert!(consolidated.content.starts_with("Consolidated 15 stale facts"));
        assert!(consolidated.metadata["gc"].as_bool().unwrap_or(false));
    }

    #[tokio::test]
    async fn gc_dry_run_touches_nothing() {
        let mem = store().await;
        raw_row(&mem, "run", "stale candidate", 60, 0, 0.5);
        let report = mem
            .gc(&GcOptions { dry_run: true, ..Default::default() })
            .await
            .unwrap();
        assert_eq!(report.stale_archived, 1);
        let filter = ScopeFilter::new().add(Scope::Run, "");
        assert_eq!(mem.db.list(&filter, Some("active"), usize::MAX).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn gc_decays_confidence_and_archives_low() {
        let mem = store().await;
        // A memory with confidence 0.8, 200 days old, never accessed.
        // effective_decay = 0.02 * (1 - 0) * (200/30) ≈ 0.133
        // new_conf ≈ 0.8 - 0.133 ≈ 0.667 → survives
        let kept_row = raw_row(&mem, "agent", "old but confident", 200, 0, 1.0);
        // A memory with confidence 0.2, 200 days old, never accessed.
        // effective_decay = 0.02 * 1.0 * (200/30) ≈ 0.133
        // new_conf ≈ 0.2 - 0.133 ≈ 0.067 → archived
        let low_row = raw_row(&mem, "agent", "low conf old", 200, 0, 1.0);
        mem.db.update_confidence(&low_row.id, 0.2).unwrap();
        // A memory with confidence 0.2, 200 days old, but accessed 10 times.
        // resistance = min(10 * 0.05, 0.8) = 0.5
        // effective_decay = 0.02 * 0.5 * (200/30) ≈ 0.067
        // new_conf ≈ 0.2 - 0.067 ≈ 0.133 → archived (below 0.15)
        let accessed_low = raw_row(&mem, "agent", "accessed low conf", 200, 10, 1.0);
        mem.db.update_confidence(&accessed_low.id, 0.2).unwrap();

        let report = mem.gc(&GcOptions::default()).await.unwrap();
        assert_eq!(report.confidence_archived, 2, "both low-confidence rows archived");
        assert!(report.confidence_decayed >= 1, "at least one row had confidence reduced");
        assert_eq!(mem.db.get(&kept_row.id).unwrap().unwrap().status, "active");
        assert_eq!(mem.db.get(&low_row.id).unwrap().unwrap().status, "archived");
        assert_eq!(mem.db.get(&accessed_low.id).unwrap().unwrap().status, "archived");
    }
}
