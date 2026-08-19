//! Session history: listing, search and detail retrieval over persisted sessions.
//!
//! `SessionHistory` is a thin, ergonomic read-only facade on top of
//! [`Persistence`] for the features that a "past sessions" UI needs.

use std::sync::Arc;

use pr_core::{Finding, SessionId};

use crate::db::{AgentDetailRow, Persistence, SessionRow};

/// Compact session info for list/search results.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub query: String,
    pub status: String,
    pub output_dir: Option<String>,
    pub total_tokens: i64,
    pub total_agents: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<SessionRow> for SessionSummary {
    fn from(row: SessionRow) -> Self {
        Self {
            id: SessionId(row.id),
            query: row.query,
            status: row.status,
            output_dir: row.output_dir,
            total_tokens: row.total_tokens,
            total_agents: row.total_agents,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Everything known about one session: metadata, agents and findings.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionDetails {
    pub session: SessionSummary,
    pub agents: Vec<AgentDetailRow>,
    pub findings: Vec<Finding>,
}

/// Read-only access to the session history stored in the database.
#[derive(Clone)]
pub struct SessionHistory {
    db: Arc<Persistence>,
}

impl std::fmt::Debug for SessionHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionHistory").finish_non_exhaustive()
    }
}

impl SessionHistory {
    pub fn new(db: Arc<Persistence>) -> Self {
        Self { db }
    }

    /// Most recent sessions, newest first, capped at `limit`.
    pub fn list_sessions(&self, limit: usize) -> Vec<SessionSummary> {
        match self.db.list_sessions() {
            Ok(rows) => rows.into_iter().take(limit).map(Into::into).collect(),
            Err(e) => {
                tracing::error!("failed to list sessions: {e}");
                Vec::new()
            }
        }
    }

    /// Sessions whose query text matches `query` (case-insensitive substring),
    /// newest first.
    pub fn search_sessions(&self, query: &str) -> Vec<SessionSummary> {
        if query.trim().is_empty() {
            return self.list_sessions(usize::MAX);
        }
        match self.db.search_sessions(query.trim()) {
            Ok(rows) => rows.into_iter().map(Into::into).collect(),
            Err(e) => {
                tracing::error!("failed to search sessions: {e}");
                Vec::new()
            }
        }
    }

    /// Full details for a session, or `None` if it does not exist.
    pub fn get_session_details(&self, id: &SessionId) -> Option<SessionDetails> {
        let row = match self.db.get_session(id) {
            Ok(Some(row)) => row,
            Ok(None) => return None,
            Err(e) => {
                tracing::error!("failed to fetch session {id}: {e}");
                return None;
            }
        };

        let agents = self
            .db
            .get_session_agents_detail(id)
            .unwrap_or_else(|e| {
                tracing::error!("failed to fetch agents for session {id}: {e}");
                Vec::new()
            });

        let findings = self
            .db
            .get_session_findings(id)
            .unwrap_or_else(|e| {
                tracing::error!("failed to fetch findings for session {id}: {e}");
                Vec::new()
            });

        Some(SessionDetails {
            session: row.into(),
            agents,
            findings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::{AgentId, AgentRecord, AgentRole, AgentStatus, FindingId, Source};

    fn create_session_with_agent(db: &Persistence, query: &str) -> (SessionId, AgentId) {
        let session_id = SessionId::new();
        db.create_session(&session_id, query).unwrap();

        let agent_id = AgentId::new();
        db.create_agent(&AgentRecord {
            id: agent_id.clone(),
            session_id: session_id.0.clone(),
            parent_id: None,
            role: AgentRole::Researcher,
            task: format!("task for {query}"),
            status: AgentStatus::Spawned,
            depth: 1,
            tokens_used: 0,
            created_at: chrono::Utc::now(),
            completed_at: None,
        })
        .unwrap();

        (session_id, agent_id)
    }

    #[test]
    fn test_list_sessions_newest_first_and_limit() {
        let db = Arc::new(Persistence::in_memory().unwrap());

        let (s1, _) = create_session_with_agent(&db, "first query");
        db.complete_session(&s1, "/tmp/first", 100, 1).unwrap();
        let (s2, _) = create_session_with_agent(&db, "second query");

        let history = SessionHistory::new(db.clone());

        let all = history.list_sessions(10);
        assert_eq!(all.len(), 2);
        // s2 was updated more recently (complete_session touches updated_at).
        assert_eq!(all[0].id, s2);
        assert_eq!(all[1].id, s1);
        assert_eq!(all[1].status, "completed");
        assert_eq!(all[1].total_tokens, 100);

        let limited = history.list_sessions(1);
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].id, s2);
    }

    #[test]
    fn test_search_sessions_substring() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        create_session_with_agent(&db, "Research quantum computing");
        create_session_with_agent(&db, "Analyze coffee markets");
        create_session_with_agent(&db, "QUANTUM sensing overview");

        let history = SessionHistory::new(db.clone());

        let hits = history.search_sessions("quantum");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|s| s.query.to_lowercase().contains("quantum")));

        // SQL wildcards in user input are treated literally.
        assert!(history.search_sessions("%").is_empty());

        // Empty query falls back to listing everything.
        assert_eq!(history.search_sessions("  ").len(), 3);
    }

    #[test]
    fn test_get_session_details_includes_agents_and_findings() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let (session_id, agent_id) = create_session_with_agent(&db, "detail test");

        db.update_agent_status(&agent_id, AgentStatus::Completed, 55, Some("agent summary"))
            .unwrap();

        db.add_finding(&Finding {
            id: FindingId::new(),
            agent_id: agent_id.clone(),
            title: "Key insight".to_string(),
            content: "The answer is 42.".to_string(),
            sources: vec![Source {
                url: "https://example.com".to_string(),
                title: "Example".to_string(),
                excerpt: String::new(),
            }],
            confidence: 0.9,
            created_at: chrono::Utc::now(),
        })
        .unwrap();

        let history = SessionHistory::new(db.clone());
        let details = history.get_session_details(&session_id).expect("session exists");

        assert_eq!(details.session.query, "detail test");
        assert_eq!(details.agents.len(), 1);
        assert_eq!(details.agents[0].status, "completed");
        assert_eq!(details.agents[0].tokens_used, 55);
        assert_eq!(details.agents[0].summary.as_deref(), Some("agent summary"));
        assert_eq!(details.findings.len(), 1);
        assert_eq!(details.findings[0].title, "Key insight");
        assert_eq!(details.findings[0].sources[0].url, "https://example.com");
    }

    #[test]
    fn test_get_session_details_missing_returns_none() {
        let db = Arc::new(Persistence::in_memory().unwrap());
        let history = SessionHistory::new(db);
        assert!(history.get_session_details(&SessionId("nope".into())).is_none());
    }
}
