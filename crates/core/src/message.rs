use serde::{Deserialize, Serialize};

/// Extended thinking block returned by Anthropic (Claude 3.7 Sonnet) and OpenAI/DeepSeek
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ThinkingBlock {
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl IntoToolArgs,
    ) -> Self {
        Self {
            id: id.into(),
            call_type: "function".into(),
            function: ToolCallFunction {
                name: name.into(),
                arguments: arguments.into_string(),
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.function.name
    }

    pub fn arguments(&self) -> serde_json::Value {
        serde_json::from_str(&self.function.arguments)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
    }
}

pub trait IntoToolArgs {
    fn into_string(self) -> String;
}

impl IntoToolArgs for String {
    fn into_string(self) -> String {
        self
    }
}

impl IntoToolArgs for &str {
    fn into_string(self) -> String {
        self.to_string()
    }
}

impl IntoToolArgs for serde_json::Value {
    fn into_string(self) -> String {
        serde_json::to_string(&self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },

    #[serde(rename = "user")]
    User { content: String },

    #[serde(rename = "assistant")]
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        thinking_blocks: Vec<ThinkingBlock>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },

    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: Some(content.into()),
            thinking_blocks: vec![],
            tool_calls: vec![],
        }
    }

    pub fn assistant_with_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self::Assistant {
            content,
            thinking_blocks: vec![],
            tool_calls,
        }
    }

    pub fn assistant_full(
        content: Option<String>,
        thinking_blocks: Vec<ThinkingBlock>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self::Assistant {
            content,
            thinking_blocks,
            tool_calls,
        }
    }

    pub fn tool(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Tool {
            tool_call_id: call_id.into(),
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_new_sets_function_type() {
        let tc = ToolCall::new("id-1", "shell", "{\"cmd\":\"ls\"}");
        assert_eq!(tc.call_type, "function");
        assert_eq!(tc.name(), "shell");
        assert_eq!(tc.function.arguments, "{\"cmd\":\"ls\"}");
    }

    #[test]
    fn tool_call_arguments_parses_json() {
        let tc = ToolCall::new("id", "t", "{\"a\":1}");
        assert_eq!(tc.arguments(), serde_json::json!({"a": 1}));
    }

    #[test]
    fn tool_call_arguments_invalid_json_is_empty_object() {
        let tc = ToolCall::new("id", "t", "not json {{{");
        assert_eq!(tc.arguments(), serde_json::json!({}));
    }

    #[test]
    fn into_tool_args_variants() {
        assert_eq!("s".into_string(), "s");
        assert_eq!(String::from("owned").into_string(), "owned");
        assert_eq!(serde_json::json!({"k": 2}).into_string(), "{\"k\":2}");
        // Unserializable values fall back to "{}" — NaN is not a valid JSON literal.
        assert_eq!(serde_json::json!(f64::NAN).into_string(), "null");
    }

    #[test]
    fn message_constructors() {
        assert!(matches!(Message::system("s"), Message::System { .. }));
        assert!(matches!(Message::user("u"), Message::User { .. }));
        assert!(matches!(Message::tool("id", "c"), Message::Tool { .. }));
        if let Message::Assistant {
            content,
            tool_calls,
            thinking_blocks,
        } = Message::assistant("hi")
        {
            assert_eq!(content.as_deref(), Some("hi"));
            assert!(tool_calls.is_empty());
            assert!(thinking_blocks.is_empty());
        } else {
            panic!("assistant() produced wrong variant");
        }
    }

    #[test]
    fn assistant_with_tools_keeps_calls() {
        let tc = ToolCall::new("id", "name", "{}");
        let m = Message::assistant_with_tools(None, vec![tc.clone()]);
        if let Message::Assistant {
            tool_calls,
            content,
            ..
        } = m
        {
            assert_eq!(tool_calls.len(), 1);
            assert_eq!(tool_calls[0], tc);
            assert!(content.is_none());
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn message_serde_roundtrip() {
        let m = Message::assistant_full(
            Some("out".into()),
            vec![ThinkingBlock::Thinking {
                thinking: "hmm".into(),
                signature: None,
            }],
            vec![ToolCall::new("id", "tool", "{\"x\":1}")],
        );
        let json = serde_json::to_string(&m).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"type\":\"thinking\""));
    }

    #[test]
    fn tool_result_serde_default_is_error_false() {
        let tr = ToolResult {
            tool_call_id: "id".into(),
            content: "ok".into(),
            is_error: false,
        };
        let back: ToolResult = serde_json::from_str(&serde_json::to_string(&tr).unwrap()).unwrap();
        assert!(!back.is_error);
    }

    #[test]
    fn thinking_block_redacted_variant() {
        let b = ThinkingBlock::RedactedThinking { data: "xyz".into() };
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("redacted_thinking"));
        let back: ThinkingBlock = serde_json::from_str(&json).unwrap();
        assert_eq!(back, b);
    }
}
