use pr_core::ToolOutput;
use std::path::Path;

// ── Approach note (fleet report B16) ──
//
// The spill-to-disk write below (`create_dir_all` + `write`) is blocking I/O
// on the async tool path. The clean fix would be to make `truncate_tool_output`
// / `apply_turn_budget` async and move the write into
// `tokio::task::spawn_blocking`, but `apply_turn_budget` is called from
// `crates/agent/src/runtime.rs` (the only non-test caller), which this task
// does not own. Both functions therefore stay synchronous and we instead bound
// the cost of the blocking write with [`MAX_SPILL_BYTES`]: outputs larger than
// 5 MB are truncated in memory only and never written to disk, so the worst
// case synchronous write is a ~5 MB file rather than an unbounded body.

/// Tools that should never be truncated.
const PINNED_TOOLS: &[&str] = &["file_read"];

/// Default preview size when a large result is persisted to disk (2 KB).
const PREVIEW_BYTES: usize = 2048;

/// Maximum content size that will be spilled to disk when truncating (5 MB).
/// Larger outputs are truncated in memory only — see the approach note above.
const MAX_SPILL_BYTES: usize = 5 * 1024 * 1024;

/// Truncation result: either the original output (unchanged) or a replacement
/// that points to the full result on disk.
#[derive(Debug)]
pub enum Truncated {
    /// Output was within limits.
    Unchanged(ToolOutput),
    /// Output was truncated and persisted to disk.
    Truncated {
        /// Replacement output with a preview + pointer.
        replacement: ToolOutput,
        /// Path where the full result was written.
        persisted_path: std::path::PathBuf,
        /// Original byte size.
        original_bytes: usize,
    },
}

/// Truncate a single tool output according to the configured limits.
///
/// - `tool_name` is checked against the pinned-tools list.
/// - `max_bytes` / `max_lines` are the per-tool caps.
/// - `working_dir` is used to derive the directory for persisted files.
/// - Outputs over [`MAX_SPILL_BYTES`] are truncated in memory only (no disk
///   spill); see the approach note at the top of this module.
pub fn truncate_tool_output(
    tool_name: &str,
    output: &ToolOutput,
    max_bytes: u32,
    max_lines: u32,
    working_dir: &Path,
) -> anyhow::Result<Truncated> {
    // Pinned tools are never truncated.
    if PINNED_TOOLS.contains(&tool_name) {
        return Ok(Truncated::Unchanged(output.clone()));
    }

    let content = &output.content;
    let byte_len = content.len();
    let line_count = content.lines().count();

    let over_bytes = byte_len > max_bytes as usize;
    let over_lines = line_count > max_lines as usize;

    if !over_bytes && !over_lines {
        return Ok(Truncated::Unchanged(output.clone()));
    }

    // Persist the full result to disk, subject to the spill size guard
    // (fleet report B16): outputs over MAX_SPILL_BYTES are truncated in
    // memory only so the synchronous write stays bounded. Disk failures must
    // NOT abort the agent run — fall back to an in-memory-only truncation.
    let persist_dir = working_dir.join(".pr-context");
    let filename = format!(
        "{}_{}.txt",
        tool_name,
        chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f")
    );
    let persisted_path = persist_dir.join(&filename);
    let spillable = byte_len <= MAX_SPILL_BYTES;
    let persisted = spillable
        && std::fs::create_dir_all(&persist_dir)
            .and_then(|_| std::fs::write(&persisted_path, content))
            .is_ok();
    if spillable && !persisted {
        tracing::warn!(
            "failed to persist oversized tool output to {}; truncating in memory only",
            persisted_path.display()
        );
    }

    // Build a preview (first PREVIEW_BYTES bytes, truncated at a char boundary).
    let preview = truncate_to_bytes(content, PREVIEW_BYTES);
    let location = if persisted {
        format!(" Full result saved to: {}", persisted_path.display())
    } else {
        String::new()
    };
    let replacement_content = format!(
        "[Output truncated: {} bytes, {} lines.{}]\n\nPreview:\n{}",
        byte_len, line_count, location, preview,
    );

    let replacement = if output.success {
        let mut meta = output.metadata.clone().unwrap_or(serde_json::json!({}));
        if let serde_json::Value::Object(ref mut map) = meta {
            if persisted {
                map.insert(
                    "persisted_to".to_string(),
                    serde_json::Value::String(persisted_path.display().to_string()),
                );
            }
            map.insert(
                "original_bytes".to_string(),
                serde_json::Value::Number(byte_len.into()),
            );
        }
        ToolOutput::ok_with_meta(replacement_content, meta)
    } else {
        ToolOutput::err(replacement_content)
    };

    Ok(Truncated::Truncated {
        replacement,
        persisted_path: if persisted {
            persisted_path
        } else {
            working_dir.join(".pr-context")
        },
        original_bytes: byte_len,
    })
}

