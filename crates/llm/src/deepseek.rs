use async_trait::async_trait;
use pr_core::{Message, ToolCall, PrResult, PrError};
use crate::provider::LlmProvider;
use crate::types::{CompletionRequest, CompletionResponse, StreamChunk, Usage};
use crate::retry::with_retry;
use serde::Deserialize;
use futures::StreamExt;
use std::time::Duration;

/// Maximum response body size in bytes (50 MB). Responses larger than this
/// trigger a fallback to streaming mode.
const MAX_RESPONSE_BYTES: usize = 50 * 1024 * 1024;

/// Number of retry attempts for HTTP requests.
const MAX_RETRIES: u32 = 3;

/// Threshold (10 MB) above which we proactively switch to streaming rather
/// than buffering the full response body.
const STREAMING_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;

/// Take a byte-length prefix without splitting a UTF-8 character.
fn safe_prefix(s: &str, max_bytes: usize) -> &str {
    let mut end = s.len().min(max_bytes);
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

pub struct DeepSeekProvider {
    base_url: String,
    api_key: String,
    model: String,
    http: reqwest::Client,
    /// Logical provider name reported by [`LlmProvider::name`]. Defaults to
    /// "deepseek"; overridden when the provider is built from configuration
    /// (the wire protocol is OpenAI-compatible for all supported providers).
    provider_name: String,
    /// Bounded per-model concurrency: a swarm fan-out queues instead of
    /// self-inflicting 429s.
    semaphore: crate::concurrency::ModelSemaphore,
    /// 429/5xx-aware cooldown so a rate-limited model isn't re-molten
    /// round after round.
    cooldown: crate::concurrency::FallbackCooldown,
}

impl DeepSeekProvider {
    pub fn new(base_url: &str, api_key: &str, model: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5-minute timeout
            .build()
            .expect("failed to build HTTP client");

        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
            http,
            provider_name: "deepseek".to_string(),
            semaphore: crate::concurrency::ModelSemaphore::default(),
            cooldown: crate::concurrency::FallbackCooldown::default(),
        }
    }

    /// Override the logical provider name (e.g. "openai", "openrouter",
    /// "ollama") when this OpenAI-compatible client is built from config.
    pub fn with_provider_name(mut self, name: impl Into<String>) -> Self {
        self.provider_name = name.into();
        self
    }

    fn build_request_body(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": req.messages,
            "stream": stream,
        });

        if let Some(temp) = req.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tok) = req.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tok);
        }

        if !req.tools.is_empty() {
            let tools: Vec<serde_json::Value> = req.tools.iter().map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            }).collect();
            body["tools"] = serde_json::json!(tools);
        }

        if stream {
            body["stream_options"] = serde_json::json!({"include_usage": true});
        }

        body
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
    #[serde(default)]
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: Option<ApiMessage>,
    delta: Option<ApiMessage>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    #[serde(default)]
    content: Option<String>,
    /// Reasoning models (e.g. DeepSeek R1/V4-thinking variants) return the
    /// chain-of-thought in a separate field. It is not part of the answer,
    /// but we parse it to detect budget-truncation (empty `content` while
    /// reasoning consumed the whole `max_tokens` budget).
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ApiToolCall>,
}

#[derive(Debug, Deserialize)]
struct ApiToolCall {
    /// Streaming: position of this call within the response (correlation
    /// key across deltas). Non-streaming responses omit it.
    #[serde(default)]
    index: Option<usize>,
    id: Option<String>,
    // The API also returns "type": "function" here; serde ignores unknown
    // fields, so it is not modeled.
    function: Option<ApiFunction>,
}

#[derive(Debug, Deserialize)]
struct ApiFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl DeepSeekProvider {
    /// Parse a non-streaming JSON response into a CompletionResponse.
    fn parse_response(text: &str) -> PrResult<CompletionResponse> {
        let api_resp: ApiResponse = serde_json::from_str(text).map_err(|e| {
            let preview = safe_prefix(text, 500);
            PrError::Llm(format!(
                "parse response failed: {e}, body preview: {preview}"
            ))
        })?;

        let choice = api_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| PrError::Llm("no choices in response".to_string()))?;

