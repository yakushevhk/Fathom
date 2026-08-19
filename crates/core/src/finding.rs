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
