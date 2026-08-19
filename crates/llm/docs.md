# Crate Documentation `crates/llm`

The `llm` crate handles interaction with language models via the OpenAI-compatible chat-completions protocol. It includes typed request/response structures, a trait provider, a single `DeepSeekProvider` implementation (compatible with any OpenAI-compatible endpoint), a retry mechanism with exponential backoff, a per-model concurrency throttle, and a provider factory.

---

## Architecture Overview

The crate is designed around a **single-trait abstraction** (`LlmProvider`) with a single concrete implementation (`DeepSeekProvider`) that speaks the OpenAI chat-completions wire protocol. Despite the name, it works with **any** OpenAI-compatible endpoint — DeepSeek, OpenAI, OpenRouter, Ollama, vLLM, LM Studio, and others. The factory maps a configuration key to a provider instance, enabling multi-model setups where a primary (expensive) model handles reasoning and a "fast" model handles high-volume auxiliary calls (entity extraction, memory classification, search reranking).

Two orthogonal throttles protect against self-inflicted rate limits in a multi-agent swarm:

- **`ModelSemaphore`** — a bounded semaphore per model lane (default 3 concurrent requests) so a fan-out of sub-agents queues *saturate* instead of slamming the same model simultaneously.
- **`FallbackCooldown`** — after a 429 (rate limit) or 5xx (server error) the provider is known to be throttled; a cooldown window is recorded so the swarm does not re-hammer the endpoint round after round from separate agents.

---

## Table of Contents

