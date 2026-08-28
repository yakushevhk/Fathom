use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use futures::Stream;
use pr_core::{PrError, PrResult};
use crate::provider::LlmProvider;
use crate::types::{CompletionRequest, CompletionResponse, StreamChunk, Usage};

/// Native Anthropic Messages API client supporting Ephemeral Prompt Caching and Extended Thinking.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
    thinking_budget: Option<u32>,
    prompt_caching: bool,
}

impl AnthropicProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            client,
            thinking_budget: None,
            prompt_caching: true,
        }
    }

    pub fn with_thinking_budget(mut self, budget: u32) -> Self {
        self.thinking_budget = Some(budget);
        self
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> {
        let url = format!("{}/v1/messages", self.base_url);

        let mut system_blocks = Vec::new();
        let mut messages = Vec::new();

        for msg in &req.messages {
            match msg {
                pr_core::Message::System { content } => {
                    let mut block = serde_json::json!({
                        "type": "text",
                        "text": content
                    });
                    if self.prompt_caching {
                        block["cache_control"] = serde_json::json!({ "type": "ephemeral" });
                    }
                    system_blocks.push(block);
                }
                pr_core::Message::User { content } => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": content
                    }));
                }
                pr_core::Message::Assistant { content, tool_calls } => {
                    let mut content_blocks = Vec::new();
                    if let Some(text) = content {
                        if !text.is_empty() {
                            content_blocks.push(serde_json::json!({
                                "type": "text",
                                "text": text
                            }));
                        }
                    }
                    for tc in tool_calls {
                        let parsed_args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                        content_blocks.push(serde_json::json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.function.name,
                            "input": parsed_args
                        }));
                    }
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content_blocks
                    }));
                }
                pr_core::Message::Tool { tool_call_id, content } => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content
                        }]
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": req.max_tokens.unwrap_or(8192),
            "messages": messages,
        });

        if !system_blocks.is_empty() {
            body["system"] = serde_json::Value::Array(system_blocks);
        }

        if let Some(budget) = self.thinking_budget {
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": budget
            });
        }

        if !req.tools.is_empty() {
            let anthropic_tools: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters
                    })
                })
                .collect();
            if !anthropic_tools.is_empty() {
                body["tools"] = serde_json::Value::Array(anthropic_tools);
            }
        }

        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| PrError::Llm(format!("Anthropic HTTP request error: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(PrError::Llm(format!("Anthropic API error (HTTP {}): {}", status, err_text)));
        }

        let json_val: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PrError::Llm(format!("Failed to parse Anthropic JSON response: {}", e)))?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(content_arr) = json_val.get("content").and_then(|v| v.as_array()) {
            for item in content_arr {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                if item_type == "text" {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(t.to_string());
                    }
                } else if item_type == "tool_use" {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                    let input = item.get("input").cloned().unwrap_or(serde_json::json!({}));
                    tool_calls.push(pr_core::ToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: pr_core::ToolCallFunction {
                            name,
                            arguments: serde_json::to_string(&input).unwrap_or_default(),
                        },
                    });
                }
            }
        }
        let usage = json_val.get("usage").map(|u| Usage {
            prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            total_tokens: (u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                + u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0)) as u32,
        });
        let finish_reason = json_val.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string());

        let final_text = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        };

        Ok(CompletionResponse {
            message: pr_core::Message::Assistant {
                content: final_text,
                tool_calls,
            },
            usage,
            finish_reason,
        })
    }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        let res = self.complete(req).await?;
        let text_delta = match &res.message {
            pr_core::Message::Assistant { content, .. } => content.clone().unwrap_or_default(),
            _ => String::new(),
        };
        let chunk = StreamChunk::Text { delta: text_delta };
        let done_chunk = StreamChunk::Done {
            message: res.message,
            usage: res.usage,
            finish_reason: res.finish_reason,
        };
        Ok(Box::new(futures::stream::iter(vec![Ok(chunk), Ok(done_chunk)])))
}
}