        let api_msg = choice
            .message
            .ok_or_else(|| PrError::Llm("no message in choice".to_string()))?;

        let tool_calls: Vec<ToolCall> = api_msg
            .tool_calls
            .iter()
            .filter_map(|tc| {
                let func = tc.function.as_ref()?;
                let name = func.name.as_ref()?.clone();
                let args_str = func.arguments.as_deref().unwrap_or("{}");
                let arguments: serde_json::Value =
                    serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));
                Some(ToolCall::new(
                    tc.id.clone().unwrap_or_default(),
                    name,
                    arguments,
                ))
            })
            .collect();

        // Reasoning-model truncation diagnostic: when the model spent the
        // whole completion budget on reasoning, `content` comes back empty
        // (finish_reason "length") while `reasoning_content` is non-empty.
        // Surface this loudly — silently returning "" produces empty reports.
        let content_empty = api_msg
            .content
            .as_deref()
            .map(|c| c.trim().is_empty())
            .unwrap_or(true);
        if content_empty && tool_calls.is_empty() {
            if let Some(reasoning) = &api_msg.reasoning_content {
                if !reasoning.trim().is_empty() {
                    tracing::warn!(
                        finish_reason = choice.finish_reason.as_deref().unwrap_or("?"),
                        reasoning_chars = reasoning.len(),
                        "LLM returned empty content: reasoning consumed the \
                         max_tokens budget — increase max_tokens for this call"
                    );
                }
            }
        }

        let message = Message::assistant_with_tools(api_msg.content, tool_calls);

        Ok(CompletionResponse {
            message,
            usage: api_resp.usage.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            finish_reason: choice.finish_reason,
        })
    }

    /// Fall back to streaming mode to handle very large responses.
    /// Collects all text chunks and assembles a single CompletionResponse.
    async fn complete_via_streaming(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<CompletionResponse> {
        let mut stream = self.stream(req).await?;
        let mut content = String::new();
        let mut usage: Option<Usage> = None;
        let mut finish_reason: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::Text { delta } => {
                    if content.len() + delta.len() > MAX_RESPONSE_BYTES {
                        return Err(PrError::Llm(format!(
                            "streaming response exceeded {} byte limit",
                            MAX_RESPONSE_BYTES
                        )));
                    }
                    content.push_str(&delta);
                }
                StreamChunk::Done {
                    usage: u,
                    finish_reason: fr,
                    ..
                } => {
                    usage = u;
                    finish_reason = fr;
                }
                // Ignore tool-call deltas and errors during collection;
                // the final content string is what matters for complete().
                StreamChunk::ToolCallDelta { .. } | StreamChunk::Error { .. } => {}
            }
        }

        Ok(CompletionResponse {
            message: Message::assistant(content),
            usage,
            finish_reason,
        })
    }
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {
    fn name(&self) -> &str {
        &self.provider_name
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> {
        // Rate-limit cooldown: if this model lane just got throttled, wait out
        // the window before trying again so a swarm does not re-hammer it.
        if self.cooldown.is_cooldown(&self.model).await {
            if let Some(wait) = self.cooldown.wait_hint(&self.model).await {
                tracing::warn!("model {} in rate-limit cooldown; waiting {wait:?}", self.model);
                tokio::time::sleep(wait).await;
            }
        }
        let body = self.build_request_body(req, false);
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let http = self.http.clone();
        // Serialize ONCE and reuse across retries — previously the whole
        // history was re-cloned into a Value per attempt and re-serialized
        // by `.json()` every time (fleet B10).
        let body_str = serde_json::to_string(&body)
            .map_err(|e| PrError::Llm(format!("serialize: {e}")))?;

        // First attempt: try non-streaming with retries
        let result = with_retry(
            || {
                let http = http.clone();
                let url = url.clone();
                let api_key = api_key.clone();
                let body = body_str.clone();
                async move {
                    let response = http
                        .post(&url)
                        .bearer_auth(&api_key)
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .body(body)
                        .send()
                        .await
                        .map_err(|e| {
                            let kind = if e.is_timeout() {
                                "timeout"
                            } else if e.is_decode() {
                                "decode"
                            } else {
                                "connect"
                            };
                            PrError::Llm(format!(
                                "request failed ({kind}): {e}"
                            ))
                        })?;

                    let status = response.status();

                    // Check Content-Length before buffering; if the body is
                    // large, signal that the caller should fall back to streaming.
                    if let Some(content_length) = response.content_length() {
                        if content_length > STREAMING_THRESHOLD_BYTES {
                            return Err(PrError::ResponseTooLarge(format!(
                                "{content_length} bytes exceeds {STREAMING_THRESHOLD_BYTES} \
                                 streaming threshold (HTTP {status})"
                            )));
                        }
                    }

                    // Read before consuming the body below.
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok());

                    let text = response.text().await.map_err(|e| {
                        let kind = if e.is_decode() {
                            "decode"
                        } else if e.is_timeout() {
                            "timeout"
                        } else {
                            "body read"
                        };
                        PrError::Llm(format!(
                            "read body failed ({kind}): {e}"
                        ))
                    })?;

                    // Post-buffer size guard
                    if text.len() > MAX_RESPONSE_BYTES {
                        return Err(PrError::ResponseTooLarge(format!(
                            "{} bytes exceeds {} byte limit (HTTP {})",
                            text.len(),
                            MAX_RESPONSE_BYTES,
                            status
                        )));
                    }

                    if !status.is_success() {
                        return Err(PrError::Http {
                            status: status.as_u16(),
                            message: safe_prefix(&text, 2000).to_string(),
                            retry_after,
                        });
                    }

                    Ok(text)
                }
            },
            MAX_RETRIES,
        )
        .await;

        // If the provider is throttled, record the cooldown so subsequent
        // agents in the swarm wait instead of re-hammering it.
        if let Err(PrError::Http { status, .. }) = &result {
            if *status == 429 || (*status >= 500 && *status < 600) {
                let rate_limited = *status == 429;
                self.cooldown.note_limit(&self.model, rate_limited).await;
            }
        }

        // If non-streaming succeeded, parse and return
        match result {
            Ok(text) => {
                return Self::parse_response(&text);
            }
            Err(PrError::ResponseTooLarge(_)) => {
                tracing::warn!(
                    "Response body too large for non-streaming, \
                     falling back to streaming mode"
                );
            }
            Err(e) => return Err(e),
        }

        // Fallback: streaming mode (collect all chunks)
        self.complete_via_streaming(req).await
    }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn futures::Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        let body = self.build_request_body(req, true);
        let url = format!("{}/chat/completions", self.base_url);

        let response = self.http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| PrError::Llm(format!("request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(PrError::Llm(format!("API error {status}: {text}")));
        }

        // Pin the byte stream so the unfold state stays Unpin.
        let byte_stream = Box::pin(response.bytes_stream());

        // Line-buffered SSE decoding: keeps a byte remainder across HTTP
        // chunks so frames split mid-chunk (and multibyte UTF-8 split at
        // chunk edges — '\n' cannot occur inside a UTF-8 sequence) are not
        // corrupted (fleet bug round 2).
        let stream: Box<dyn futures::Stream<Item = PrResult<StreamChunk>> + Send + Unpin> =
            Box::new(futures::stream::try_unfold(
            (byte_stream, Vec::<u8>::new()),
            |(mut byte_stream, mut remainder)| Box::pin(async move {
                use futures::StreamExt;
                loop {
                    if let Some(pos) = remainder.iter().position(|b| *b == b'\n') {
                        let line_bytes: Vec<u8> = remainder.drain(..=pos).collect();
                        let line = String::from_utf8_lossy(&line_bytes);
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(chunk) = parse_sse_line(line) {
                            return Ok(Some((chunk, (byte_stream, remainder))));
                        }
                        continue;
                    }
                    match byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            remainder.extend_from_slice(&bytes);
                        }
                        Some(Err(e)) => {
                            return Err(PrError::Llm(format!("stream error: {e}")));
                        }
                        None => {
                            // Stream ended: flush a trailing partial line.
                            if !remainder.is_empty() {
                                let tail: Vec<u8> = remainder.drain(..).collect();
                                let line = String::from_utf8_lossy(&tail).trim().to_string();
                                if let Some(chunk) = parse_sse_line(&line) {
                                    return Ok(Some((chunk, (byte_stream, remainder))));
                                }
                            }
                            return Ok(None);
                        }
                    }
                }
            }),
        ));

        Ok(stream)
    }
}