/// Truncate a string to at most `max_bytes` bytes, stopping at a UTF-8 char boundary.
fn truncate_to_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find the largest valid UTF-8 boundary <= max_bytes.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let truncated = &s[..end];
    format!("{}... [truncated]", truncated)
}

/// Aggregate budget tracker for a single agent turn.
///
/// Tracks how many bytes of tool output have been consumed in the current turn.
/// When the budget is exceeded, subsequent tool outputs are aggressively truncated.
#[derive(Debug)]
pub struct TurnBudget {
    used_bytes: usize,
    budget_bytes: usize,
}

impl TurnBudget {
    pub fn new(budget_bytes: u32) -> Self {
        Self {
            used_bytes: 0,
            budget_bytes: budget_bytes as usize,
        }
    }

    /// Record `n` bytes consumed and return the remaining budget.
    pub fn consume(&mut self, n: usize) -> usize {
        self.used_bytes += n;
        self.remaining()
    }

    /// Bytes remaining in the budget.
    pub fn remaining(&self) -> usize {
        self.budget_bytes.saturating_sub(self.used_bytes)
    }

    /// Whether the budget is exhausted.
    pub fn is_exhausted(&self) -> bool {
        self.used_bytes >= self.budget_bytes
    }

    /// Total bytes used so far.
    pub fn used(&self) -> usize {
        self.used_bytes
    }
}

