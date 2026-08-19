use serde::{Deserialize, Serialize};

use crate::ids::SessionId;

/// Final output of a completed research session.
///
/// Produced by the coordinator once all agents have finished and the
/// synthesis report has been written to disk. Consumed by the export and
/// notification subsystems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOutput {
    pub session_id: SessionId,
    pub output_dir: std::path::PathBuf,
    /// Synthesized final report (markdown).
    pub synthesis: String,
    /// Total tokens consumed by the whole session.
    pub total_tokens: u64,
    /// Total number of agents that ran in this session.
    pub total_agents: u32,
}

impl SessionOutput {
    /// Short human-readable completion summary used by notifications.
    pub fn summary_line(&self) -> String {
        format!(
            "Research session {} completed: {} agent(s), {} tokens. Output: {}",
            self.session_id, self.total_agents, self.total_tokens, self.output_dir.display()
        )
    }

    /// First non-empty line(s) of the synthesis, truncated to `max_chars`.
    pub fn synthesis_preview(&self, max_chars: usize) -> String {
        let preview: String = self.synthesis.chars().take(max_chars).collect();
        if self.synthesis.chars().count() > max_chars {
            format!("{preview}…")
        } else {
            preview
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SessionOutput {
        SessionOutput {
            session_id: SessionId("sess-test".to_string()),
            output_dir: std::path::PathBuf::from("/tmp/out"),
            synthesis: "# Report\n\nSome findings.".to_string(),
            total_tokens: 1234,
            total_agents: 3,
        }
    }

    #[test]
    fn test_summary_line() {
        let s = sample().summary_line();
        assert!(s.contains("sess-test"));
        assert!(s.contains("3 agent(s)"));
        assert!(s.contains("1234 tokens"));
    }

    #[test]
    fn test_synthesis_preview_truncates() {
        let out = sample().synthesis_preview(5);
        assert!(out.chars().count() <= 6); // 5 chars + ellipsis
        assert!(out.ends_with('…'));
    }

    #[test]
    fn test_synthesis_preview_short_text_untouched() {
        let out = sample().synthesis_preview(10_000);
        assert_eq!(out, "# Report\n\nSome findings.");
    }
}
