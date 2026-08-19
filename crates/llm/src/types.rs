use pr_core::{Message, ToolSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolSchema>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message: Message,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamChunk {
    #[serde(rename = "text")]
    Text { delta: String },
    #[serde(rename = "tool_call")]
    ToolCallDelta {
        /// Position of the tool call within the response. The OpenAI
        /// streaming protocol sends `id`/`name` only in the FIRST delta of
        /// each index — subsequent argument fragments carry the same index,
        /// so it is the correlation key for reassembly.
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

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::Message;

    // -----------------------------------------------------------------------
    // Usage
    // -----------------------------------------------------------------------

    #[test]
    fn usage_construction() {
        let u = Usage { prompt_tokens: 10, completion_tokens: 20, total_tokens: 30 };
        assert_eq!(u.prompt_tokens, 10);
        assert_eq!(u.completion_tokens, 20);
        assert_eq!(u.total_tokens, 30);
    }

    #[test]
    fn usage_serde_roundtrip() {
        let u = Usage { prompt_tokens: 5, completion_tokens: 15, total_tokens: 20 };
        let json = serde_json::to_string(&u).unwrap();
        let back: Usage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.prompt_tokens, 5);
        assert_eq!(back.completion_tokens, 15);
        assert_eq!(back.total_tokens, 20);
    }

    // -----------------------------------------------------------------------
    // CompletionRequest
    // -----------------------------------------------------------------------

    #[test]
    fn completion_request_minimal() {
        let req = CompletionRequest {
            messages: vec![Message::user("hello")],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        assert_eq!(req.messages.len(), 1);
        assert!(req.tools.is_empty());
        assert!(!req.stream);
    }

    #[test]
    fn completion_request_serde_roundtrip() {
        let req = CompletionRequest {
            messages: vec![Message::system("be brief"), Message::user("hi")],
            tools: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: CompletionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.messages.len(), 2);
        assert_eq!(back.temperature, Some(0.7));
        assert_eq!(back.max_tokens, Some(100));
        assert!(back.stream);
    }

    #[test]
    fn completion_request_defaults_omitted() {
        let req = CompletionRequest {
            messages: vec![Message::user("x")],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        // stream is false so it serializes as `false`
        assert!(json.contains("\"stream\":false"));
        // empty tools and None optionals should be absent
        assert!(!json.contains("\"tools\""));
        assert!(!json.contains("\"temperature\""));
        assert!(!json.contains("\"max_tokens\""));
    }

    #[test]
    fn completion_request_skips_empty_tools() {
        let req = CompletionRequest {
            messages: vec![Message::user("x")],
            tools: vec![],
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"tools\""));
    }

    #[test]
    fn completion_request_with_tools() {
        let schema = ToolSchema {
            name: "search".into(),
            description: "Search the web".into(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let req = CompletionRequest {
            messages: vec![Message::user("search something")],
            tools: vec![schema],
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"tools\""));
        assert!(json.contains("search"));
    }

    // -----------------------------------------------------------------------
    // CompletionResponse
    // -----------------------------------------------------------------------

    #[test]
    fn completion_response_serde_roundtrip() {
        let resp = CompletionResponse {
            message: Message::assistant("hello"),
            usage: Some(Usage { prompt_tokens: 5, completion_tokens: 10, total_tokens: 15 }),
            finish_reason: Some("stop".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert_eq!(back.finish_reason.as_deref(), Some("stop"));
        assert_eq!(back.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn completion_response_no_usage() {
        let resp = CompletionResponse {
            message: Message::assistant("ok"),
            usage: None,
            finish_reason: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert!(back.usage.is_none());
        assert!(back.finish_reason.is_none());
    }

    // -----------------------------------------------------------------------
    // StreamChunk
    // -----------------------------------------------------------------------

    #[test]
    fn stream_chunk_text_serde() {
        let chunk = StreamChunk::Text { delta: "Hello".into() };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("Hello"));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        match back {
            StreamChunk::Text { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn stream_chunk_tool_call_serde() {
        let chunk = StreamChunk::ToolCallDelta {
            index: 0,
            id: "call_1".into(),
            name: "search".into(),
            arguments_delta: "{\"q\":\"test\"".into(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"type\":\"tool_call\""));
        assert!(json.contains("call_1"));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        match back {
            StreamChunk::ToolCallDelta { index, id, name, .. } => {
                assert_eq!(index, 0);
                assert_eq!(id, "call_1");
                assert_eq!(name, "search");
            }
            _ => panic!("expected ToolCallDelta variant"),
        }
    }

    #[test]
    fn stream_chunk_done_serde() {
        let chunk = StreamChunk::Done {
            message: Message::assistant("done"),
            usage: Some(Usage { prompt_tokens: 1, completion_tokens: 2, total_tokens: 3 }),
            finish_reason: Some("stop".into()),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"type\":\"done\""));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        match back {
            StreamChunk::Done { message: _, usage, finish_reason } => {
                assert_eq!(usage.as_ref().unwrap().total_tokens, 3);
                assert_eq!(finish_reason.as_deref(), Some("stop"));
            }
            _ => panic!("expected Done variant"),
        }
    }

    #[test]
    fn stream_chunk_error_serde() {
        let chunk = StreamChunk::Error { message: "rate limited".into() };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        match back {
            StreamChunk::Error { message } => assert_eq!(message, "rate limited"),
            _ => panic!("expected Error variant"),
        }
    }

    #[test]
    fn stream_chunk_done_no_usage() {
        let chunk = StreamChunk::Done {
            message: Message::assistant(""),
            usage: None,
            finish_reason: None,
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let back: StreamChunk = serde_json::from_str(&json).unwrap();
        match back {
            StreamChunk::Done { usage, finish_reason, .. } => {
                assert!(usage.is_none());
                assert!(finish_reason.is_none());
            }
            _ => panic!("expected Done variant"),
        }
    }

    #[test]
    fn stream_chunk_tool_call_default_index_zero() {
        // index has #[serde(default)] so it should deserialize even when absent
        let json = r#"{"type":"tool_call","id":"c1","name":"search","arguments_delta":"{}"}"#;
        let chunk: StreamChunk = serde_json::from_str(json).unwrap();
        match chunk {
            StreamChunk::ToolCallDelta { index, .. } => assert_eq!(index, 0),
            _ => panic!("expected ToolCallDelta"),
        }
    }

    // -----------------------------------------------------------------------
    // Proptest: serde roundtrip for all chunks
    // -----------------------------------------------------------------------
    proptest::proptest! {
        #[test]
        fn usage_proptest_serde(prompt_tokens: u32, completion_tokens: u32) {
            let total = prompt_tokens.saturating_add(completion_tokens);
            let u = Usage { prompt_tokens, completion_tokens, total_tokens: total };
            let json = serde_json::to_string(&u).unwrap();
            let back: Usage = serde_json::from_str(&json).unwrap();
            assert_eq!(back.prompt_tokens, prompt_tokens);
            assert_eq!(back.completion_tokens, completion_tokens);
            assert_eq!(back.total_tokens, total);
        }
    }
}
