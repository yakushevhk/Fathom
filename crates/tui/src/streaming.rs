/// Line-buffered streaming display optimization.
///
/// Buffers incoming LLM tokens and only publishes completed lines to the display.
/// A trailing partial line is hidden until it completes (receives a newline),
/// dramatically reducing re-renders during streaming.
pub struct StreamingBuffer {
    /// Raw buffer accumulating all incoming text
    buffer: String,
    /// Lines that have been fully completed (ended with newline)
    published_lines: Vec<String>,
    /// The current incomplete line (not yet published)
    partial_line: String,
    /// Flag indicating new content is available
    has_new: bool,
}

impl StreamingBuffer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            published_lines: Vec::new(),
            partial_line: String::new(),
            has_new: false,
        }
    }

    /// Push a text delta into the buffer. Completed lines (after newline) are
    /// moved to `published_lines`; the trailing partial line stays hidden.
    pub fn push(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        self.buffer.push_str(delta);
        self.has_new = true;

        // Split accumulated buffer into lines; the last element is the
        // incomplete partial line (even if empty).
        let mut parts: Vec<String> = self
            .buffer
            .split('\n')
            .map(|s| s.to_string())
            .collect();

        // The last element is always the partial (incomplete) line
        if let Some(partial) = parts.pop() {
            self.partial_line = partial;
        }

        // Everything before the last element is a completed line
        for line in parts {
            self.published_lines.push(line);
        }

        // Rebuild the raw buffer from partial_line only (published lines are
        // already stored separately)
        self.buffer = self.partial_line.clone();
    }

    /// Returns all published text (completed lines joined with newlines).
    pub fn published_text(&self) -> String {
        self.published_lines.join("\n")
    }

    /// Returns true if there is new content since the last check.
    pub fn has_new_content(&self) -> bool {
        self.has_new
    }

    /// Mark content as consumed (resets the new-content flag).
    pub fn ack_new_content(&mut self) {
        self.has_new = false;
    }

    /// Returns the current partial (incomplete) line that has not yet been
    /// published because it has no trailing newline.
    pub fn partial_line(&self) -> &str {
        &self.partial_line
    }

    /// Flush the partial line into published lines (useful when streaming ends).
    pub fn flush(&mut self) {
        if !self.partial_line.is_empty() {
            self.published_lines.push(self.partial_line.clone());
            self.partial_line.clear();
            self.buffer.clear();
            self.has_new = true;
        }
    }

    /// Total number of published lines.
    pub fn line_count(&self) -> usize {
        self.published_lines.len()
    }

    /// Clear all content.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.published_lines.clear();
        self.partial_line.clear();
        self.has_new = false;
    }
}

impl Default for StreamingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_buffer() {
        let buf = StreamingBuffer::new();
        assert_eq!(buf.published_text(), "");
        assert_eq!(buf.partial_line(), "");
        assert!(!buf.has_new_content());
        assert_eq!(buf.line_count(), 0);
    }

    #[test]
    fn test_partial_line_not_published() {
        let mut buf = StreamingBuffer::new();
        buf.push("hello world");
        assert_eq!(buf.published_text(), "");
        assert_eq!(buf.partial_line(), "hello world");
        assert!(buf.has_new_content());
        assert_eq!(buf.line_count(), 0);
    }

    #[test]
    fn test_completed_line_published() {
        let mut buf = StreamingBuffer::new();
        buf.push("hello world\n");
        assert_eq!(buf.published_text(), "hello world");
        assert_eq!(buf.partial_line(), "");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_multiple_pushes_accumulate() {
        let mut buf = StreamingBuffer::new();
        buf.push("hello ");
        buf.push("world\n");
        assert_eq!(buf.published_text(), "hello world");
        assert_eq!(buf.partial_line(), "");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_multiple_lines() {
        let mut buf = StreamingBuffer::new();
        buf.push("line1\nline2\nline3");
        assert_eq!(buf.published_text(), "line1\nline2");
        assert_eq!(buf.partial_line(), "line3");
        assert_eq!(buf.line_count(), 2);
    }

    #[test]
    fn test_flush_publishes_partial() {
        let mut buf = StreamingBuffer::new();
        buf.push("partial text");
        assert_eq!(buf.published_text(), "");
        buf.flush();
        assert_eq!(buf.published_text(), "partial text");
        assert_eq!(buf.partial_line(), "");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_flush_empty_partial_is_noop() {
        let mut buf = StreamingBuffer::new();
        buf.push("complete\n");
        buf.flush();
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_clear_resets_everything() {
        let mut buf = StreamingBuffer::new();
        buf.push("line1\nline2\npartial");
        buf.clear();
        assert_eq!(buf.published_text(), "");
        assert_eq!(buf.partial_line(), "");
        assert!(!buf.has_new_content());
        assert_eq!(buf.line_count(), 0);
    }

    #[test]
    fn test_ack_new_content() {
        let mut buf = StreamingBuffer::new();
        buf.push("text");
        assert!(buf.has_new_content());
        buf.ack_new_content();
        assert!(!buf.has_new_content());
    }

    #[test]
    fn test_empty_push_is_noop() {
        let mut buf = StreamingBuffer::new();
        buf.push("hello\n");
        buf.push("");
        assert_eq!(buf.published_text(), "hello");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn test_complex_streaming_scenario() {
        let mut buf = StreamingBuffer::new();

        // Simulate token-by-token streaming
        buf.push("# Research");
        assert_eq!(buf.published_text(), "");
        assert_eq!(buf.partial_line(), "# Research");

        buf.push(" Summary\n\n");
        assert_eq!(buf.published_text(), "# Research Summary\n");
        assert_eq!(buf.partial_line(), "");
        assert_eq!(buf.line_count(), 2); // "# Research Summary" and ""

        buf.push("The topic");
        buf.push(" was researched");
        buf.push(" thoroughly.\n");
        assert_eq!(buf.line_count(), 3);

        buf.push("Key findings");
        buf.flush();
        assert_eq!(buf.line_count(), 4);
    }

    #[test]
    fn test_default_impl() {
        let buf = StreamingBuffer::default();
        assert_eq!(buf.published_text(), "");
    }
}