1. [lib.rs — entry point](#librs)
2. [types.rs — data types](#typesrs)
3. [provider.rs — LlmProvider trait](#providerrs)
4. [deepseek.rs — DeepSeekProvider implementation](#deepseekrs)
5. [retry.rs — retry mechanism](#retryrs)
6. [concurrency.rs — per-model throttles](#concurrencyrs)
7. [factory.rs — provider factory](#factoryrs)

---

## lib.rs

The file [lib.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/lib.rs) declares **six** public modules and re-exports all their contents via `pub use`:

```rust
pub mod provider;
pub mod deepseek;
pub mod types;
pub mod retry;
pub mod factory;
pub mod concurrency;

pub use provider::*;
pub use deepseek::*;
pub use types::*;
pub use retry::*;
pub use factory::*;
pub use concurrency::*;
```

This means crate consumers can write `use llm::LlmProvider`, `use llm::CompletionRequest`, `use llm::ModelSemaphore`, etc. directly without specifying internal modules. The `concurrency` module is the newest addition, providing the swarm-protection throttles that the `DeepSeekProvider` uses internally.

---

## types.rs

The file [types.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/types.rs) defines four key structures and one enum that serve as the lingua franca of the entire LLM layer. These types are serialized/deserialized with `serde` and are designed to match the OpenAI chat-completions JSON schema.

### `CompletionRequest`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSchema>,       // default = empty, skip_serializing_if empty
    pub temperature: Option<f32>,      // skip_serializing_if = None
    pub max_tokens: Option<u32>,       // skip_serializing_if = None
    pub stream: bool,                  // default = false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `messages` | `Vec<Message>` | The full conversation history. Each `Message` is an enum from `pr_core` containing the roles `System`, `User`, `Assistant` (with optional `content` and `Vec<ToolCall>`) and `Tool` (tool execution result). |
| `tools` | `Vec<ToolSchema>` | List of available tools. Each `ToolSchema` contains `name: String`, `description: String`, `parameters: serde_json::Value` (JSON Schema). During serialization the field is skipped if the vector is empty — this saves bandwidth when tools are not needed. |
| `temperature` | `Option<f32>` | Sampling temperature. Skipped when `None`. |
| `max_tokens` | `Option<u32>` | Maximum number of tokens in the response. Skipped when `None`. |
| `stream` | `bool` | Streaming flag. Defaults to `false`. The server side uses it to select the response format (SSE vs JSON). |

**Design note**: `temperature` and `max_tokens` are `Option` rather than having defaults so that `None` is omitted from the JSON body entirely, letting the downstream API use its own defaults. This is important because different providers (and even different models from the same provider) have different sensible defaults.

### `CompletionResponse`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message: Message,
    pub usage: Option<Usage>,
    pub finish_reason: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Message` | The model response. Typically `Message::Assistant { content, tool_calls }`. If the model calls tools, `content` may be `None` and `tool_calls` may contain one or more calls. |
| `usage` | `Option<Usage>` | Token statistics (prompt/completion/total). May be `None` if the provider did not return this information. |
| `finish_reason` | `Option<String>` | The reason generation completed. Standard values: `"stop"` (model decided to stop), `"length"` (token limit reached), `"tool_calls"` (model requested a tool call). |

### `Usage`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

Three fields — an exact copy of the `usage` structure from the OpenAI API. `total_tokens` is always `prompt_tokens + completion_tokens`. This structure is used for cost tracking and token budget management by upstream callers. The `DeepSeekProvider` faithfully maps the API's `ApiUsage` into this type, and live integration tests verify the invariant `prompt_tokens + completion_tokens == total_tokens` across multiple calls.

### `StreamChunk`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamChunk {
    Text { delta: String },
    ToolCallDelta { index: usize, id: String, name: String, arguments_delta: String },
    Done { message: Message, usage: Option<Usage>, finish_reason: Option<String> },
    Error { message: String },
}
```

An enum with an internal `"type"` tag (serde attribute `#[serde(tag = "type")]`). Each variant represents one "step" of the stream:

- **`Text`** — the next fragment of the model's text response. `delta` contains incremental text.
- **`ToolCallDelta`** — an incremental fragment of a tool call. `index` is the position of the tool call within the response (the OpenAI streaming protocol sends `id`/`name` only in the **first** delta of each index; subsequent argument fragments carry the same index, so it is the correlation key for reassembly). `id` is the call identifier, `name` is the function name, `arguments_delta` is a partial string of JSON arguments (the model generates JSON arguments character by character).
- **`Done`** — stream completion signal. Contains the final `Message` (usually empty since content was already delivered via `Text`/`ToolCallDelta`), `usage` statistics, and `finish_reason`.
- **`Error`** — an error that occurred during streaming.

---

## provider.rs

The file [provider.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/provider.rs) defines the `LlmProvider` trait — the central abstraction for any LLM provider.

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse>;
    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>>;
}
```

| Method | Description |
|--------|-------------|
| `name()` | The logical name of the provider (e.g. `"deepseek"`, `"openrouter"`). Used for logging and identification. |
| `model()` | The model name (e.g. `"deepseek-chat"`). |
| `complete()` | A full (non-streaming) request to the LLM. Returns the entire `CompletionResponse`. Internally uses retry with exponential backoff and may fall back to streaming for large responses. |
| `stream()` | A streaming request. Returns `Box<dyn Stream<Item = PrResult<StreamChunk>>>` — an asynchronous stream of chunks that the consumer reads in a loop. |

The trait requires `Send + Sync`, allowing safe use of the provider from multiple tokio tasks simultaneously. This is essential for the multi-agent architecture where many sub-agents may call the same provider concurrently.

**Design rationale**: The trait deliberately separates `complete` and `stream` into two methods rather than a single method with a `stream` flag. This allows the implementation to use different code paths for each mode — the `complete` path pre-serializes the request body once and reuses it across retry attempts, while the `stream` path uses `reqwest::Response::bytes_stream()` for SSE decoding. The `complete` path also has a fallback: if the response body exceeds the streaming threshold (10 MB), it transparently re-issues the request in streaming mode and collects chunks into a single `CompletionResponse`. This hybrid approach keeps the caller's interface simple while handling edge cases.

---

## deepseek.rs

The file [deepseek.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/deepseek.rs) is the main and only implementation of `LlmProvider`. Despite the name, it works with any API compatible with OpenAI chat-completions. It implements the full lifecycle: request construction, HTTP transport with retry, SSE stream decoding, response parsing, reasoning-model diagnostics, and rate-limit cooldown tracking.

### Constants

```rust
const MAX_RESPONSE_BYTES: usize = 50 * 1024 * 1024;       // 50 MB
const MAX_RETRIES: u32 = 3;
const STREAMING_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;  // 10 MB
```

- `MAX_RESPONSE_BYTES` — absolute limit on response body size. If the response exceeds 50 MB, an error is generated. This prevents memory exhaustion from unexpectedly large responses.
- `MAX_RETRIES` — number of retry attempts for HTTP requests (passed to `with_retry`). With 3 retries, a single request can make up to 4 HTTP calls before giving up.
- `STREAMING_THRESHOLD_BYTES` — threshold above which the client proactively switches to streaming without waiting for the full body to download. This is checked via the `Content-Length` header before the body is buffered, avoiding unnecessary memory allocation for large responses.

### Helper function `safe_prefix`

```rust
fn safe_prefix(s: &str, max_bytes: usize) -> &str
```

Algorithm:
1. Computes `end = min(len(s), max_bytes)`.
2. If `end` falls inside a multi-byte UTF-8 character (is not a char boundary), decrements `end` by 1 until it finds a character boundary.
3. Returns `&s[..end]`.

Used to safely truncate response bodies when constructing error messages, to avoid breaking UTF-8 validity. Error messages use this to show a preview of the response body (first 500 bytes for parse errors, first 2000 bytes for HTTP errors) without risking invalid UTF-8 slices.

### Structure `DeepSeekProvider`

```rust
pub struct DeepSeekProvider {
    base_url: String,       // base API URL (without trailing /)
    api_key: String,        // API key
    model: String,          // model name
    http: reqwest::Client,  // HTTP client with 5-minute timeout
    provider_name: String,  // logical name (defaults to "deepseek")
    semaphore: crate::concurrency::ModelSemaphore,  // per-model concurrency throttle
    cooldown: crate::concurrency::FallbackCooldown, // 429/5xx-aware cooldown
}
```

The `DeepSeekProvider` bundles two concurrency-control mechanisms:

- **`semaphore`** (`ModelSemaphore`): a bounded semaphore per model lane (default 3 permits). When a swarm of sub-agents fans out, concurrent requests queue up against this semaphore rather than all hitting the API at once and getting 429'd. The semaphore is keyed by model name, so different models have independent lanes.
- **`cooldown`** (`FallbackCooldown`): after a 429 (rate limit) or 5xx (server error), the model lane is marked as cooling down for 30–60 seconds. Subsequent requests to the same model first check the cooldown and wait before attempting, preventing the swarm from re-hammering a rate-limited endpoint.

#### Constructor `new(base_url, api_key, model)`

Algorithm:
1. Creates `reqwest::Client::builder()` with a 300-second (5 minute) timeout.
2. Calls `.build().expect(...)` — panics if the HTTP client could not be created.
3. Strips trailing `/` from `base_url`.
4. Sets `provider_name = "deepseek"`.
5. Initializes `semaphore` and `cooldown` with their default values (3 permits, 30s default cooldown, 60s rate-limit cooldown).

#### Method `with_provider_name(self, name)`

Builder method (consumes self, returns self). Overrides `provider_name`. Used by the factory when the configuration specifies a different provider (e.g. `"openrouter"`).

#### Method `build_request_body(&self, req, stream) -> serde_json::Value`

Algorithm for constructing the request JSON body:
1. Creates a base JSON object with fields `model`, `messages`, `stream`.
2. If `req.temperature != None`, adds the `temperature` field.
3. If `req.max_tokens != None`, adds the `max_tokens` field.
4. If `req.tools` is not empty, maps each `ToolSchema` to the OpenAI format:
   ```json
   {
     "type": "function",
     "function": {
       "name": "<tool.name>",
       "description": "<tool.description>",
       "parameters": "<tool.parameters>"
     }
   }
   ```
   and adds the array to the `tools` field.
5. If `stream == true`, adds the field `"stream_options": {"include_usage": true}` — this is an OpenAI/DeepSeek-specific option that makes the server include `usage` statistics in the last SSE chunk.

### Internal structures for API response deserialization

```rust
struct ApiResponse { choices: Vec<ApiChoice>, usage: Option<ApiUsage> }
struct ApiChoice { message: Option<ApiMessage>, delta: Option<ApiMessage>, finish_reason: Option<String> }
struct ApiMessage { content: Option<String>, reasoning_content: Option<String>, tool_calls: Vec<ApiToolCall> }
struct ApiToolCall { index: Option<usize>, id: Option<String>, function: Option<ApiFunction> }
struct ApiFunction { name: Option<String>, arguments: Option<String> }
struct ApiUsage { prompt_tokens: u32, completion_tokens: u32, total_tokens: u32 }
```

- `ApiChoice.message` is used in non-streaming responses.
- `ApiChoice.delta` is used in streaming responses (SSE chunks).
- Both fields (`message` and `delta`) are deserialized into the same `ApiMessage` structure, since their schemas are identical.
- `ApiToolCall.function.arguments` is a **string** of JSON, not an object. This is an OpenAI API peculiarity: arguments are passed as a serialized JSON string.
- `ApiMessage.reasoning_content` captures the chain-of-thought output from reasoning models like DeepSeek R1 or V4-thinking variants. It is not part of the answer, but parsing it allows the provider to detect **budget truncation** — when the model spent the entire `max_tokens` budget on reasoning and returned empty `content`. This is surfaced as a warning log so callers can increase `max_tokens` for that call.
- `ApiToolCall.index` captures the streaming position of the tool call, used as a correlation key when reassembling tool-call deltas across multiple SSE chunks.

### Method `parse_response(text: &str) -> PrResult<CompletionResponse>`

Algorithm for parsing a non-streaming JSON response:

1. **Deserialization**: `serde_json::from_str::<ApiResponse>(text)`. On error, creates `PrError::Llm` with a preview of the body (first 500 bytes via `safe_prefix`).
2. **Extracting the first choice**: takes `api_resp.choices.into_iter().next()`. If the array is empty — error `"no choices in response"`.
3. **Extracting message**: `choice.message.ok_or_else(...)`. In non-streaming mode, `message` is always present.
4. **Extracting tool_calls**: iterates over `api_msg.tool_calls`, for each element:
   - Takes `function` (if `None` — element is skipped via `filter_map`).
   - Takes `function.name` (if `None` — element is skipped).
   - Parses `function.arguments` as JSON. If the string is `None` or parsing fails, uses `{}` as a fallback.
   - Creates `ToolCall::new(id, name, arguments)`.
   - `id` is taken from `tc.id.unwrap_or_default()` (empty string if absent).
5. **Reasoning-model truncation diagnostic**: if `content` is empty (or only whitespace) and there are no tool calls, checks whether `reasoning_content` is non-empty. If so, the model consumed its entire `max_tokens` budget on reasoning — logs a warning with the `finish_reason` and reasoning character count.
6. **Building Message**: `Message::assistant_with_tools(content, tool_calls)` — creates an `Assistant` variant with text and a list of calls.
7. **Mapping Usage**: `api_resp.usage` is mapped from `ApiUsage` to `Usage` (fields are identical).
8. **Returning**: `CompletionResponse { message, usage, finish_reason }`.

### Method `complete_via_streaming(&self, req) -> PrResult<CompletionResponse>`

Fallback strategy: when a non-streaming request received a response that is too large (`ResponseTooLarge`), this method assembles the full response from streaming chunks. This is a **transparent fallback** — the caller does not need to know that the request was re-issued in streaming mode.

Algorithm:
1. Calls `self.stream(req)` to obtain a streaming stream.
2. Initializes: `content = String::new()`, `usage = None`, `finish_reason = None`.
3. In a loop `while let Some(chunk) = stream.next().await`:
   - `StreamChunk::Text { delta }`: checks whether `content.len() + delta.len()` would exceed the `MAX_RESPONSE_BYTES` limit. If so — returns an error. Otherwise appends `delta` to `content`.
   - `StreamChunk::Done { usage, finish_reason, .. }`: saves `usage` and `finish_reason`.
   - `StreamChunk::ToolCallDelta` and `StreamChunk::Error`: ignored.
4. Returns `CompletionResponse` with `Message::assistant(content)` (without tool_calls), the collected `usage`, and `finish_reason`.

**Important**: when falling back to streaming, tool_calls are lost — this is intentional, as the fallback is only needed for text responses that are too large to buffer. Tool-calling responses are typically small enough to never trigger this path.

### `LlmProvider` implementation for `DeepSeekProvider`

#### `name()` and `model()`

Return `self.provider_name` and `self.model` respectively.

#### `complete(&self, req) -> PrResult<CompletionResponse>`

This is the main method. Algorithm step by step:

**Step 1 — Cooldown check:**
- Checks if the model lane is in cooldown via `self.cooldown.is_cooldown(&self.model)`. If so, waits for the remaining cooldown duration before proceeding. This prevents a swarm from re-hammering a recently rate-limited endpoint.

**Step 2 — Request body preparation:**
- Calls `build_request_body(req, false)` to get the JSON body (stream = false).
- Constructs the URL: `"{base_url}/chat/completions"`.
- Clones `api_key` and `http` client for the closure.
- Serializes the body to a string **once** (`serde_json::to_string(&body)`), to avoid re-serializing on every retry attempt. This is a performance optimization: previously the entire conversation history was re-cloned into a `Value` per attempt and re-serialized by `.json()` every time.

**Step 3 — HTTP request with retry (non-streaming):**

Calls `with_retry(closure, MAX_RETRIES)`. The closure on each attempt:

1. Sends `POST {url}` with headers:
   - `Authorization: Bearer {api_key}`
   - `Content-Type: application/json`
   - Body: the previously serialized JSON string.
2. Handles send errors: classifies as `"timeout"`, `"decode"`, or `"connect"` and wraps in `PrError::Llm`.
3. **Content-Length check**: if the server returned a `Content-Length` header larger than `STREAMING_THRESHOLD_BYTES` (10 MB), immediately returns `PrError::ResponseTooLarge(...)` — this signals a fallback to streaming.
4. **Extracting Retry-After**: reads the `Retry-After` header from the response, parses as `u64` (seconds). If the header is absent or unparseable — `retry_after = None`.
5. **Reading the body**: `response.text().await`. On read error, classifies as `"decode"`, `"timeout"`, or `"body read"`.
6. **Post-buffer size check**: if `text.len() > MAX_RESPONSE_BYTES` (50 MB), returns `PrError::ResponseTooLarge`.
7. **HTTP status check**: if status is not 2xx, returns `PrError::Http { status, message (first 2000 bytes of body), retry_after }`.
8. On success, returns `Ok(text)`.

**Step 4 — Handling the result:**
- `Ok(text)` → calls `Self::parse_response(&text)` and returns the result.
- `Err(PrError::ResponseTooLarge(_))` → logs a warning and proceeds to step 5.
- Any other error → returns it immediately.

**Step 5 — Cooldown recording:**
- If the error was HTTP 429 or 5xx, records the cooldown via `self.cooldown.note_limit(&self.model, rate_limited)`. This ensures subsequent calls from the same or different agents wait before retrying.

**Step 6 — Fallback to streaming:**
- Calls `self.complete_via_streaming(req).await` and returns the result.

#### `stream(&self, req) -> PrResult<Box<dyn Stream<...>>>`

Streaming request. Algorithm:

**Step 1 — Sending the request:**
- Calls `build_request_body(req, true)` (stream = true, includes `stream_options`).
- Constructs the URL: `"{base_url}/chat/completions"`.
- Sends `POST {url}` with headers:
  - `Authorization: Bearer {api_key}`
  - Body is serialized via `.json(&body)` (unlike `complete()`, where the body is pre-serialized to a string).
- On send error, returns `PrError::Llm`.
- Checks HTTP status: if not 2xx, reads the body and returns `PrError::Llm("API error {status}: {text}")`.

**Step 2 — Preparing the byte stream:**
- `response.bytes_stream()` returns `impl Stream<Item = Result<Bytes, reqwest::Error>>`.
- Wraps in `Box::pin(...)` to pin it in memory.

**Step 3 — SSE decoding with line buffering:**

Uses `futures::stream::try_unfold` with state `(byte_stream, remainder: Vec<u8>)`.

Algorithm of the unfold closure (called for each element of the output stream):

1. **Search for a line in the buffer**: finds the position of the first `b'\n'` in `remainder`.
   - If found: extracts all bytes up to and including `\n` into `line_bytes`, keeps the rest in `remainder`. Decodes `line_bytes` via `String::from_utf8_lossy`, trims whitespace.
     - If the line is empty — skips (empty lines are SSE event separators).
     - Calls `parse_sse_line(line)`.
       - If it returns `Some(chunk)` — returns `Ok(Some((chunk, (byte_stream, remainder))))`.
       - If `None` — skips (e.g., a `data: [DONE]` line) and proceeds to the next iteration.
   - If `\n` is not found — proceeds to step 2.
2. **Reading from the byte stream**: `byte_stream.next().await`.
   - `Some(Ok(bytes))` — appends `bytes` to `remainder` and returns to step 1.
   - `Some(Err(e))` — returns `PrError::Llm("stream error: {e}")`.
   - `None` (stream completed):
     - If `remainder` is not empty — processes the remaining bytes as the last line (without a trailing `\n`): `from_utf8_lossy`, `trim`, `parse_sse_line`.
     - Returns `Ok(None)` — end of stream.

**Key feature**: `remainder` persists between unfold calls, ensuring correct handling of HTTP chunks that may split SSE lines at any point (even inside a multi-byte UTF-8 character, since `\n` (0x0A) cannot appear inside a UTF-8 sequence).

### Function `parse_sse_line(line: &str) -> Option<StreamChunk>`

Algorithm for parsing a single SSE line:

1. Extracts the `data: ` prefix via `strip_prefix("data: ")`. If the prefix is absent — returns `None` (the line is a comment, `event:`, `id:`, etc.).
2. If `data == "[DONE]"` — returns `None` (SSE termination signal, does not produce a chunk).
3. Deserializes `data` as `ApiResponse`. On error — returns `None`.
4. Takes the first `choice` from `api_resp.choices`. If the array is empty — `None`.
5. **Delta check**: if `choice.delta` exists:
   - If `delta.content` exists and is non-empty — returns `StreamChunk::Text { delta: content.clone() }`.
   - Tool-call deltas: iterates `delta.tool_calls`. For each tool call with a non-empty `name` or `arguments`, emits `StreamChunk::ToolCallDelta { index, id, name, arguments_delta }`. The `index` field is the correlation key for reassembly — the streaming protocol sends `id`/`name` only in the first delta of each index, while subsequent fragments carry only argument pieces.
6. **Finish reason/usage check**: if `choice.finish_reason.is_some()` OR `api_resp.usage.is_some()`:
   - Maps `api_resp.usage` to `Usage`.
   - Returns `StreamChunk::Done { message: Message::assistant(""), usage, finish_reason }`.
7. If none of the conditions matched — returns `None`.

---

## retry.rs

The file [retry.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/retry.rs) implements a generic retry mechanism with exponential backoff and jitter. It is designed to be reusable across any async operation, not just LLM calls.

### Global jitter counter

```rust
static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);
```

An atomic counter used as a deterministic source of "pseudo-randomness" for jitter. Does not require the `rand` dependency. Each call to `jitter()` atomically increments the counter, ensuring different values for concurrent tasks. This is important because if multiple agents all fail at the same time, they would otherwise back off on the exact same schedule and retry simultaneously — jitter spreads them out.

### Function `jitter(delay: Duration) -> Duration`

Algorithm for computing jitter:

1. Reads and increments `JITTER_SEQ` (fetch_add with Ordering::Relaxed).
2. Computes `ms = delay.as_millis() as u64`.
3. Computes `spread = ms / 4` (±25% of the base delay).
4. If `spread == 0`, `delta = 0`.
5. Otherwise: `delta = (seq % (2 * spread + 1)) as i64 - spread as i64`.
   - `seq % (2 * spread + 1)` gives a value in the range `[0, 2*spread]`.
   - Subtracting `spread` shifts the range to `[-spread, +spread]`.
   - Thus, the final delay varies in `[75% * delay, 125% * delay]`.
6. Final delay: `max(ms + delta, 50)` — minimum 50 ms to avoid pauses that are too short.

**Example**: with `delay = 1000ms`, `spread = 250`, the final delay will be in the range `[750ms, 1250ms]`.

### Function `with_retry<F, Fut, T>(f: F, max_retries: u32) -> PrResult<T>`

The main retry logic function. Parameters:
- `f: F` — a closure returning `Future<Output = PrResult<T>>`. Called on each attempt.
- `max_retries: u32` — maximum number of **retry** attempts (not counting the first).

Algorithm step by step:

1. Initializes `delay = 500ms` (initial delay before the first retry attempt).
2. Loop `for attempt in 0..=max_retries` (includes attempt 0 — the first attempt):
   - Calls `f().await`.
   - **Success**: returns `Ok(val)`.
   - **Retryable error** (`attempt < max_retries && e.is_retryable()`):
     1. Checks `e.retry_after_secs()` — if the error contains Retry-After (e.g., HTTP 429), uses it, but capped at 60 seconds.
     2. If Retry-After is absent, uses the current `delay`.
     3. Applies `jitter(wait)` to the chosen delay.
     4. Logs a warning with the attempt number and delay.
     5. `tokio::time::sleep(wait).await` — waiting.
     6. Increases `delay = (delay * 2).min(60s)` — exponential growth, capped at 60 seconds.
   - **Non-retryable error** (or attempts exhausted): returns `Err(e)` immediately.
3. `unreachable!()` — if the loop completes without returning (should not happen).

**Delay schedule** (without jitter and Retry-After):
- After attempt 0: 500ms → next attempt 1
- After attempt 1: 1000ms → next attempt 2
- After attempt 2: 2000ms → next attempt 3
- Cap: 60 seconds

**Retryable error classification** is determined by the `e.is_retryable()` method from `PrError` (in the `pr_core` crate). From the tests, retryable errors are:
- HTTP 408 (Request Timeout)
- HTTP 429 (Too Many Requests)
- HTTP 5xx (server errors)
- Network errors (timeout, connect)

And **non-retryable**:
- HTTP 400 (Bad Request)
- HTTP 401 (Unauthorized)
- HTTP 403 (Forbidden)
- HTTP 404 (Not Found)
- `PrError::ResponseTooLarge`
- `PrError::Llm` (general parsing/format errors)

**Design rationale**: Permanent errors like 400/401/403/404 are not retried because they indicate a misconfiguration or invalid request that will never succeed on retry. Retrying them would waste time, quota, and money. `ResponseTooLarge` is also not retried in the normal path — instead it triggers a streaming fallback in `DeepSeekProvider::complete()`.

---

## concurrency.rs

The file [concurrency.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/concurrency.rs) provides two orthogonal throttles that protect a multi-agent swarm from self-inflicting provider rate limits. Inspired by the ouroboros `model_concurrency.py` and `fallback_cooldown.py` patterns.

### Constants

```rust
pub const DEFAULT_LANE_CONCURRENCY: usize = 3;
pub const DEFAULT_COOLDOWN: Duration = Duration::from_secs(30);
pub const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(60);
```

- `DEFAULT_LANE_CONCURRENCY` — default concurrent requests allowed per model lane (3). This means that if 10 sub-agents all try to call the same model simultaneously, only 3 will be in-flight at any time; the rest queue up.
- `DEFAULT_COOLDOWN` — default cooldown after a 5xx server error (30 seconds). During this window, the model lane is marked as unhealthy and subsequent requests wait.
- `RATE_LIMIT_COOLDOWN` — cooldown after an explicit HTTP 429 rate limit (60 seconds). A 429 is a stronger signal than a generic 5xx, so the cooldown is doubled.

### Helper function `now_millis()`

```rust
fn now_millis() -> u64
```

Returns the current time in milliseconds since the Unix epoch. Used for cooldown expiry comparisons. Falls back to `0` on system time errors (which should never happen in practice).

### `ModelSemaphore`

A bounded per-model semaphore keyed by model id.

```rust
#[derive(Clone)]
pub struct ModelSemaphore {
    lanes: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    permits: usize,
}
```

- `lanes` — a shared `HashMap<String, Arc<Semaphore>>` protected by a `Mutex`. Each model name maps to an independent semaphore.
- `permits` — the maximum number of concurrent permits per semaphore.

**Methods:**

- `new(permits: usize) -> Self` — creates a new `ModelSemaphore` with the given number of permits (minimum 1).
- `async acquire<'a, T>(&'a self, model: &str, f: impl Future<Output = T> + Send) -> T` — acquires a permit for the given model, runs `f` while holding it, then releases. The semaphore for the model is created lazily on first access.
- `Default` — creates a `ModelSemaphore` with `DEFAULT_LANE_CONCURRENCY` (3) permits.

**How it works**: When `acquire` is called, the method first looks up (or creates) the semaphore for the given model name, then calls `sem.acquire().await` to obtain a permit. The `_permit` guard is dropped at the end of the closure, releasing the semaphore slot. Because `Arc<Semaphore>` is cloned, the lock on the `HashMap` is held only briefly during lookup — the actual semaphore wait is lock-free.

### `FallbackCooldown`

A 429 / 5xx-aware cooldown per model lane.

```rust
#[derive(Clone)]
pub struct FallbackCooldown {
    expires: Arc<Mutex<HashMap<String, u64>>>,
    default: Duration,    // cooldown for generic 5xx
    rate_limit: Duration, // cooldown for explicit 429
}
```

- `expires` — a shared `HashMap<String, u64>` mapping model names to cooldown expiry timestamps (milliseconds since epoch).
- `default` — cooldown window for a generic limit signal (30 seconds).
- `rate_limit` — cooldown window for an explicit HTTP 429 (60 seconds).

**Methods:**

- `new(default: Duration, rate_limit: Duration) -> Self` — creates a new `FallbackCooldown` with custom durations.
- `async note_limit(&self, model: &str, is_rate_limit: bool)` — records that `model` was throttled. If `is_rate_limit` is true (HTTP 429), uses the longer `rate_limit` window; otherwise uses `default`.
- `async is_cooldown(&self, model: &str) -> bool` — returns whether `model` is currently in cooldown. Lazily prunes stale entries (if the expiry has passed, removes the entry and returns `false`).
- `async wait_hint(&self, model: &str) -> Option<Duration>` — if the lane is cooling down, returns the remaining wait time; otherwise `None`.
- `Default` — creates a `FallbackCooldown` with `DEFAULT_COOLDOWN` (30s) and `RATE_LIMIT_COOLDOWN` (60s).

**How it works**: The `DeepSeekProvider::complete()` method calls `cooldown.note_limit()` when it receives an HTTP 429 or 5xx error. Before the next `complete()` call, it checks `cooldown.is_cooldown()` — if the model is still cooling down, it waits for the remaining duration. This prevents the swarm from re-hammering a rate-limited endpoint round after round from separate agents.

---

## factory.rs

The file [factory.rs](file:///Users/yakushev/Documents/GitHub/Parallel/research-agent/crates/llm/src/factory.rs) provides a factory function for creating a provider from configuration. It supports both a primary model and an optional "fast" model for high-volume auxiliary calls.

### List of known providers

```rust
const OPENAI_COMPATIBLE: &[&str] = &[
    "deepseek", "openai", "openrouter", "ollama", "vllm", "lmstudio", "openai-compatible",
];
```

### Function `build_provider(cfg: &LlmConfig) -> anyhow::Result<Arc<dyn LlmProvider>>`

Algorithm:

1. **API key check**: if `cfg.api_key.trim().is_empty()` — returns an error with instructions on setting up `config.toml`. The caller decides whether a missing key is fatal (e.g., `serve` can start without a key and refuse session creation).
2. **base_url check**: if empty — returns an error.
3. **Provider name check**: if `cfg.provider` (lowercase) is not in `OPENAI_COMPATIBLE` — logs `tracing::warn` with a warning, but **does not reject** the configuration. Unknown providers are assumed to potentially be OpenAI-compatible as well.
4. Creates `DeepSeekProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model)`.
5. Applies `.with_provider_name(cfg.provider.clone())` — so that `name()` returns the name from the configuration.
6. Wraps in `Arc::new(...)` and returns.

### Function `build_fast_provider(cfg: &LlmConfig) -> anyhow::Result<Option<Arc<dyn LlmProvider>>>`

This function builds an optional **cheap/fast model** for high-volume auxiliary calls. Use cases include:

- Entity extraction from documents
- Memory classification and summarization
- Search result reranking
- Any other lightweight inference task where using the primary model would be overkill

Algorithm:

1. Reads `cfg.fast_model` (trimmed). If empty or identical to the primary `cfg.model`, returns `None` — the caller should fall back to the primary model.
2. Clones the config and replaces `model` with the fast model name.
3. Delegates to `build_provider(&fast)` to create the provider instance.

**Multi-model routing**: The factory pattern enables a clean separation of concerns. The primary provider handles complex reasoning (agent planning, tool selection, synthesis), while the fast provider handles cheap, repetitive tasks. Both use the same `base_url` and `api_key` — only the model name differs. If the fast model is not configured, the system gracefully falls back to the primary model.

**Note**: `LlmConfig` (from `pr_core`) contains fields: `provider`, `base_url`, `api_key`, `model`, `fast_model`, `max_tokens`, `temperature`. Of these, only `provider`, `base_url`, `api_key`, `model`, and `fast_model` are used in the factory. The `max_tokens` and `temperature` fields are ignored at the factory level — they should be set in `CompletionRequest` by the calling code.