/// Parse one SSE `data:` line into a stream chunk, if it carries one.
fn parse_sse_line(line: &str) -> Option<StreamChunk> {
    let data = line.strip_prefix("data: ")?;
    if data == "[DONE]" {
        return None;
    }
    let api_resp: ApiResponse = serde_json::from_str(data).ok()?;
    let choice = api_resp.choices.into_iter().next()?;
    if let Some(delta) = choice.delta {
        if let Some(content) = &delta.content {
            if !content.is_empty() {
                return Some(StreamChunk::Text {
                    delta: content.clone(),
                });
            }
        }
        // Tool-call deltas: `id`/`name` arrive in the FIRST delta of each
        // index; later fragments carry only argument pieces. Emit every
        // non-empty fragment keyed by index so the caller can reassemble.
        for tc in &delta.tool_calls {
            let Some(func) = &tc.function else { continue };
            let name = func.name.clone().unwrap_or_default();
            let args = func.arguments.clone().unwrap_or_default();
            if name.is_empty() && args.is_empty() {
                continue;
            }
            return Some(StreamChunk::ToolCallDelta {
                index: tc.index.unwrap_or(0),
                id: tc.id.clone().unwrap_or_default(),
                name,
                arguments_delta: args,
            });
        }
    }
    if choice.finish_reason.is_some() || api_resp.usage.is_some() {
        let usage = api_resp.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });
        return Some(StreamChunk::Done {
            message: Message::assistant(""),
            usage,
            finish_reason: choice.finish_reason.clone(),
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_sensible() {
        assert!(MAX_RESPONSE_BYTES >= 10 * 1024 * 1024);
        assert!(MAX_RETRIES >= 1);
        assert!(STREAMING_THRESHOLD_BYTES <= MAX_RESPONSE_BYTES as u64);
    }

    #[test]
    fn parse_valid_response() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": "Hello world",
                    "tool_calls": []
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let resp = DeepSeekProvider::parse_response(json).unwrap();
        match &resp.message {
            Message::Assistant { content, tool_calls } => {
                assert_eq!(content.as_deref(), Some("Hello world"));
                assert!(tool_calls.is_empty());
            }
            other => panic!("expected Assistant variant, got {:?}", other),
        }
        assert_eq!(resp.usage.as_ref().unwrap().total_tokens, 15);
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn parse_response_no_choices_fails() {
        let json = r#"{"choices": []}"#;
        let result = DeepSeekProvider::parse_response(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_response_with_tool_calls() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"query\":\"test\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        }"#;

        let resp = DeepSeekProvider::parse_response(json).unwrap();
        match &resp.message {
            Message::Assistant { content, tool_calls } => {
                assert!(content.is_none());
                assert_eq!(tool_calls.len(), 1);
                assert_eq!(tool_calls[0].name(), "search");
            }
            other => panic!("expected Assistant variant, got {:?}", other),
        }
    }

    #[test]
    fn parse_reasoning_model_response() {
        let json = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "The answer is 4.",
                    "reasoning_content": "Let me think step by step..."
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 15,
                "total_tokens": 25
            }
        }"#;

        let resp = DeepSeekProvider::parse_response(json).unwrap();
        match &resp.message {
            Message::Assistant { content, tool_calls } => {
                assert_eq!(content.as_deref(), Some("The answer is 4."));
                assert!(tool_calls.is_empty());
            }
            other => panic!("expected Assistant variant, got {:?}", other),
        }
    }

    #[test]
    fn parse_reasoning_truncated_response() {
        // Reasoning consumed the whole max_tokens budget: content is empty,
        // reasoning_content carries the truncated chain-of-thought.
        let json = r#"{
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "",
                    "reasoning_content": "We need to think about..."
                },
                "finish_reason": "length"
            }],
            "usage": {
                "prompt_tokens": 111,
                "completion_tokens": 100,
                "total_tokens": 211
            }
        }"#;

        let resp = DeepSeekProvider::parse_response(json).unwrap();
        match &resp.message {
            Message::Assistant { content, .. } => {
                assert!(content.as_deref().map(|c| c.is_empty()).unwrap_or(true));
            }
            other => panic!("expected Assistant variant, got {:?}", other),
        }
        assert_eq!(resp.finish_reason.as_deref(), Some("length"));
    }

    #[tokio::test]
    async fn new_builds_client_with_timeout() {
        // Just verify constructor does not panic
        let provider = DeepSeekProvider::new(
            "https://api.deepseek.com/v1",
            "test-key",
            "deepseek-chat",
        );
        assert_eq!(provider.name(), "deepseek");
        assert_eq!(provider.model(), "deepseek-chat");
    }

    // ── SSE line parsing (true streaming) ─────────────────────────────

    #[test]
    fn parse_sse_text_delta() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        match parse_sse_line(line).unwrap() {
            StreamChunk::Text { delta } => assert_eq!(delta, "Hello"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_first_delta_has_id_and_name() {
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"web_search","arguments":""}}]}}]}"#;
        match parse_sse_line(line).unwrap() {
            StreamChunk::ToolCallDelta { index, id, name, arguments_delta } => {
                assert_eq!(index, 0);
                assert_eq!(id, "call_1");
                assert_eq!(name, "web_search");
                assert_eq!(arguments_delta, "");
            }
            other => panic!("expected ToolCallDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_tool_call_argument_fragment() {
        // Subsequent fragments carry only index + argument pieces.
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"q\":"}}]}}]}"#;
        match parse_sse_line(line).unwrap() {
            StreamChunk::ToolCallDelta { index, id, name, arguments_delta } => {
                assert_eq!(index, 0);
                assert!(id.is_empty());
                assert!(name.is_empty());
                assert_eq!(arguments_delta, "{\"q\":");
            }
            other => panic!("expected ToolCallDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_done_with_usage() {
        let line = r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7}}"#;
        match parse_sse_line(line).unwrap() {
            StreamChunk::Done { usage, finish_reason, .. } => {
                assert_eq!(usage.unwrap().total_tokens, 7);
                assert_eq!(finish_reason.as_deref(), Some("stop"));
            }
            other => panic!("expected Done, got {:?}", other),
        }
    }

    #[test]
    fn parse_sse_sentinel_and_garbage() {
        assert!(parse_sse_line("data: [DONE]").is_none());
        assert!(parse_sse_line("data: not-json").is_none());
        assert!(parse_sse_line(": keep-alive comment").is_none());
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(128))]

            #[test]
            fn parse_sse_line_never_panics(line in "\\PC{0,300}") {
                // Arbitrary bytes: must yield Some/None, never panic.
                let _ = parse_sse_line(&line);
            }

            #[test]
            fn parse_sse_data_prefixed_json_never_panics(payload in "\\PC{0,200}") {
                let line = format!("data: {payload}");
                let _ = parse_sse_line(&line);
            }

            #[test]
            fn valid_text_delta_roundtrips(text in "[a-zA-Z0-9 ]{1,80}") {
                let line = format!(
                    r#"data: {{"choices":[{{"delta":{{"content":{}}}}}]}}"#,
                    serde_json::to_string(&text).unwrap()
                );
                match parse_sse_line(&line) {
                    Some(StreamChunk::Text { delta }) => prop_assert_eq!(delta, text),
                    other => prop_assert!(false, "expected Text chunk, got {:?}", other),
                }
            }
        }
    }
}
