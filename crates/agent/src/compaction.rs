use pr_core::{ContextConfig, Message};
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

/// Tokens reserved for the LLM summarization call itself.
#[allow(dead_code)]
const SUMMARIZATION_OVERHEAD_TOKENS: u32 = 4_000;

/// Tokens older than this threshold are eligible for micro-compaction (no LLM).
const MICRO_COMPACT_THRESHOLD_TOKENS: u32 = 40_000;

/// Number of passes that can be ineffective before entering cooldown.
const MAX_INEFFECTIVE_PASSES: u32 = 2;

/// Cooldown duration after too many ineffective compaction passes.
const COOLDOWN_DURATION: Duration = Duration::from_secs(300);

/// Hysteresis region (necessity vs utility): after a compaction pass that
/// freed nothing useful, further passes are suppressed until the transcript
/// grows back by this factor (or a floor number of rounds pass). Prevents
/// re-running compaction round-after-round and losing provider cache in a
/// window that compaction can never fit.
const HYSTERESIS_GROWTH: f32 = 1.2;
/// Minimum reduction for a pass to count as "useful" (else hysteresis engages).
const USEFUL_REDUCTION_RATIO: f32 = 0.05;

/// Result of a compaction pass.
#[derive(Debug)]
pub struct CompactionResult {
    /// The compacted messages.
    pub messages: Vec<Message>,
    /// Tokens before compaction.
    pub tokens_before: u32,
    /// Tokens after compaction.
    pub tokens_after: u32,
    /// Whether a cooldown was triggered.
    pub cooldown_triggered: bool,
    /// Number of tool messages pruned by micro-compaction.
    pub micro_pruned: u32,
    /// Whether LLM summarization was used.
    pub used_llm: bool,
}

/// Engine that manages context compaction state across the agent lifecycle.
pub struct CompactionEngine {
    /// Number of consecutive ineffective passes (reduced < 5% of tokens).
    ineffective_passes: u32,
    /// When cooldown was triggered (None = not in cooldown).
    cooldown_until: Option<Instant>,
    /// Running token estimate.
    estimated_tokens: u32,
    /// Configuration.
    config: ContextConfig,
    /// Bytes of transcript before which compaction is suppressed (hysteresis):
    /// last pass failed to help, so we wait for meaningful growth.
    hysteresis_until_tokens: u32,
}

impl CompactionEngine {
    pub fn new(config: ContextConfig) -> Self {
        Self {
            ineffective_passes: 0,
            cooldown_until: None,
            estimated_tokens: 0,
            config,
            hysteresis_until_tokens: 0,
        }
    }

    /// Update the running token estimate (call after each message addition).
    pub fn set_estimated_tokens(&mut self, tokens: u32) {
        self.estimated_tokens = tokens;
    }

    /// Current estimated tokens.
    pub fn estimated_tokens(&self) -> u32 {
        self.estimated_tokens
    }

    /// Check whether compaction should be triggered.
    ///
    /// Hybrid necessity-vs-utility gate (ouroboros `context_budget` inspiration):
    /// besides the token threshold, a pass is **suppressed** while the transcript
    /// is still inside the hysteresis region left by a previous *useless* pass —
    /// re-running compaction on a frame that it already failed to shrink would
    /// only burn LLM calls and lose provider cache for zero gain.
    pub fn should_compact(&self) -> bool {
        let window = self.config.resolved_window();
        let threshold = (window as f32 * self.config.compact_threshold) as u32;
        if self.estimated_tokens < threshold {
            return false;
        }
        if self.estimated_tokens < self.hysteresis_until_tokens {
            // Still inside the suppressed region; wait for growth.
            return false;
        }
        true
    }

    /// Engage the hysteresis region after a pass that did not meaningfully
    /// help: suppress further compaction until the transcript grows back.
    pub fn suppress_compaction(&mut self, current_tokens: u32) {
        self.hysteresis_until_tokens =
            (current_tokens as f32 * HYSTERESIS_GROWTH).ceil() as u32;
    }

    /// Whether we are currently in a cooldown period.
    pub fn is_in_cooldown(&self) -> bool {
        if let Some(until) = self.cooldown_until {
            if Instant::now() < until {
                return true;
            }
            // Cooldown expired.
        }
        false
    }

