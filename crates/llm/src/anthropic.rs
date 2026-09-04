use async_trait::async_trait;
use pr_core::{PrError, PrResult, ThinkingBlock, ToolCall, ToolCallFunction};

use crate::types::{CompletionRequest, CompletionResponse, StreamChunk, Usage};
use crate::provider::LlmProvider;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
    prompt_caching: bool,
    thinking_budget: Option<u32>,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: Option<String>,
        prompt_caching: bool,
        thinking_budget: Option<u32>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            base_url: base_url.unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            prompt_caching,
            thinking_budget,
            client: pr_core::http_client(),
        }
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

        // 1. System blocks with Breakpoint #1 on last system block
        let mut system_blocks = Vec::new();
        let sys_msgs: Vec<_> = req.messages.iter().filter_map(|m| match m {
            pr_core::Message::System { content } => Some(content),
            _ => None,
        }).collect();

        for (idx, content) in sys_msgs.iter().enumerate() {
            let mut block = serde_json::json!({
                "type": "text",
                "text": content
            });
            if self.prompt_caching && idx + 1 == sys_msgs.len() {
                block["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
            system_blocks.push(block);
        }

        // 2. Non-system messages with Thinking and Signature preservation
        let non_sys_msgs: Vec<_> = req.messages.iter().filter(|m| !matches!(m, pr_core::Message::System { .. })).collect();
        let total_non_sys = non_sys_msgs.len();
        let mut messages = Vec::new();

        for (turn_idx, msg) in non_sys_msgs.iter().enumerate() {
            let is_transcript_head = turn_idx == 0;
            let is_rolling_checkpoint = total_non_sys >= 4 && turn_idx == total_non_sys - 2;

            match msg {
                pr_core::Message::System { .. } => unreachable!(),
                pr_core::Message::User { content } => {
                    let mut user_val = serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "text",
                            "text": content
                        }]
                    });
                    if self.prompt_caching && (is_transcript_head || is_rolling_checkpoint) {
                        user_val["content"][0]["cache_control"] = serde_json::json!({ "type": "ephemeral" });
                    }
                    messages.push(user_val);
                }
                pr_core::Message::Assistant { content, thinking_blocks, tool_calls } => {
                    let mut content_blocks = Vec::new();

                    for tb in thinking_blocks {
                        match tb {
                            ThinkingBlock::Thinking { thinking, signature } => {
                                let mut block = serde_json::json!({
                                    "type": "thinking",
                                    "thinking": thinking
                                });
                                if let Some(sig) = signature {
                                    block["signature"] = serde_json::json!(sig);
                                }
                                content_blocks.push(block);
                            }
                            ThinkingBlock::RedactedThinking { data } => {
                                content_blocks.push(serde_json::json!({
                                    "type": "redacted_thinking",
                                    "data": data
                                }));
                            }
                        }
                    }

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
                    let mut tool_res = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": tool_call_id,
                        "content": content
                    });
                    if self.prompt_caching && is_rolling_checkpoint {
                        tool_res["cache_control"] = serde_json::json!({ "type": "ephemeral" });
                    }
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [tool_res]
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
            let mut anthropic_tools: Vec<serde_json::Value> = req
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
            if self.prompt_caching && !anthropic_tools.is_empty() {
                let last_idx = anthropic_tools.len() - 1;
                anthropic_tools[last_idx]["cache_control"] = serde_json::json!({ "type": "ephemeral" });
            }
            body["tools"] = serde_json::Value::Array(anthropic_tools);
        }

        let resp = self.client.post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "prompt-caching-2024-07-31")
            .json(&body)
            .send()
            .await
            .map_err(|e| PrError::Llm(format!("Anthropic HTTP error: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let err_text = resp.text().await.unwrap_or_default();
            return Err(PrError::Http {
                status: status.as_u16(),
                message: format!("Anthropic API error (HTTP {status}): {err_text}"),
                retry_after,
            });
        }

        let json_val: serde_json::Value = resp.json().await
            .map_err(|e| PrError::Llm(format!("Failed to parse Anthropic JSON: {e}")))?;

        let mut text_parts = Vec::new();
        let mut thinking_blocks = Vec::new();
        let mut tool_calls = Vec::new();

        if let Some(content_arr) = json_val.get("content").and_then(|v| v.as_array()) {
            for item in content_arr {
                let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                match item_type {
                    "thinking" => {
                        if let Some(th) = item.get("thinking").and_then(|v| v.as_str()) {
                            let sig = item.get("signature").and_then(|v| v.as_str()).map(|s| s.to_string());
                            thinking_blocks.push(ThinkingBlock::Thinking {
                                thinking: th.to_string(),
                                signature: sig,
                            });
                        }
                    }
                    "redacted_thinking" => {
                        if let Some(d) = item.get("data").and_then(|v| v.as_str()) {
                            thinking_blocks.push(ThinkingBlock::RedactedThinking {
                                data: d.to_string(),
                            });
                        }
                    }
                    "text" => {
                        if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(t.to_string());
                        }
                    }
                    "tool_use" => {
                        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                        let input = item.get("input").cloned().unwrap_or(serde_json::json!({}));
                        tool_calls.push(ToolCall {
                            id,
                            call_type: "function".to_string(),
                            function: ToolCallFunction {
                                name,
                                arguments: serde_json::to_string(&input).unwrap_or_default(),
                            },
                        });
                    }
                    _ => {}
                }
            }
        }

        let usage = json_val.get("usage").map(|u| {
            let input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let output_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cache_creation = u.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            let cache_read = u.get("cache_read_input_tokens").and_then(|v| v.as_u64()).map(|v| v as u32);
            Usage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens: input_tokens + output_tokens,
                cache_creation_input_tokens: cache_creation,
                cache_read_input_tokens: cache_read,
            }
        });

        let finish_reason = json_val.get("stop_reason").and_then(|v| v.as_str()).map(|s| s.to_string());
        let final_text = if text_parts.is_empty() { None } else { Some(text_parts.join("\n")) };

        Ok(CompletionResponse {
            message: pr_core::Message::assistant_full(final_text, thinking_blocks, tool_calls),
            usage,
            finish_reason,
        })
    }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn futures::Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        let resp = self.complete(req).await?;
        let text = match &resp.message {
            pr_core::Message::Assistant { content: Some(c), .. } => c.clone(),
            _ => String::new(),
        };

        let stream = futures::stream::iter(vec![
            Ok(StreamChunk::Text { delta: text }),
            Ok(StreamChunk::Done {
                message: resp.message,
                usage: resp.usage,
                finish_reason: resp.finish_reason,
            }),
        ]);

        Ok(Box::new(stream))
    }
}
