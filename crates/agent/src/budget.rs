use std::path::PathBuf;

/// Default maximum characters for a capped result summary.
const DEFAULT_MAX_SUMMARY_CHARS: usize = 4096;

/// Minimum cap even when headroom is very small.
const MIN_CAP_CHARS: usize = 512;

/// Hermes-style result budget capping for sub-agent outputs.
///
/// When a parent agent spawns sub-agents, the results may exceed what
/// fits in the parent's context window. `ResultBudget` caps each result
/// to a fair share of the available space, spilling the full text to disk.
///
/// The effective cap per result is:
///   `min(static_max, parent_headroom / batch_size)`
///
/// If the result exceeds the cap, it is truncated and the full text is
/// written to a spill file on disk.
pub struct ResultBudget {
    /// Static cap per result (characters).
    pub max_summary_chars: usize,
    /// Available tokens in the parent context (characters for simplicity).
    pub parent_headroom: usize,
    /// Number of sub-agents in the batch.
    pub batch_size: usize,
    /// Directory for spill files.
    spill_dir: PathBuf,
}

/// The output of capping a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CappedResult {
    /// The (possibly truncated) summary text.
    pub summary: String,
    /// Path to the full text on disk, if it was spilled.
    pub spill_path: Option<PathBuf>,
    /// Whether the result was capped.
    pub was_capped: bool,
    /// Original byte length before capping.
    pub original_len: usize,
}

impl ResultBudget {
    /// Create a new ResultBudget with default settings.
    pub fn new(parent_headroom: usize, batch_size: usize, spill_dir: PathBuf) -> Self {
        Self {
            max_summary_chars: DEFAULT_MAX_SUMMARY_CHARS,
            parent_headroom,
            batch_size: batch_size.max(1), // avoid division by zero
            spill_dir,
        }
    }

    /// Create a ResultBudget with a custom static cap.
    pub fn with_max_chars(mut self, max_chars: usize) -> Self {
        self.max_summary_chars = max_chars;
        self
    }

    /// Calculate the effective cap for a single result.
    ///
    /// `cap = min(static_max, parent_headroom / batch_size)`
    /// with a minimum floor of `MIN_CAP_CHARS`.
    pub fn effective_cap(&self) -> usize {
        let headroom_share = (self.parent_headroom / self.batch_size).max(MIN_CAP_CHARS);
        self.max_summary_chars.min(headroom_share)
    }

    /// Cap a result string. Returns a `CappedResult` with the (possibly
    /// truncated) summary and an optional spill path for the full text.
    ///
    /// If the result fits within the cap, it is returned unchanged with
    /// `spill_path = None`. Otherwise, the summary is truncated at a
    /// UTF-8 char boundary and the full text is written to disk.
    pub fn cap_result(&self, result: &str) -> CappedResult {
        let cap = self.effective_cap();
        let original_len = result.len();

        if original_len <= cap {
            return CappedResult {
                summary: result.to_string(),
                spill_path: None,
                was_capped: false,
                original_len,
            };
        }

        // Truncate at a UTF-8 char boundary.
        let truncated = truncate_to_char_boundary(result, cap);

        // Write the full text to disk.
        let spill_path = self.spill_dir.join(format!(
            "spill_{}.txt",
            chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f")
        ));

        let summary = if let Err(e) = std::fs::create_dir_all(&self.spill_dir) {
            format!(
                "{}\n\n[Result capped at {}/{} chars. Spill failed: {}]",
                truncated, cap, original_len, e
            )
        } else if let Err(e) = std::fs::write(&spill_path, result) {
            format!(
                "{}\n\n[Result capped at {}/{} chars. Spill write failed: {}]",
                truncated, cap, original_len, e
            )
        } else {
            format!(
                "{}\n\n[Result capped at {}/{} chars. Full text: {}]",
                truncated,
                cap,
                original_len,
                spill_path.display()
            )
        };

        CappedResult {
            summary,
            spill_path: Some(spill_path),
            was_capped: true,
            original_len,
        }
    }
}

