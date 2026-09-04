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