/// Apply a tool output to the turn budget, truncating if necessary.
///
/// This combines per-tool truncation with the per-turn aggregate limit.
pub fn apply_turn_budget(
    tool_name: &str,
    output: &ToolOutput,
    max_bytes: u32,
    max_lines: u32,
    turn_budget: &mut TurnBudget,
    working_dir: &Path,
) -> anyhow::Result<Truncated> {
    // First, apply per-tool truncation.
    let result = truncate_tool_output(tool_name, output, max_bytes, max_lines, working_dir)?;

    let (content, original_bytes) = match &result {
        Truncated::Unchanged(o) => (o.content.clone(), o.content.len()),
        Truncated::Truncated {
            replacement,
            original_bytes,
            ..
        } => (replacement.content.clone(), *original_bytes),
    };

    // Record against turn budget.
    let remaining = turn_budget.consume(content.len());

    if remaining == 0 && content.len() > 1024 {
        // Budget exhausted: aggressively truncate to a tiny preview.
        let tiny = truncate_to_bytes(&content, 512);
        let aggressive = ToolOutput::ok(format!(
            "[Turn budget exhausted — {} bytes used]\n{}",
            turn_budget.used(),
            tiny,
        ));
        return Ok(Truncated::Truncated {
            replacement: aggressive,
            persisted_path: Path::new("(budget-exceeded)").to_path_buf(),
            original_bytes,
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_no_truncation_when_within_limits() {
        let tmp = TempDir::new().unwrap();
        let output = ToolOutput::ok("small output");
        let result =
            truncate_tool_output("web_search", &output, 50_000, 2000, tmp.path()).unwrap();
        match result {
            Truncated::Unchanged(o) => assert_eq!(o.content, "small output"),
            _ => panic!("expected Unchanged"),
        }
    }

    #[test]
    fn test_pinned_tool_never_truncated() {
        let tmp = TempDir::new().unwrap();
        let big = "x".repeat(100_000);
        let output = ToolOutput::ok(&big);
        let result =
            truncate_tool_output("file_read", &output, 50_000, 2000, tmp.path()).unwrap();
        match result {
            Truncated::Unchanged(o) => assert_eq!(o.content.len(), 100_000),
            _ => panic!("file_read should never be truncated"),
        }
    }

    #[test]
    fn test_truncation_by_bytes() {
        let tmp = TempDir::new().unwrap();
        let big = "x".repeat(100_000);
        let output = ToolOutput::ok(&big);
        let result =
            truncate_tool_output("shell", &output, 50_000, 10_000, tmp.path()).unwrap();
        match result {
            Truncated::Truncated {
                replacement,
                original_bytes,
                ..
            } => {
                assert_eq!(original_bytes, 100_000);
                assert!(replacement.content.contains("Output truncated"));
                assert!(replacement.content.contains("100000 bytes"));
            }
            _ => panic!("expected Truncated"),
        }
    }

    #[test]
    fn test_truncation_by_lines() {
        let tmp = TempDir::new().unwrap();
        let lines: Vec<String> = (0..5000).map(|i| format!("line {i}")).collect();
        let content = lines.join("\n");
        let output = ToolOutput::ok(&content);
        let result =
            truncate_tool_output("shell", &output, 1_000_000, 2000, tmp.path()).unwrap();
        match result {
            Truncated::Truncated { original_bytes, .. } => {
                assert!(original_bytes > 0);
            }
            _ => panic!("expected Truncated (line limit)"),
        }
    }

    #[test]
    fn test_turn_budget() {
        let mut budget = TurnBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
        assert!(!budget.is_exhausted());

        budget.consume(600);
        assert_eq!(budget.remaining(), 400);

        budget.consume(500);
        assert_eq!(budget.remaining(), 0);
        assert!(budget.is_exhausted());
    }

    #[test]
    fn test_truncate_to_bytes_utf8_boundary() {
        // Multi-byte UTF-8: each Chinese char is 3 bytes.
        let s = "你好世界你好世界"; // 8 chars * 3 bytes = 24 bytes
        let t = truncate_to_bytes(s, 10);
        // Should stop at a valid boundary, likely after 3 chars (9 bytes).
        assert!(t.len() <= 10 + "... [truncated]".len());
        // Verify it's valid UTF-8.
        assert!(std::str::from_utf8(t.as_bytes()).is_ok());
    }

    #[test]
    fn test_apply_turn_budget() {
        let tmp = TempDir::new().unwrap();
        let mut budget = TurnBudget::new(1000);
        let output = ToolOutput::ok("hello");
        let result =
            apply_turn_budget("web_search", &output, 50_000, 2000, &mut budget, tmp.path())
                .unwrap();
        // Should be unchanged since it's small.
        match result {
            Truncated::Unchanged(_) => {}
            _ => panic!("expected Unchanged"),
        }
    }

    // ── Spill size guard (fleet report B16, fallback approach) ──

    #[test]
    fn test_oversized_output_truncated_in_memory_only() {
        let tmp = TempDir::new().unwrap();
        // One byte over the spill guard: must NOT be written to disk.
        let huge = "x".repeat(MAX_SPILL_BYTES + 1);
        let output = ToolOutput::ok(&huge);
        let result =
            truncate_tool_output("shell", &output, 50_000, 10_000, tmp.path()).unwrap();
        match result {
            Truncated::Truncated {
                replacement,
                original_bytes,
                ..
            } => {
                assert_eq!(original_bytes, MAX_SPILL_BYTES + 1);
                assert!(replacement.content.contains("Output truncated"));
                // No disk pointer: pure in-memory truncation.
                assert!(!replacement.content.contains("Full result saved to"));
                if let Some(meta) = &replacement.metadata {
                    assert!(meta.get("persisted_to").is_none());
                }
            }
            _ => panic!("expected Truncated"),
        }
        // Nothing was written to the working dir.
        assert!(!tmp.path().join(".pr-context").exists());
    }

    #[test]
    fn test_within_guard_output_still_spilled_to_disk() {
        let tmp = TempDir::new().unwrap();
        // Over the per-tool byte cap but well under MAX_SPILL_BYTES.
        let big = "y".repeat(100_000);
        let output = ToolOutput::ok(&big);
        let result =
            truncate_tool_output("shell", &output, 50_000, 10_000, tmp.path()).unwrap();
        match result {
            Truncated::Truncated {
                replacement,
                persisted_path,
                ..
            } => {
                assert!(replacement.content.contains("Full result saved to"));
                assert!(persisted_path.exists(), "spill file should exist on disk");
                assert_eq!(persisted_path.parent().unwrap().file_name().unwrap(), ".pr-context");
            }
            _ => panic!("expected Truncated"),
        }
    }
}