    /// Run micro-compaction: prune old tool outputs that exceed the token threshold.
    /// This does NOT use the LLM — it simply replaces old tool message content with a
    /// one-line summary.
    ///
    /// Returns the number of messages pruned.
    pub fn micro_compact(&self, messages: &mut Vec<Message>) -> u32 {
        let mut pruned = 0u32;
        let mut running_tokens: u32 = 0;
        let mut seen_content_hashes: HashSet<u64> = HashSet::new();

        // Walk messages from oldest to newest, tracking cumulative tokens.
        for msg in messages.iter_mut() {
            if let Message::Tool { content, .. } = msg {
                let tokens = pr_core::estimate_tokens(content);
                running_tokens += tokens;

                // Dedup: if we've seen identical content before, collapse to one line.
                let hash = content_hash(content);
                if !seen_content_hashes.insert(hash) {
                    let summary = format!("[Duplicate tool result — {} bytes, {} tokens]",
                        content.len(), tokens);
                    *content = summary;
                    pruned += 1;
                    continue;
                }

                // Prune old tool outputs beyond the micro-compact threshold.
                if running_tokens > MICRO_COMPACT_THRESHOLD_TOKENS && tokens > 100 {
                    let summary = format!(
                        "[Tool output pruned — {} bytes, {} tokens. Original output was from an earlier conversation turn.]",
                        content.len(), tokens
                    );
                    *content = summary;
                    pruned += 1;
                }
            }
        }

        pruned
    }

    /// Full compaction pass. Returns the compacted messages and a result summary.
    ///
    /// `summarize_fn` is an async closure that takes a list of messages and returns
    /// a summary string. This allows the caller to use whichever LLM provider is available.
    pub async fn compact<F, Fut>(
        &mut self,
        messages: &mut Vec<Message>,
        summarize_fn: F,
    ) -> anyhow::Result<CompactionResult>
    where
        F: FnOnce(Vec<Message>) -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<String>>,
    {
        let tokens_before = pr_core::estimate_messages_tokens(messages);

        // Check cooldown.
        if self.is_in_cooldown() {
            return Ok(CompactionResult {
                messages: messages.clone(),
                tokens_before,
                tokens_after: tokens_before,
                cooldown_triggered: true,
                micro_pruned: 0,
                used_llm: false,
            });
        }

        // Phase 1: Micro-compaction (prune old tool outputs, dedup).
        let micro_pruned = self.micro_compact(messages);

        // If micro-compaction was sufficient (brought us below threshold), done.
        let after_micro = pr_core::estimate_messages_tokens(messages);
        let window = self.config.resolved_window();
        let threshold = (window as f32 * self.config.compact_threshold) as u32;
        if after_micro < threshold {
            self.update_effectiveness(tokens_before, after_micro);
            return Ok(CompactionResult {
                messages: messages.clone(),
                tokens_before,
                tokens_after: after_micro,
                cooldown_triggered: false,
                micro_pruned,
                used_llm: false,
            });
        }

        // Phase 2: Split into head, middle, tail.
        let (head, middle, tail) = split_head_middle_tail(messages);

        // Phase 3: Summarize the middle section via LLM.
        let summary_text = if !middle.is_empty() {
            // Build a summarization prompt.
            let middle_text = messages_to_text(&middle);
            let prompt = vec![
                Message::system(SUMMARIZE_SYSTEM_PROMPT),
                Message::user(format!(
                    "Summarize the following conversation section concisely. \
                     Preserve all key findings, decisions, and open questions.\n\n{}",
                    middle_text
                )),
            ];
            summarize_fn(prompt).await.unwrap_or_else(|e| {
                format!("[Compaction summarization failed: {}. Middle section removed.]", e)
            })
        } else {
            String::new()
        };

        // Phase 4: Reassemble: [head] + [summary message] + [tail].
        let mut compacted = Vec::with_capacity(head.len() + 2 + tail.len());
        compacted.extend(head);

        if !summary_text.is_empty() {
            compacted.push(Message::system(format!(
                "[Context compaction — previous conversation summarized]\n\n{}",
                summary_text
            )));
        }

        compacted.extend(tail);

        let tokens_after = pr_core::estimate_messages_tokens(&compacted);
        self.update_effectiveness(tokens_before, tokens_after);

        Ok(CompactionResult {
            messages: compacted,
            tokens_before,
            tokens_after,
            cooldown_triggered: false,
            micro_pruned,
            used_llm: !summary_text.is_empty(),
        })
    }

    /// Track whether the compaction pass was effective.
    fn update_effectiveness(&mut self, before: u32, after: u32) {
        let reduction = before.saturating_sub(after);
        let minimum_reduction = (before as f32 * USEFUL_REDUCTION_RATIO) as u32;

        if reduction < minimum_reduction {
            self.ineffective_passes += 1;
            // Hysteresis: a useless pass suppresses further passes until the
            // transcript grows back, so we don't re-run compaction on a frame
            // it already failed to shrink.
            self.suppress_compaction(after);
            if self.ineffective_passes >= MAX_INEFFECTIVE_PASSES {
                self.cooldown_until = Some(Instant::now() + COOLDOWN_DURATION);
                self.ineffective_passes = 0;
            }
        } else {
            self.ineffective_passes = 0;
            self.cooldown_until = None;
            // A useful pass clears any hysteresis suppression.
            self.hysteresis_until_tokens = 0;
        }
    }

