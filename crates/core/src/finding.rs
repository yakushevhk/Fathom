use serde::{Deserialize, Serialize};
use crate::ids::{AgentId, FindingId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub agent_id: AgentId,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

fn default_confidence() -> f32 {
    0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, FindingId};
    use chrono::Utc;

    // -----------------------------------------------------------------------
    // Source
    // -----------------------------------------------------------------------

    #[test]
    fn source_minimal() {
        let s = Source {
            url: "https://example.com".into(),
            title: "Example".into(),
            excerpt: String::new(),
        };
        assert_eq!(s.url, "https://example.com");
        assert_eq!(s.title, "Example");
        assert!(s.excerpt.is_empty());
    }

    #[test]
    fn source_serde_roundtrip() {
        let s = Source {
            url: "https://example.com/doc".into(),
            title: "Doc".into(),
            excerpt: "some text".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Source = serde_json::from_str(&json).unwrap();
        assert_eq!(back.url, s.url);
        assert_eq!(back.title, s.title);
        assert_eq!(back.excerpt, s.excerpt);
    }

    #[test]
    fn source_excerpt_defaults_empty() {
        // excerpt has #[serde(default)] so it should deserialize from missing
        let json = r#"{"url":"https://x.com","title":"X"}"#;
        let s: Source = serde_json::from_str(json).unwrap();
        assert_eq!(s.url, "https://x.com");
        assert_eq!(s.title, "X");
        assert!(s.excerpt.is_empty());
    }

    // -----------------------------------------------------------------------
    // Finding
    // -----------------------------------------------------------------------

    #[test]
    fn finding_minimal() {
        let now = Utc::now();
        let f = Finding {
            id: FindingId("f-1".into()),
            agent_id: AgentId("a-1".into()),
            title: "Test Finding".into(),
            content: "Some content".into(),
            sources: vec![],
            confidence: 0.5,
            created_at: now,
        };
        assert_eq!(f.title, "Test Finding");
        assert_eq!(f.content, "Some content");
        assert!(f.sources.is_empty());
        assert_eq!(f.confidence, 0.5);
    }

    #[test]
    fn finding_default_confidence_used() {
        // When confidence is absent from JSON, it should default to 0.5
        let now = Utc::now();
        let json = serde_json::json!({
            "id": "f-2",
            "agent_id": "a-2",
            "title": "Default Confidence",
            "content": "test",
            "sources": [],
            "created_at": now.to_rfc3339(),
        });
        let f: Finding = serde_json::from_value(json).unwrap();
        assert_eq!(f.confidence, 0.5);
    }

    #[test]
    fn finding_serde_roundtrip() {
        let now = Utc::now();
        let f = Finding {
            id: FindingId("f-3".into()),
            agent_id: AgentId("a-3".into()),
            title: "Serde".into(),
            content: "Roundtrip".into(),
            sources: vec![Source {
                url: "https://src.com".into(),
                title: "Source".into(),
                excerpt: "excerpt".into(),
            }],
            confidence: 0.9,
            created_at: now,
        };
        let json = serde_json::to_string(&f).unwrap();
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, f.id);
        assert_eq!(back.agent_id, f.agent_id);
        assert_eq!(back.title, f.title);
        assert_eq!(back.content, f.content);
        assert_eq!(back.sources.len(), 1);
        assert_eq!(back.sources[0].url, "https://src.com");
        assert!((back.confidence - 0.9).abs() < f32::EPSILON);
        assert_eq!(back.created_at, now);
    }

    #[test]
    fn finding_with_sources() {
        let now = Utc::now();
        let f = Finding {
            id: FindingId("f-4".into()),
            agent_id: AgentId("a-4".into()),
            title: "Multi Source".into(),
            content: "content".into(),
            sources: vec![
                Source {
                    url: "https://a.com".into(),
                    title: "A".into(),
                    excerpt: "excerpt a".into(),
                },
                Source {
                    url: "https://b.com".into(),
                    title: "B".into(),
                    excerpt: "excerpt b".into(),
                },
            ],
            confidence: 0.3,
            created_at: now,
        };
        assert_eq!(f.sources.len(), 2);
        assert_eq!(f.sources[0].url, "https://a.com");
        assert_eq!(f.sources[1].url, "https://b.com");
    }

    #[test]
    fn finding_confidence_bounds() {
        let now = Utc::now();
        let f = Finding {
            id: FindingId("f-5".into()),
            agent_id: AgentId("a-5".into()),
            title: "Confidence".into(),
            content: "test".into(),
            sources: vec![],
            confidence: 0.0,
            created_at: now,
        };
        assert_eq!(f.confidence, 0.0);

        let f2 = Finding {
            id: FindingId("f-6".into()),
            agent_id: AgentId("a-6".into()),
            title: "Max Confidence".into(),
            content: "test".into(),
            sources: vec![],
            confidence: 1.0,
            created_at: now,
        };
        assert_eq!(f2.confidence, 1.0);
    }

    #[test]
    fn finding_sources_defaults_empty() {
        let now = Utc::now();
        let json = serde_json::json!({
            "id": "f-7",
            "agent_id": "a-7",
            "title": "No Sources",
            "content": "test",
            "created_at": now.to_rfc3339(),
        });
        let f: Finding = serde_json::from_value(json).unwrap();
        assert!(f.sources.is_empty());
    }

    // -----------------------------------------------------------------------
    // Proptest
    // -----------------------------------------------------------------------
    proptest::proptest! {
        #[test]
        fn source_serde_roundtrip_proptest(
            url: String,
            title: String,
            excerpt: String,
        ) {
            let s = Source { url: url.clone(), title: title.clone(), excerpt: excerpt.clone() };
            let json = serde_json::to_string(&s).unwrap();
            let back: Source = serde_json::from_str(&json).unwrap();
            assert_eq!(back.url, url);
            assert_eq!(back.title, title);
            assert_eq!(back.excerpt, excerpt);
        }
    }
}