/// Truncate a string to at most `max_bytes` bytes, stopping at a UTF-8 char boundary.
fn truncate_to_char_boundary(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    // Find the largest valid UTF-8 boundary <= max_chars.
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_effective_cap_default() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 5, tmp.path().to_path_buf());
        // headroom_share = 100_000 / 5 = 20_000
        // cap = min(4096, 20_000) = 4096
        assert_eq!(budget.effective_cap(), DEFAULT_MAX_SUMMARY_CHARS);
    }

    #[test]
    fn test_effective_cap_small_headroom() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(1000, 2, tmp.path().to_path_buf());
        // headroom_share = 1000 / 2 = 500
        // cap = min(4096, 500) = 500, but min floor is 512
        assert_eq!(budget.effective_cap(), MIN_CAP_CHARS);
    }

    #[test]
    fn test_effective_cap_zero_batch_size_clamped() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(10_000, 0, tmp.path().to_path_buf());
        // batch_size is clamped to 1, so headroom_share = 10_000
        assert_eq!(budget.effective_cap(), DEFAULT_MAX_SUMMARY_CHARS);
    }

    #[test]
    fn test_cap_result_within_budget() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 1, tmp.path().to_path_buf());
        let result = budget.cap_result("short result");
        assert!(!result.was_capped);
        assert_eq!(result.summary, "short result");
        assert!(result.spill_path.is_none());
        assert_eq!(result.original_len, 12);
    }

    #[test]
    fn test_cap_result_exceeds_budget() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(1000, 1, tmp.path().to_path_buf())
            .with_max_chars(200);
        let long_text = "x".repeat(500);
        let result = budget.cap_result(&long_text);
        assert!(result.was_capped);
        assert!(result.summary.contains("Result capped"));
        assert!(result.summary.contains("500 chars"));
        assert!(result.spill_path.is_some());
        assert_eq!(result.original_len, 500);

        // Verify spill file contains full text.
        let spill = std::fs::read_to_string(result.spill_path.unwrap()).unwrap();
        assert_eq!(spill.len(), 500);
    }

    #[test]
    fn test_cap_result_exact_boundary() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 1, tmp.path().to_path_buf())
            .with_max_chars(100);
        let text = "a".repeat(100);
        let result = budget.cap_result(&text);
        assert!(!result.was_capped);
        assert_eq!(result.summary, text);
    }

    #[test]
    fn test_cap_result_one_over_boundary() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 1, tmp.path().to_path_buf())
            .with_max_chars(100);
        let text = "a".repeat(101);
        let result = budget.cap_result(&text);
        assert!(result.was_capped);
    }

    #[test]
    fn test_cap_result_utf8_boundary() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 1, tmp.path().to_path_buf())
            .with_max_chars(10);
        // Chinese chars are 3 bytes each.
        let text = "你好世界你好世界"; // 8 chars, 24 bytes
        let result = budget.cap_result(&text);
        assert!(result.was_capped);
        // Summary should be valid UTF-8.
        assert!(std::str::from_utf8(result.summary.as_bytes()).is_ok());
    }

    #[test]
    fn test_effective_cap_custom_max() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100_000, 1, tmp.path().to_path_buf())
            .with_max_chars(2048);
        assert_eq!(budget.effective_cap(), 2048);
    }

    #[test]
    fn test_capped_result_debug() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(100, 1, tmp.path().to_path_buf())
            .with_max_chars(50);
        let result = budget.cap_result(&"x".repeat(200));
        let debug = format!("{:?}", result);
        assert!(debug.contains("was_capped: true"));
        assert!(debug.contains("original_len: 200"));
    }

    #[test]
    fn test_truncate_to_char_boundary_ascii() {
        let s = "hello world";
        assert_eq!(truncate_to_char_boundary(s, 5), "hello");
        assert_eq!(truncate_to_char_boundary(s, 100), s);
    }

    #[test]
    fn test_truncate_to_char_boundary_multibyte() {
        // Each Chinese char is 3 bytes.
        let s = "你好世界";
        let truncated = truncate_to_char_boundary(s, 4);
        // Should be "你好" (6 bytes) since "你好世" is 9 bytes > 4.
        // Actually 4 bytes: boundary at 3 = "你好" (but 3 < 4 and 6 > 4).
        // Wait: 4 bytes, valid boundaries at 0, 3, 6, 9, 12.
        // Largest <= 4 is 3, so result is "你".
        assert_eq!(truncated, "你");
    }

    #[test]
    fn test_with_max_chars_builder() {
        let tmp = TempDir::new().unwrap();
        let budget = ResultBudget::new(10_000, 1, tmp.path().to_path_buf())
            .with_max_chars(8192);
        assert_eq!(budget.max_summary_chars, 8192);
    }
}
