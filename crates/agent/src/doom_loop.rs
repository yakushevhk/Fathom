//! Doom loop detection.
//!
//! A "doom loop" occurs when an agent repeatedly issues the exact same tool
//! call (same tool, same arguments) without making progress — typically a sign
//! the model is stuck retrying a failing operation. This module tracks recent
//! tool call signatures and raises an alarm when the last N calls are all
//! identical.

use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// Default number of consecutive identical tool calls that triggers detection.
pub const DEFAULT_MAX_IDENTICAL: usize = 3;

/// A compact fingerprint of a single tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCallSignature {
    tool_name: String,
    args_hash: String,
}

/// Detects agents stuck in a loop of identical tool calls.
///
/// The detector keeps a bounded history of the most recent tool call
/// signatures. After each recorded call, it reports whether the last
/// `max_identical` calls are all identical.
pub struct DoomLoopDetector {
    history: VecDeque<ToolCallSignature>,
    max_identical: usize,
}

impl DoomLoopDetector {
    /// Create a detector with the default threshold (3 identical calls).
    pub fn new() -> Self {
        Self::with_max_identical(DEFAULT_MAX_IDENTICAL)
    }

    /// Create a detector with a custom threshold.
    ///
    /// Values below 1 are clamped to 1.
    pub fn with_max_identical(max_identical: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_identical.max(1)),
            max_identical: max_identical.max(1),
        }
    }

    /// Record a tool call and check whether a doom loop is now present.
    ///
    /// Returns `true` when the last `max_identical` recorded calls are all
    /// identical (same tool name and same argument hash).
    pub fn record_and_check(&mut self, tool_name: &str, args: &serde_json::Value) -> bool {
        let signature = ToolCallSignature {
            tool_name: tool_name.to_string(),
            args_hash: hash_args(args),
        };
        self.history.push_back(signature);

        // Keep the history bounded to the detection window.
        while self.history.len() > self.max_identical {
            self.history.pop_front();
        }

        self.history.len() == self.max_identical
            && self
                .history
                .iter()
                .all(|sig| *sig == self.history[0])
    }

    /// Clear the recorded history (e.g. after the agent recovers or after a
    /// doom loop warning has been handled).
    pub fn reset(&mut self) {
        self.history.clear();
    }

    /// Number of tool calls currently held in the history window.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

impl Default for DoomLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash tool call arguments into a stable hex string.
///
/// `serde_json::Value` maps serialize in sorted key order (BTreeMap backing),
/// so semantically identical argument objects produce the same hash
/// regardless of insertion order.
fn hash_args(args: &serde_json::Value) -> String {
    let serialized = serde_json::to_string(args).unwrap_or_default();
    let mut hasher = DefaultHasher::new();
    serialized.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_no_doom_loop_below_threshold() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_doom_loop_detected_on_third_identical_call() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_different_args_do_not_trigger() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "pwd"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_different_tools_do_not_trigger() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("file_read", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_interrupted_sequence_resets_window() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        // Different call breaks the streak.
        assert!(!d.record_and_check("web_search", &json!({"query": "rust"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_stays_true_while_loop_continues() {
        let mut d = DoomLoopDetector::new();
        for _ in 0..2 {
            assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        }
        assert!(d.record_and_check("shell", &json!({"command": "ls"})));
        // A fourth identical call is still a doom loop.
        assert!(d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_reset_clears_history() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        d.reset();
        assert_eq!(d.history_len(), 0);
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_custom_threshold() {
        let mut d = DoomLoopDetector::with_max_identical(2);
        assert!(!d.record_and_check("shell", &json!({"command": "ls"})));
        assert!(d.record_and_check("shell", &json!({"command": "ls"})));
    }

    #[test]
    fn test_arg_key_order_does_not_matter() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("web_search", &json!({"query": "rust", "limit": 5})));
        assert!(!d.record_and_check("web_search", &json!({"limit": 5, "query": "rust"})));
        assert!(d.record_and_check("web_search", &json!({"query": "rust", "limit": 5})));
    }

    #[test]
    fn test_empty_args_are_equal() {
        let mut d = DoomLoopDetector::new();
        assert!(!d.record_and_check("noop", &json!({})));
        assert!(!d.record_and_check("noop", &json!({})));
        assert!(d.record_and_check("noop", &json!({})));
    }

    #[test]
    fn test_history_is_bounded() {
        let mut d = DoomLoopDetector::new();
        for i in 0..10 {
            d.record_and_check("shell", &json!({"command": format!("cmd-{i}")}));
        }
        assert!(d.history_len() <= 3);
    }
}
