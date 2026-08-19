use crate::message::{Message, ToolCall};
use std::sync::OnceLock;

/// Flat token cost for image content (conservative estimate).
#[allow(dead_code)]
const IMAGE_TOKEN_COST: u32 = 1500;

/// Overhead tokens for a message envelope (role, separators, etc.).
const MESSAGE_OVERHEAD: u32 = 4;

/// Overhead tokens for a tool call entry (JSON structure, id, function wrapper).
const TOOL_CALL_OVERHEAD: u32 = 10;

/// Lazily-initialized cl100k_base BPE tokenizer (the GPT-4 family encoding;
/// DeepSeek and other OpenAI-compatible models are close enough for budget
/// accounting). Loaded once per process; the ~2 MB vocabulary is embedded
/// in the binary, so no network/filesystem access is needed.
fn bpe() -> Option<&'static tiktoken_rs::CoreBPE> {
    static BPE: OnceLock<Option<tiktoken_rs::CoreBPE>> = OnceLock::new();
    BPE.get_or_init(|| match tiktoken_rs::cl100k_base() {
        Ok(bpe) => Some(bpe),
        Err(e) => {
            // Practically unreachable (embedded data), but counting must
            // never panic — the heuristic below takes over.
            eprintln!("warning: tiktoken unavailable ({e}); using heuristic token counts");
            None
        }
    })
    .as_ref()
}

/// Count tokens for a plain text string with the real BPE tokenizer
/// (falls back to a per-script heuristic if tiktoken is unavailable).
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    if let Some(bpe) = bpe() {
        return bpe.encode_ordinary(text).len() as u32;
    }
    heuristic_tokens(text)
}

/// Per-character token cost (fallback heuristic).
fn char_cost(ch: char) -> u32 {
    let cp = ch as u32;
    match cp {
        // ASCII printable + whitespace: 0.25 tokens per char (4 chars = 1 token)
        // We accumulate fractional tokens by counting chars and dividing at the end.
        // For simplicity, we track ASCII chars separately.
        0x0000..=0x007F => 0, // handled in bulk below
        // CJK Unified Ideographs
        0x4E00..=0x9FFF => 1,
        // CJK Unified Ideographs Extension A
        0x3400..=0x4DBF => 1,
        // CJK Compatibility Ideographs
        0xF900..=0xFAFF => 1,
        // CJK Unified Ideographs Extension B
        0x20000..=0x2A6DF => 1,
        // Hangul Syllables
        0xAC00..=0xD7AF => 1,
        // Hangul Jamo
        0x1100..=0x11FF => 1,
        // Hiragana
        0x3040..=0x309F => 1,
        // Katakana
        0x30A0..=0x30FF => 1,
        // Katakana Phonetic Extensions
        0x31F0..=0x31FF => 1,
        // CJK Radicals / Kangxi Radicals
        0x2E80..=0x2FDF => 1,
        // CJK Symbols and Punctuation
        0x3000..=0x303F => 1,
        // Everything else (Cyrillic, Arabic, emoji, etc.): 1 token per char
        _ => 1,
    }
}

/// Fallback estimate that separates ASCII and non-ASCII portions.
fn heuristic_tokens(text: &str) -> u32 {
    let mut ascii_count: u32 = 0;
    let mut other_tokens: u32 = 0;

    for ch in text.chars() {
        let cp = ch as u32;
        if cp <= 0x007F {
            ascii_count += 1;
        } else {
            other_tokens += char_cost(ch);
        }
    }

    // 4 ASCII chars ~ 1 token (round up)
    let ascii_tokens = (ascii_count + 3) / 4;
    ascii_tokens + other_tokens
}

/// Estimate the total token cost of an OpenAI-compatible message.
///
/// Counts:
/// - The textual content of the message
/// - Any tool call JSON structures (name + arguments)
/// - A fixed envelope overhead per message
pub fn estimate_message_tokens(msg: &Message) -> u32 {
    let mut total = MESSAGE_OVERHEAD;

    match msg {
        Message::System { content } => {
            total += estimate_tokens(content);
        }
        Message::User { content } => {
            total += estimate_tokens(content);
        }
        Message::Assistant { content, tool_calls } => {
            if let Some(text) = content {
                total += estimate_tokens(text);
            }
            for tc in tool_calls {
                total += estimate_tool_call_tokens(tc);
            }
        }
        Message::Tool { tool_call_id, content } => {
            // Tool call ID costs a few tokens
            total += estimate_tokens(tool_call_id);
            total += estimate_tokens(content);
        }
    }

    total
}

