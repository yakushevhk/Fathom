//! Session resume support.
//!
//! When the process crashes or is killed, sessions remain in the database
//! with status `running`. [`SessionResumer`] finds those interrupted sessions
//! and reconstructs enough state ([`ResumeState`]) for the coordinator to
//! continue where it left off: already-completed agent outputs are recovered,
//! and unfinished agent tasks are collected for re-execution.

use pr_core::{AgentId, SessionId};
use pr_persistence::Persistence;
use std::sync::Arc;

use crate::runtime::AgentOutput;

/// A session that has not been updated for longer than this is considered
/// interrupted (a live session constantly updates the DB while working).
const DEFAULT_STALENESS_MINUTES: i64 = 5;

/// Summary information about a stored session.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: SessionId,
    pub query: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Total number of agents recorded for the session.
    pub total_agents: usize,
    /// Number of agents that finished successfully.
    pub completed_agents: usize,
}

/// Reconstructed state of an interrupted session, ready to be resumed.
#[derive(Debug, Clone)]
pub struct ResumeState {
    pub session_id: SessionId,
    pub query: String,
    /// Outputs of agents that completed before the interruption.
    pub completed_agents: Vec<AgentOutput>,
    /// Tasks of agents that did not complete and should be re-run.
    pub pending_tasks: Vec<String>,
}

/// Finds and resumes interrupted sessions from the persistence layer.
pub struct SessionResumer {
    db: Arc<Persistence>,
    /// A 'running' session with no activity for this long is interrupted.
    staleness: chrono::Duration,
}

impl SessionResumer {
    pub fn new(db: Arc<Persistence>) -> Self {
        Self {
            db,
            staleness: chrono::Duration::minutes(DEFAULT_STALENESS_MINUTES),
        }
    }

    /// Override the staleness threshold (mainly useful for tests).
    pub fn with_staleness(db: Arc<Persistence>, staleness: chrono::Duration) -> Self {
        Self { db, staleness }
    }

    /// Find sessions that were interrupted: status is `running` but there has
    /// been no database activity within the staleness window.
    ///
    /// Errors are logged and swallowed (returns an empty list) so that
    /// callers can use this opportunistically at startup.
    pub fn find_interrupted_sessions(&self) -> Vec<SessionInfo> {
        let cutoff = chrono::Utc::now() - self.staleness;

        let rows = match self.db.list_sessions_with_status("running") {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!("failed to list running sessions: {e}");
                return Vec::new();
            }
        };

        rows.into_iter()
            .filter_map(|row| {
                let updated_at = parse_rfc3339(&row.updated_at)?;
                // Still fresh — likely a live session, not an interrupted one.
                if updated_at > cutoff {
                    return None;
                }
                let session_id = SessionId(row.id.clone());
                let (total_agents, completed_agents) = self
                    .db
                    .count_session_agents(&session_id)
                    .unwrap_or((0, 0));
                Some(SessionInfo {
                    session_id,
                    query: row.query,
                    created_at: parse_rfc3339(&row.created_at).unwrap_or(updated_at),
                    updated_at,
                    total_agents,
                    completed_agents,
                })
            })
            .collect()
    }

    /// Resume a session from where it left off.
    ///
    /// Loads the session query, recovers outputs of completed agents
    /// (including their findings and subtree token accounting) and collects
    /// the tasks of agents that never finished (spawned, running, failed or
    /// cancelled) so they can be re-executed.
    pub async fn resume_session(&self, session_id: &SessionId) -> anyhow::Result<ResumeState> {
        let session = self
            .db
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;

        let agent_rows = self.db.get_session_agent_rows(session_id)?;

        // Findings grouped by their agent (stored separately in the DB).
        let mut findings_by_agent: std::collections::HashMap<String, Vec<pr_core::Finding>> =
            std::collections::HashMap::new();
        match self.db.get_session_findings(session_id) {
            Ok(findings) => {
                for f in findings {
                    findings_by_agent.entry(f.agent_id.0.clone()).or_default().push(f);
                }
            }
            Err(e) => tracing::warn!("resume: findings unavailable: {e}"),
        }

        // Subtree token accounting: descendant_tokens(agent) = Σ (tokens +
        // descendants) over the whole subtree below it.
        let subtree_tokens = compute_subtree_tokens(&agent_rows);

        let mut completed_agents = Vec::new();
        let mut pending_tasks = Vec::new();

        for row in agent_rows {
            if row.status == "completed" {
                completed_agents.push(AgentOutput {
                    agent_id: AgentId(row.id.clone()),
                    summary: row.summary.unwrap_or_default(),
                    tokens_used: row.tokens_used.max(0) as u64,
                    descendant_tokens: subtree_tokens.get(&row.id).copied().unwrap_or(0),
                    findings: findings_by_agent.remove(&row.id).unwrap_or_default(),
                    aborted: false,
                });
            } else {
                pending_tasks.push(row.task);
            }
        }

        tracing::info!(
            session = %session_id,
            completed = completed_agents.len(),
            pending = pending_tasks.len(),
            "session state recovered for resume"
        );

        Ok(ResumeState {
            session_id: session_id.clone(),
            query: session.query,
            completed_agents,
            pending_tasks,
        })
    }
}

