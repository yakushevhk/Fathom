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
