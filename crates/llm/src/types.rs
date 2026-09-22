use futures::Stream;
use pr_core::{Message, ToolSchema};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

impl Usage {
    pub fn simple(prompt: u32, completion: u32, total: u32) -> Self {
        Self {
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message: Message,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamChunk {
    #[serde(rename = "text")]
    Text { delta: String },
    #[serde(rename = "reasoning")]
    Reasoning { delta: String },
    #[serde(rename = "tool_call")]
    ToolCallDelta {
        #[serde(default)]
        index: usize,
        id: String,
        name: String,
        arguments_delta: String,
    },
    #[serde(rename = "done")]
    Done {
        message: Message,
        usage: Option<Usage>,
        finish_reason: Option<String>,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

pub type ResponseStream = Pin<Box<dyn Stream<Item = anyhow::Result<StreamChunk>> + Send>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_simple_sets_fields() {
        let u = Usage::simple(10, 5, 15);
        assert_eq!(u.prompt_tokens, 10);
        assert_eq!(u.completion_tokens, 5);
        assert_eq!(u.total_tokens, 15);
        assert!(u.cache_creation_input_tokens.is_none());
        assert!(u.cache_read_input_tokens.is_none());
    }

    #[test]
    fn usage_skips_none_cache_fields() {
        let u = Usage::simple(1, 2, 3);
        let json = serde_json::to_string(&u).unwrap();
        assert!(!json.contains("cache_creation_input_tokens"));
        assert!(!json.contains("cache_read_input_tokens"));
    }

    #[test]
    fn usage_keeps_some_cache_fields() {
        let mut u = Usage::simple(1, 2, 3);
        u.cache_read_input_tokens = Some(7);
        let json = serde_json::to_string(&u).unwrap();
        assert!(json.contains("\"cache_read_input_tokens\":7"));
    }

    #[test]
    fn stream_chunk_text_tag() {
        let c = StreamChunk::Text { delta: "hi".into() };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        matches!(back, StreamChunk::Text { delta } if delta == "hi");
    }

    #[test]
    fn stream_chunk_tool_call_tag_and_default_index() {
        let json = r#"{"type":"tool_call","id":"c1","name":"shell","arguments_delta":"{}"}"#;
        let c: StreamChunk = serde_json::from_str(json).unwrap();
        if let StreamChunk::ToolCallDelta {
            index, id, name, ..
        } = c
        {
            assert_eq!(index, 0); // default index
            assert_eq!(id, "c1");
            assert_eq!(name, "shell");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn stream_chunk_done_roundtrip() {
        let c = StreamChunk::Done {
            message: Message::assistant("done"),
            usage: Some(Usage::simple(1, 2, 3)),
            finish_reason: Some("stop".into()),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"type\":\"done\""));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        matches!(back, StreamChunk::Done { .. });
    }

    #[test]
    fn stream_chunk_error_variant() {
        let c = StreamChunk::Error {
            message: "boom".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"type\":\"error\""));
    }

    #[test]
    fn stream_chunk_reasoning_variant() {
        let c = StreamChunk::Reasoning {
            delta: "thinking".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("\"type\":\"reasoning\""));
    }

    #[test]
    fn completion_request_serde_roundtrip() {
        let r = CompletionRequest {
            messages: vec![Message::user("hi")],
            tools: vec![],
            temperature: Some(0.5),
            max_tokens: Some(100),
            stream: false,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: CompletionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.max_tokens, Some(100));
        assert!(!back.stream);
    }
}