fn parse_rfc3339(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

/// For every agent id, the total tokens consumed by its whole subtree
/// (excluding the agent's own `tokens_used`, matching the runtime's
/// `descendant_tokens` semantics). Cycles are impossible in the schema but
/// guarded against anyway.
fn compute_subtree_tokens(
    rows: &[pr_persistence::AgentRow],
) -> std::collections::HashMap<String, u64> {
    let mut children: std::collections::HashMap<String, Vec<&pr_persistence::AgentRow>> =
        std::collections::HashMap::new();
    for row in rows {
        if let Some(parent) = &row.parent_id {
            children.entry(parent.clone()).or_default().push(row);
        }
    }

    let mut memo: std::collections::HashMap<String, u64> = std::collections::HashMap::new();

    fn subtree_sum(
        id: &str,
        children: &std::collections::HashMap<String, Vec<&pr_persistence::AgentRow>>,
        memo: &mut std::collections::HashMap<String, u64>,
        depth: usize,
    ) -> u64 {
        if depth > 64 {
            return 0; // cycle guard
        }
        if let Some(v) = memo.get(id) {
            return *v;
        }
        let mut sum = 0u64;
        if let Some(kids) = children.get(id) {
            for kid in kids {
                let own = kid.tokens_used.max(0) as u64;
                let below = subtree_sum(&kid.id, children, memo, depth + 1);
                sum += own + below;
            }
        }
        memo.insert(id.to_string(), sum);
        sum
    }

    for row in rows {
        subtree_sum(&row.id, &children, &mut memo, 0);
    }
    memo
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::{AgentRecord, AgentRole, AgentStatus};

    fn make_db() -> Arc<Persistence> {
        Arc::new(Persistence::in_memory().unwrap())
    }

    fn create_agent_record(
        db: &Persistence,
        session_id: &SessionId,
        task: &str,
        status: AgentStatus,
        summary: Option<&str>,
    ) {
        let agent_id = AgentId::new();
        let record = AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: task.to_string(),
            status: AgentStatus::Spawned,
            depth: 1,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        db.create_agent(&record).unwrap();
        db.update_agent_status(&agent_id, status, 42, summary)
            .unwrap();
    }

    #[tokio::test]
    async fn test_find_interrupted_sessions_detects_stale_running() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "test query").unwrap();

        // Zero staleness: the session is immediately considered stale.
        let resumer = SessionResumer::with_staleness(db.clone(), chrono::Duration::zero());
        let interrupted = resumer.find_interrupted_sessions();

        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].session_id, session_id);
        assert_eq!(interrupted[0].query, "test query");
    }

    #[tokio::test]
    async fn test_fresh_session_not_interrupted_by_default() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "fresh query").unwrap();

        // Default staleness is 5 minutes; a just-created session is fresh.
        let resumer = SessionResumer::new(db);
        assert!(resumer.find_interrupted_sessions().is_empty());
    }

    #[tokio::test]
    async fn test_completed_sessions_are_not_interrupted() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "done query").unwrap();
        db.complete_session(&session_id, "/tmp/out", 100, 2).unwrap();

        let resumer = SessionResumer::with_staleness(db.clone(), chrono::Duration::zero());
        assert!(resumer.find_interrupted_sessions().is_empty());
    }

    #[tokio::test]
    async fn test_session_info_agent_counts() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();
        create_agent_record(&db, &session_id, "a", AgentStatus::Completed, Some("done"));
        create_agent_record(&db, &session_id, "b", AgentStatus::Running, None);
        create_agent_record(&db, &session_id, "c", AgentStatus::Failed, None);

        let resumer = SessionResumer::with_staleness(db.clone(), chrono::Duration::zero());
        let interrupted = resumer.find_interrupted_sessions();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].total_agents, 3);
        assert_eq!(interrupted[0].completed_agents, 1);
    }

    #[tokio::test]
    async fn test_resume_session_recovers_state() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "research X").unwrap();
        create_agent_record(&db, &session_id, "task one", AgentStatus::Completed, Some("summary one"));
        create_agent_record(&db, &session_id, "task two", AgentStatus::Running, None);
        create_agent_record(&db, &session_id, "task three", AgentStatus::Failed, None);

        let resumer = SessionResumer::new(db);
        let state = resumer.resume_session(&session_id).await.unwrap();

        assert_eq!(state.session_id, session_id);
        assert_eq!(state.query, "research X");

        assert_eq!(state.completed_agents.len(), 1);
        assert_eq!(state.completed_agents[0].summary, "summary one");
        assert_eq!(state.completed_agents[0].tokens_used, 42);

        assert_eq!(state.pending_tasks.len(), 2);
        assert!(state.pending_tasks.contains(&"task two".to_string()));
        assert!(state.pending_tasks.contains(&"task three".to_string()));
    }

    #[tokio::test]
    async fn test_resume_missing_session_errors() {
        let db = make_db();
        let resumer = SessionResumer::new(db);
        let result = resumer.resume_session(&SessionId("does-not-exist".to_string())).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resume_session_with_no_agents() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "empty").unwrap();

        let resumer = SessionResumer::new(db);
        let state = resumer.resume_session(&session_id).await.unwrap();
        assert!(state.completed_agents.is_empty());
        assert!(state.pending_tasks.is_empty());
    }

    /// Create an agent with explicit parent and token count.
    fn create_child_record(
        db: &Persistence,
        session_id: &SessionId,
        parent: Option<&AgentId>,
        task: &str,
        status: AgentStatus,
        tokens: u64,
    ) -> AgentId {
        let agent_id = AgentId::new();
        let record = AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: parent.cloned(),
            role: AgentRole::Researcher,
            task: task.to_string(),
            status: AgentStatus::Spawned,
            depth: if parent.is_some() { 2 } else { 1 },
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        };
        db.create_agent(&record).unwrap();
        db.update_agent_status(&agent_id, status, tokens, Some("done")).unwrap();
        agent_id
    }

    #[tokio::test]
    async fn test_resume_recovers_findings_and_descendant_tokens() {
        let db = make_db();
        let session_id = SessionId::new();
        db.create_session(&session_id, "q").unwrap();

        // Parent (10 own tokens) with a completed child (32 tokens).
        let parent = create_child_record(
            &db, &session_id, None, "parent", AgentStatus::Completed, 10,
        );
        let _child = create_child_record(
            &db, &session_id, Some(&parent), "child", AgentStatus::Completed, 32,
        );
        // A finding harvested by the parent.
        let finding = pr_core::Finding {
            id: pr_core::FindingId::new(),
            agent_id: parent.clone(),
            title: "CEO identified".into(),
            content: "Maria Ivanova is the CEO".into(),
            sources: vec![],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
        };
        db.add_finding(&finding).unwrap();

        let resumer = SessionResumer::new(db);
        let state = resumer.resume_session(&session_id).await.unwrap();

        let parent_out = state
            .completed_agents
            .iter()
            .find(|a| a.agent_id == parent)
            .expect("parent recovered");
        assert_eq!(parent_out.tokens_used, 10);
        assert_eq!(parent_out.descendant_tokens, 32, "child subtree accounted");
        assert_eq!(parent_out.findings.len(), 1);
        assert_eq!(parent_out.findings[0].title, "CEO identified");
    }
}
