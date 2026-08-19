use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingId(pub String);

impl FindingId {
    pub fn new() -> Self {
        Self(Uuid::now_v7().to_string())
    }
}

impl std::fmt::Display for FindingId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_new_generates_non_empty() {
        let id = SessionId::new();
        assert!(!id.0.is_empty());
    }

    #[test]
    fn session_id_new_are_unique() {
        let a = SessionId::new();
        let b = SessionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn session_id_display_matches_inner() {
        let id = SessionId("abc-123".to_string());
        assert_eq!(id.to_string(), "abc-123");
        assert_eq!(format!("{}", id), "abc-123");
    }

    #[test]
    fn session_id_serde_roundtrip() {
        let id = SessionId("sess-42".to_string());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""sess-42""#);
        let back: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn session_id_new_is_valid_v7_uuid() {
        // Uuid::now_v7 generates v7 UUIDs; parse and check the version nibble.
        let id = SessionId::new();
        let uuid = Uuid::parse_str(&id.0).expect("SessionId::new must produce a valid UUID");
        assert_eq!(uuid.get_version_num(), 7);
    }

    #[test]
    fn session_id_clone_eq() {
        let a = SessionId("x".to_string());
        assert_eq!(a.clone(), a);
    }

    #[test]
    fn session_id_hash_consistent() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SessionId("one".to_string()));
        set.insert(SessionId("two".to_string()));
        set.insert(SessionId("one".to_string()));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn agent_id_new_generates_non_empty() {
        let id = AgentId::new();
        assert!(!id.0.is_empty());
    }

    #[test]
    fn agent_id_new_are_unique() {
        let a = AgentId::new();
        let b = AgentId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn agent_id_display_matches_inner() {
        let id = AgentId("agent-7".to_string());
        assert_eq!(id.to_string(), "agent-7");
    }

    #[test]
    fn agent_id_serde_roundtrip() {
        let id = AgentId("ag-1".to_string());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""ag-1""#);
        let back: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn agent_id_new_is_valid_v7_uuid() {
        let id = AgentId::new();
        let uuid = Uuid::parse_str(&id.0).expect("AgentId::new must produce a valid UUID");
        assert_eq!(uuid.get_version_num(), 7);
    }

    #[test]
    fn finding_id_new_generates_non_empty() {
        let id = FindingId::new();
        assert!(!id.0.is_empty());
    }

    #[test]
    fn finding_id_new_are_unique() {
        let a = FindingId::new();
        let b = FindingId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn finding_id_display_matches_inner() {
        let id = FindingId("finding-9".to_string());
        assert_eq!(id.to_string(), "finding-9");
    }

    #[test]
    fn finding_id_serde_roundtrip() {
        let id = FindingId("find-3".to_string());
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""find-3""#);
        let back: FindingId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn finding_id_new_is_valid_v7_uuid() {
        let id = FindingId::new();
        let uuid = Uuid::parse_str(&id.0).expect("FindingId::new must produce a valid UUID");
        assert_eq!(uuid.get_version_num(), 7);
    }

    #[test]
    fn ids_from_deserialized_strings() {
        let session: SessionId = serde_json::from_str(r#""session-1""#).unwrap();
        assert_eq!(session.0, "session-1");
        let agent: AgentId = serde_json::from_str(r#""agent-1""#).unwrap();
        assert_eq!(agent.0, "agent-1");
        let finding: FindingId = serde_json::from_str(r#""finding-1""#).unwrap();
        assert_eq!(finding.0, "finding-1");
    }

    proptest::proptest! {
        #[test]
        fn id_serde_roundtrip_proptest(s: String) {
            let session = SessionId(s.clone());
            let json = serde_json::to_string(&session).unwrap();
            let back: SessionId = serde_json::from_str(&json).unwrap();
            assert_eq!(back, session);

            let agent = AgentId(s.clone());
            let json = serde_json::to_string(&agent).unwrap();
            let back: AgentId = serde_json::from_str(&json).unwrap();
            assert_eq!(back, agent);

            let finding = FindingId(s);
            let json = serde_json::to_string(&finding).unwrap();
            let back: FindingId = serde_json::from_str(&json).unwrap();
            assert_eq!(back, finding);
        }
    }
}