    /// Reset the engine state (e.g. when starting a new turn).
    pub fn reset_cooldown(&mut self) {
        self.ineffective_passes = 0;
        self.cooldown_until = None;
    }
}

/// System prompt used when asking the LLM to summarize a conversation section.
const SUMMARIZE_SYSTEM_PROMPT: &str = "\
You are a context compaction assistant. Your job is to produce a concise structured summary \
of a conversation section. Use the following format:

## Goal
<what the agent was trying to accomplish>

## Done
<key findings, results, and completed actions>

## Blocked
<any errors, blockers, or unresolved issues>

## Next
<planned next steps>

Be concise. Preserve factual details, URLs, and numeric values. Remove conversational filler.";

/// Split messages into (head, middle, tail).
///
/// - **Head**: system prompt + first 3 messages.
/// - **Tail**: last 2 user/assistant turn pairs (up to 4 messages).
/// - **Middle**: everything in between.
fn split_head_middle_tail(messages: &[Message]) -> (Vec<Message>, Vec<Message>, Vec<Message>) {
    if messages.len() <= 7 {
        // Too short to split meaningfully — return everything as head.
        return (messages.to_vec(), vec![], vec![]);
    }

    // Head: first 4 messages (system + first 3).
    let mut head_end = 4.min(messages.len());

    // Tail: last 4 messages (approx 2 user/assistant turns).
    let mut tail_start = messages.len().saturating_sub(4);

    // Tool-group safety: an assistant message with tool_calls and the tool
    // result messages that follow it MUST stay in the same segment — the
    // OpenAI-compatible API rejects histories where the pair is split.
    // Pull leading tool results into the head...
    while head_end < messages.len() && is_tool_message(&messages[head_end]) {
        head_end += 1;
    }
    // ...and push the tail start back over any tool results so they stay
    // with their assistant call in the middle.
    while tail_start > head_end && is_tool_message(&messages[tail_start]) {
        tail_start -= 1;
    }
    if tail_start <= head_end {
        // Degenerate overlap — keep everything in head/tail, no middle.
        return (messages[..head_end].to_vec(), vec![], messages[head_end..].to_vec());
    }

    let head = messages[..head_end].to_vec();
    let tail = messages[tail_start..].to_vec();
    let middle = messages[head_end..tail_start].to_vec();

    (head, middle, tail)
}

fn is_tool_message(msg: &Message) -> bool {
    matches!(msg, Message::Tool { .. })
}

/// Convert a slice of messages into a readable text block for summarization.
fn messages_to_text(messages: &[Message]) -> String {
    let mut out = String::new();
    for msg in messages {
        match msg {
            Message::System { content } => {
                out.push_str(&format!("[system]: {}\n\n", content));
            }
            Message::User { content } => {
                out.push_str(&format!("[user]: {}\n\n", content));
            }
            Message::Assistant { content, tool_calls, .. } => {
                if let Some(text) = content {
                    out.push_str(&format!("[assistant]: {}\n\n", text));
                }
                for tc in tool_calls {
                    out.push_str(&format!(
                        "[assistant tool_call]: {}({})\n\n",
                        tc.name(),
                        tc.function.arguments,
                    ));
                }
            }
            Message::Tool { tool_call_id, content } => {
                // For summarization, truncate very long tool outputs
                // (char-boundary safe for multi-byte UTF-8).
                let truncated = if content.len() > 2000 {
                    let mut end = 2000;
                    while end > 0 && !content.is_char_boundary(end) {
                        end -= 1;
                    }
                    format!("{}... [truncated for summary]", &content[..end])
                } else {
                    content.clone()
                };
                out.push_str(&format!(
                    "[tool result (id={})]: {}\n\n",
                    tool_call_id, truncated,
                ));
            }
        }
    }
    out
}