/// Estimate the token cost of a single tool call entry.
fn estimate_tool_call_tokens(tc: &ToolCall) -> u32 {
    let mut total = TOOL_CALL_OVERHEAD;
    // Tool call ID
    total += estimate_tokens(&tc.id);
    // Function name
    total += estimate_tokens(&tc.function.name);
    // Arguments (JSON string)
    total += estimate_tokens(&tc.function.arguments);
    total
}

/// Estimate total tokens for a slice of messages.
pub fn estimate_messages_tokens(messages: &[Message]) -> u32 {
    messages.iter().map(|m| estimate_message_tokens(m)).sum()
}

/// Estimate the token cost of tool schemas (for the tools parameter in the API).
/// Each schema contributes roughly its JSON representation size.
pub fn estimate_schemas_tokens(schemas: &[crate::tool::ToolSchema]) -> u32 {
    schemas
        .iter()
        .map(|s| {
            let json = serde_json::to_string(s).unwrap_or_default();
            estimate_tokens(&json)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ToolCall, ToolCallFunction};

    #[test]
    fn bpe_tokenizer_is_available() {
        // The embedded cl100k vocabulary must load; otherwise every count
        // silently degrades to the heuristic and budgets drift.
        assert!(bpe().is_some(), "tiktoken cl100k_base must be available");
    }

    #[test]
    fn test_ascii_token_estimate() {
        // BPE: common words are single tokens.
        assert_eq!(estimate_tokens("hello"), 1);
        // Digits split into ~3-digit chunks in cl100k.
        let digits = estimate_tokens("12345678");
        assert!((2..=4).contains(&digits), "got {digits}");
        // Longer text: roughly 1 token per 4 chars, allow BPE variance.
        let text = "The quick brown fox jumps over the lazy dog";
        let n = estimate_tokens(text);
        assert!((6..=14).contains(&n), "got {n}");
    }

    #[test]
    fn test_cjk_tokens() {
        // CJK characters are never cheaper than ~1 token each.
        let n = estimate_tokens("你好世界");
        assert!(n >= 4, "got {n}");
    }

    #[test]
    fn test_kana_tokens() {
        // cl100k has whole common greetings as single tokens (こんにちは = 1);
        // the contract is simply "non-empty text costs at least one token".
        assert!(estimate_tokens("こんにちは") >= 1);
        // A less common kana string splits into several tokens.
        assert!(estimate_tokens("カタカナモジレツ") >= 2);
    }

    #[test]
    fn test_hangul_tokens() {
        assert!(estimate_tokens("안녕하세요") >= 2);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_single_char() {
        assert_eq!(estimate_tokens("a"), 1);
    }

    #[test]
    fn test_system_message() {
        let msg = Message::system("You are a helpful assistant.");
        let tokens = estimate_message_tokens(&msg);
        // ~6 BPE tokens + 4 envelope overhead.
        assert!(tokens >= 8, "got {tokens}");
        assert!(tokens <= 20, "got {tokens}");
    }

    #[test]
    fn test_assistant_with_tool_calls() {
        let tc = ToolCall {
            id: "call_123".to_string(),
            call_type: "function".to_string(),
            function: ToolCallFunction {
                name: "web_search".to_string(),
                arguments: r#"{"query":"rust programming"}"#.to_string(),
            },
        };
        let msg = Message::assistant_with_tools(Some("Let me search.".to_string()), vec![tc]);
        let tokens = estimate_message_tokens(&msg);
        // Should be meaningfully more than just the text
        assert!(tokens > 10);
    }

    #[test]
    fn test_tool_message() {
        let msg = Message::tool("call_123", "Search results: ...");
        let tokens = estimate_message_tokens(&msg);
        assert!(tokens > 4); // overhead + some content
    }

    #[test]
    fn test_image_token_cost() {
        // IMAGE_TOKEN_COST is a flat constant used elsewhere (e.g. in content blocks).
        // We verify it's reasonable.
        assert_eq!(IMAGE_TOKEN_COST, 1500);
    }

    #[test]
    fn test_bulk_messages() {
        let messages = vec![
            Message::system("System prompt"),
            Message::user("What is Rust?"),
            Message::assistant("Rust is a systems programming language."),
        ];
        let total = estimate_messages_tokens(&messages);
        assert!(total > 15);
    }

    #[test]
    fn heuristic_bounds_bpe_for_ascii() {
        // The fallback heuristic (chars/4) must stay within a sane factor
        // of the true BPE count on English prose — it is the degradation
        // path, not a wild guess.
        let text = "Autonomous research agents decompose goals into sub-tasks \
                    and synthesize structured reports from web findings.";
        let real = estimate_tokens(text);
        let approx = heuristic_tokens(text);
        let ratio = approx as f64 / real as f64;
        assert!(ratio > 0.5 && ratio < 2.0, "ratio {ratio}");
    }
}