/// Compute a fast content hash for deduplication.
fn content_hash(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(n: usize) -> Vec<Message> {
        let mut msgs = vec![Message::system("You are a research agent.")];
        for i in 0..n {
            msgs.push(Message::user(format!("Question {}", i)));
            msgs.push(Message::assistant(format!("Answer {}", i)));
            msgs.push(Message::tool(
                format!("call_{}", i),
                format!("Tool result for question {} with some data.", i),
            ));
        }
        msgs
    }

    #[test]
    fn test_split_head_middle_tail_short() {
        let msgs = make_messages(1); // 4 messages total
        let (head, middle, tail) = split_head_middle_tail(&msgs);
        assert_eq!(head.len(), 4);
        assert!(middle.is_empty());
        assert!(tail.is_empty());
    }

    #[test]
    fn test_split_head_middle_tail_long() {
        let msgs = make_messages(10); // 31 messages
        let (head, middle, tail) = split_head_middle_tail(&msgs);
        assert_eq!(head.len(), 4);
        // Tail boundary is rewound over tool results so no segment starts
        // with an orphaned tool message (assistant call + results stay
        // together) — hence 5 instead of a fixed 4.
        assert_eq!(tail.len(), 5);
        assert_eq!(middle.len(), 31 - 4 - 5);
        assert_eq!(head.len() + middle.len() + tail.len(), msgs.len());
        for seg in [&head, &middle, &tail] {
            assert!(
                !matches!(seg.first(), Some(Message::Tool { .. })),
                "no segment may start with an orphaned tool result"
            );
        }
    }

    #[test]
    fn test_micro_compact_dedup() {
        let mut msgs = vec![
            Message::system("sys"),
            Message::user("q1"),
            Message::tool("c1", "identical content"),
            Message::tool("c2", "identical content"),
            Message::tool("c3", "different content"),
        ];
        let engine = CompactionEngine::new(ContextConfig::default());
        let pruned = engine.micro_compact(&mut msgs);
        assert_eq!(pruned, 1); // c2 is a duplicate

        // c2 should now be a summary.
        if let Message::Tool { content, .. } = &msgs[3] {
            assert!(content.contains("Duplicate tool result"));
        } else {
            panic!("expected tool message");
        }
    }

    #[test]
    fn test_should_compact() {
        let config = ContextConfig {
            context_window: 100_000,
            compact_threshold: 0.50,
            ..Default::default()
        };
        let mut engine = CompactionEngine::new(config);

        engine.set_estimated_tokens(40_000);
        assert!(!engine.should_compact());

        engine.set_estimated_tokens(60_000);
        assert!(engine.should_compact());
    }

    #[test]
    fn test_cooldown_trigger() {
        let config = ContextConfig::default();
        let mut engine = CompactionEngine::new(config);

        // Simulate 2 ineffective passes.
        engine.update_effectiveness(1000, 990); // < 5% reduction
        assert!(!engine.is_in_cooldown());
        engine.update_effectiveness(1000, 990);
        assert!(engine.is_in_cooldown());
    }

    #[test]
    fn test_hysteresis_suppresses_repeated_compaction() {
        // Window at the safety floor; threshold = half of it.
        let config = ContextConfig {
            context_window: 32_000,
            ..Default::default()
        };
        let mut engine = CompactionEngine::new(config);

        // Frame over threshold → should compact initially.
        engine.set_estimated_tokens(20_000);
        assert!(engine.should_compact());

        // A useless pass (reduction < 5%) engages hysteresis: further passes
        // are suppressed until the transcript grows back ~20%.
        engine.update_effectiveness(20_000, 19_900);
        engine.set_estimated_tokens(20_000);
        assert_eq!(
            engine.hysteresis_until_tokens,
            (19_900.0 * HYSTERESIS_GROWTH).ceil() as u32
        );
        assert!(
            !engine.should_compact(),
            "should be suppressed inside hysteresis region"
        );

        // Once it grows back past the region, compaction is allowed again.
        engine.set_estimated_tokens(25_000);
        assert!(engine.should_compact());
    }

    #[test]
    fn test_effectiveness_reset() {
        let config = ContextConfig::default();
        let mut engine = CompactionEngine::new(config);

        engine.update_effectiveness(1000, 990);
        engine.update_effectiveness(1000, 990);
        assert!(engine.is_in_cooldown());

        engine.reset_cooldown();
        assert!(!engine.is_in_cooldown());
    }

    #[test]
    fn test_content_hash_dedup() {
        let h1 = content_hash("hello");
        let h2 = content_hash("hello");
        let h3 = content_hash("world");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_messages_to_text() {
        let msgs = vec![
            Message::user("What is Rust?"),
            Message::assistant("A systems language."),
        ];
        let text = messages_to_text(&msgs);
        assert!(text.contains("[user]: What is Rust?"));
        assert!(text.contains("[assistant]: A systems language."));
    }

    #[test]
    fn test_summarize_prompt_structure() {
        // Verify the system prompt contains the expected sections.
        assert!(SUMMARIZE_SYSTEM_PROMPT.contains("Goal"));
        assert!(SUMMARIZE_SYSTEM_PROMPT.contains("Done"));
        assert!(SUMMARIZE_SYSTEM_PROMPT.contains("Blocked"));
        assert!(SUMMARIZE_SYSTEM_PROMPT.contains("Next"));
    }
}
