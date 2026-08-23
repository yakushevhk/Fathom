# Crate Documentation `crates/llm`

The `llm` crate is responsible for interacting with language models via the OpenAI-compatible chat-completions protocol. It includes typed request/response structures, a trait provider, a single `DeepSeekProvider` implementation (compatible with any OpenAI-compatible endpoint), a retry mechanism with exponential backoff, a provider factory, and per-model concurrency throttles. The default runtime model is `deepseek-chat`; the provider is OpenAI-compatible rather than a native Anthropic or Gemini adapter. Anthropic Messages API and Google Gemini API require separate provider implementations.

---

## Table of Contents

1. [lib.rs — entry point](#librs)
2. [types.rs — data types](#typesrs)
3. [provider.rs — LlmProvider trait](#providerrs)
4. [deepseek.rs — DeepSeekProvider implementation](#deepseekrs)
5. [retry.rs — retry mechanism](#retryrs)
6. [factory.rs — provider factory](#factoryrs)
7. [concurrency.rs — per-model throttles](#concurrencyrs)

---

The file [lib.rs](../../crates/llm/src/lib.rs) declares six public modules and re-exports all their contents via `pub use`:

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

This means consumers of the crate can write `use llm::LlmProvider`, `use llm::CompletionRequest`, `use llm::ModelSemaphore`, etc. directly, without specifying the internal modules.

---

## types.rs

The file [types.rs](../../crates/llm/src/types.rs) defines four key structures and one enum that serve as the common currency (lingua franca) of the entire LLM layer.

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
| `messages` | `Vec<Message>` | Full dialog history. Each `Message` is an enum from `pr_core` containing roles `System`, `User`, `Assistant` (with optional `content` and `Vec<ToolCall>`) and `Tool` (tool execution result). |
| `tools` | `Vec<ToolSchema>` | List of available tools. Each `ToolSchema` contains `name: String`, `description: String`, `parameters: serde_json::Value` (JSON Schema). During serialization, the field is omitted if the vector is empty — this saves bandwidth when tools are not needed. |
| `temperature` | `Option<f32>` | Sampling temperature. Omitted when `None`. |
| `max_tokens` | `Option<u32>` | Maximum number of tokens in the response. Omitted when `None`. |
| `stream` | `bool` | Streaming flag. Defaults to `false`. The server side uses it to select the response format (SSE vs JSON). |

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
| `message` | `Message` | Model response. Typically `Message::Assistant { content, tool_calls }`. If the model calls tools, `content` may be `None` and `tool_calls` may contain one or more calls. |
| `usage` | `Option<Usage>` | Token statistics (prompt/completion/total). May be `None` if the provider did not return this information. |
| `finish_reason` | `Option<String>` | Reason for generation completion. Standard values: `"stop"` (model stopped on its own), `"length"` (token limit reached), `"tool_calls"` (model requested a tool call). |

### `Usage`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

Three fields — an exact copy of the `usage` structure from the OpenAI API. `total_tokens` always equals `prompt_tokens + completion_tokens`.

### `StreamChunk`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamChunk {
    Text { delta: String },
    ToolCallDelta { id: String, name: String, arguments_delta: String },
    Done { message: Message, usage: Option<Usage>, finish_reason: Option<String> },
    Error { message: String },
}
```

Enum with an internal `"type"` tag (serde attribute `#[serde(tag = "type")]`). Each variant represents one "step" of the stream:

- **`Text`** — the next fragment of the model's text response. `delta` contains incremental text.
- **`ToolCallDelta`** — an incremental fragment of a tool call. `id` is the call identifier, `name` is the function name, `arguments_delta` is a partial JSON-arguments string (the model generates JSON arguments character by character).
- **`Done`** — stream completion signal. Contains the final `Message` (usually empty, since content was already delivered via `Text`/`ToolCallDelta`), `usage` statistics, and `finish_reason`.
- **`Error`** — an error that occurred during streaming.

---

## provider.rs

The file [provider.rs](../../crates/llm/src/provider.rs) defines the `LlmProvider` trait — an abstraction for any LLM provider.

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
| `name()` | Logical provider name (e.g. `"deepseek"`, `"openrouter"`). Used for logging and identification. |
| `model()` | Model name (e.g. `"deepseek-chat"`). |
| `complete()` | Full (non-streaming) LLM request. Returns `CompletionResponse` in its entirety. Internally may use retry and streaming fallback for large responses. |
| `stream()` | Streaming request. Returns `Box<dyn Stream<Item = PrResult<StreamChunk>>>` — an asynchronous stream of chunks that the consumer reads in a loop. |

The trait requires `Send + Sync`, which allows safe use of the provider from multiple tokio tasks simultaneously.

---

## deepseek.rs

The file [deepseek.rs](../../crates/llm/src/deepseek.rs) is the main and only implementation of `LlmProvider`. Despite the name, it works with any API compatible with OpenAI chat-completions.

### Constants

```rust
const MAX_RESPONSE_BYTES: usize = 50 * 1024 * 1024;       // 50 MB
const MAX_RETRIES: u32 = 3;
const STREAMING_THRESHOLD_BYTES: u64 = 10 * 1024 * 1024;  // 10 MB
```

- `MAX_RESPONSE_BYTES` — absolute limit on response body size. If the response exceeds 50 MB, an error is generated.
- `MAX_RETRIES` — number of retry attempts for HTTP requests (passed to `with_retry`).
- `STREAMING_THRESHOLD_BYTES` — threshold beyond which the client proactively switches to streaming, without waiting for the entire body to load.

### Helper function `safe_prefix`

```rust
fn safe_prefix(s: &str, max_bytes: usize) -> &str
```

Algorithm:
1. Computes `end = min(len(s), max_bytes)`.
2. If `end` falls inside a multi-byte UTF-8 character (is not a char boundary), decrements `end` by 1 until it finds a character boundary.
3. Returns `&s[..end]`.

Used to safely truncate the response body when forming error messages, so as not to break UTF-8 validity.

### `DeepSeekProvider` structure

```rust
pub struct DeepSeekProvider {
    base_url: String,       // base API URL (without trailing /)
    api_key: String,        // API key
    model: String,          // model name
    http: reqwest::Client,  // HTTP client with 5-minute timeout
    provider_name: String,  // logical name (default "deepseek")
}
```

#### Constructor `new(base_url, api_key, model)`

Algorithm:
1. Creates `reqwest::Client::builder()` with a 300-second (5-minute) timeout.
2. Calls `.build().expect(...)` — panics if the HTTP client cannot be created.
3. Strips trailing `/` from `base_url`.
4. Sets `provider_name = "deepseek"`.

#### Method `with_provider_name(self, name)`

Builder method (consumes self, returns self). Overrides `provider_name`. Used by the factory when the configuration specifies a different provider (e.g. `"openrouter"`).

#### Method `build_request_body(&self, req, stream) -> serde_json::Value`

Algorithm for building the JSON request body:
1. Creates a base JSON with fields `model`, `messages`, `stream`.
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
5. If `stream == true`, adds the field `"stream_options": {"include_usage": true}` — this is an OpenAI/DeepSeek-specific option that forces the server to send `usage` statistics in the last SSE chunk.

### Internal structures for API response deserialization

```rust
struct ApiResponse { choices: Vec<ApiChoice>, usage: Option<ApiUsage> }
struct ApiChoice { message: Option<ApiMessage>, delta: Option<ApiMessage>, finish_reason: Option<String> }
struct ApiMessage { content: Option<String>, tool_calls: Vec<ApiToolCall> }
struct ApiToolCall { id: Option<String>, call_type: Option<String>, function: Option<ApiFunction> }
struct ApiFunction { name: Option<String>, arguments: Option<String> }
struct ApiUsage { prompt_tokens: u32, completion_tokens: u32, total_tokens: u32 }
```

- `ApiChoice.message` is used in non-streaming responses.
- `ApiChoice.delta` is used in streaming responses (SSE chunks).
- Both fields (`message` and `delta`) deserialize into the same `ApiMessage` structure, since their schemas are identical.
- `ApiToolCall.function.arguments` is a JSON **string**, not an object. This is a feature of the OpenAI API: arguments are passed as a serialized JSON string.

### Method `parse_response(text: &str) -> PrResult<CompletionResponse>`

Algorithm for parsing a non-streaming JSON response:

1. **Deserialization**: `serde_json::from_str::<ApiResponse>(text)`. On error, creates `PrError::Llm` with a body preview (first 500 bytes via `safe_prefix`).
2. **Extract the first choice**: takes `api_resp.choices.into_iter().next()`. If the array is empty — error `"no choices in response"`.
3. **Extract message**: `choice.message.ok_or_else(...)`. In non-streaming mode, `message` is always present.
4. **Extract tool_calls**: iterates over `api_msg.tool_calls`, for each element:
   - Takes `function` (if `None` — the element is skipped via `filter_map`).
   - Takes `function.name` (if `None` — the element is skipped).
   - Parses `function.arguments` as JSON. If the string is `None` or parsing fails, `{}` is used as fallback.
   - Creates `ToolCall::new(id, name, arguments)`.
   - `id` is taken from `tc.id.unwrap_or_default()` (empty string if absent).
5. **Assemble Message**: `Message::assistant_with_tools(content, tool_calls)` — creates an `Assistant` variant with text and a list of calls.
6. **Map Usage**: `api_resp.usage` is mapped from `ApiUsage` to `Usage` (identical fields).
7. **Return**: `CompletionResponse { message, usage, finish_reason }`.

### Method `complete_via_streaming(&self, req) -> PrResult<CompletionResponse>`

Fallback strategy: when a non-streaming request received a response that is too large (ResponseTooLarge), this method assembles the full response from streaming chunks.

Algorithm:
1. Calls `self.stream(req)` to obtain a streaming stream.
2. Initializes: `content = String::new()`, `usage = None`, `finish_reason = None`.
3. In a loop `while let Some(chunk) = stream.next().await`:
   - `StreamChunk::Text { delta }`: checks whether `content.len() + delta.len()` would exceed the `MAX_RESPONSE_BYTES` limit. If so, returns an error. Otherwise appends `delta` to `content`.
   - `StreamChunk::Done { usage, finish_reason, .. }`: saves `usage` and `finish_reason`.
   - `StreamChunk::ToolCallDelta` and `StreamChunk::Error`: ignored.
4. Returns `CompletionResponse` with `Message::assistant(content)` (without tool_calls), collected `usage`, and `finish_reason`.

**Important**: during streaming fallback, tool_calls are lost — this is intentional, since the fallback is only needed for text responses too large for buffering.

### `LlmProvider` implementation for `DeepSeekProvider`

#### `name()` and `model()`

Return `self.provider_name` and `self.model` respectively.

#### `complete(&self, req) -> PrResult<CompletionResponse>`

This is the main method. Step-by-step algorithm:

**Step 1 — Preparation:**
- Calls `build_request_body(req, false)` to get the JSON body (stream = false).
- Forms the URL: `"{base_url}/chat/completions"`.
- Clones `api_key` and `http` client for the closure.
- Serializes the body to a string **once** (`serde_json::to_string(&body)`), to avoid re-serializing on each retry attempt.

**Step 2 — HTTP request with retry (non-streaming):**

Calls `with_retry(closure, MAX_RETRIES)`. The closure on each attempt:

1. Sends `POST {url}` with headers:
   - `Authorization: Bearer {api_key}`
   - `Content-Type: application/json`
   - Body: the previously serialized JSON string.
2. Handles send errors: classifies as `"timeout"`, `"decode"`, or `"connect"` and wraps in `PrError::Llm`.
3. **Content-Length check**: if the server returned a `Content-Length` header greater than `STREAMING_THRESHOLD_BYTES` (10 MB), immediately returns `PrError::ResponseTooLarge(...)` — this signals a fallback to streaming.
4. **Extract Retry-After**: reads the `Retry-After` header from the response, parses as `u64` (seconds). If the header is absent or unparseable — `retry_after = None`.
5. **Read body**: `response.text().await`. On read error, classifies as `"decode"`, `"timeout"`, or `"body read"`.
6. **Post-buffer size check**: if `text.len() > MAX_RESPONSE_BYTES` (50 MB), returns `PrError::ResponseTooLarge`.
7. **HTTP status check**: if the status is not 2xx, returns `PrError::Http { status, message (first 2000 bytes of body), retry_after }`.
8. On success, returns `Ok(text)`.

**Step 3 — Result handling:**
- `Ok(text)` → calls `Self::parse_response(&text)` and returns the result.
- `Err(PrError::ResponseTooLarge(_))` → logs a warning and proceeds to step 4.
- Any other error → returns it immediately.

**Step 4 — Streaming fallback:**
- Calls `self.complete_via_streaming(req).await` and returns the result.

#### `stream(&self, req) -> PrResult<Box<dyn Stream<...>>>`

Streaming request. Algorithm:

**Step 1 — Sending the request:**
- Calls `build_request_body(req, true)` (stream = true, includes `stream_options`).
- Forms the URL: `"{base_url}/chat/completions"`.
- Sends `POST {url}` with headers:
  - `Authorization: Bearer {api_key}`
  - Body serialized via `.json(&body)` (unlike `complete()`, where the body is pre-serialized to a string).
- On send error, returns `PrError::Llm`.
- Checks HTTP status: if not 2xx, reads the body and returns `PrError::Llm("API error {status}: {text}")`.

**Step 2 — Preparing the byte stream:**
- `response.bytes_stream()` returns `impl Stream<Item = Result<Bytes, reqwest::Error>>`.
- Wraps in `Box::pin(...)` to pin it in memory.

**Step 3 — SSE decoding with line buffering:**

Uses `futures::stream::try_unfold` with state `(byte_stream, remainder: Vec<u8>)`.

Algorithm of the unfold closure (called for each element of the output stream):

1. **Search for a line in the buffer**: looks for the position of the first `b'\n'` in `remainder`.
   - If found: extracts all bytes up to and including `\n` into `line_bytes`, keeps the rest in `remainder`. Decodes `line_bytes` via `String::from_utf8_lossy`, trims whitespace.
     - If the line is empty — skips (empty lines are SSE event separators).
     - Calls `parse_sse_line(line)`.
       - If it returns `Some(chunk)` — returns `Ok(Some((chunk, (byte_stream, remainder))))`.
       - If `None` — skips (e.g. line `data: [DONE]`) and goes to the next iteration.
   - If `\n` is not found — proceeds to step 2.
2. **Read from the byte stream**: `byte_stream.next().await`.
   - `Some(Ok(bytes))` — appends `bytes` to `remainder` and returns to step 1.
   - `Some(Err(e))` — returns `PrError::Llm("stream error: {e}")`.
   - `None` (stream ended):
     - If `remainder` is not empty — processes the remaining bytes as the last line (without trailing `\n`): `from_utf8_lossy`, `trim`, `parse_sse_line`.
     - Returns `Ok(None)` — end of stream.

**Key feature**: `remainder` persists between unfold calls, guaranteeing correct handling of HTTP chunks that may cut SSE lines at any point (even inside a multi-byte UTF-8 character, since `\n` (0x0A) cannot appear inside a UTF-8 sequence).

### Function `parse_sse_line(line: &str) -> Option<StreamChunk>`

Algorithm for parsing a single SSE line:

1. Extracts the `data: ` prefix via `strip_prefix("data: ")`. If the prefix is absent — returns `None` (the line is a comment, `event:`, `id:`, etc.).
2. If `data == "[DONE]"` — returns `None` (SSE termination signal, does not produce a chunk).
3. Deserializes `data` as `ApiResponse`. On error — returns `None`.
4. Takes the first `choice` from `api_resp.choices`. If the array is empty — `None`.
5. **Delta check**: if `choice.delta` exists:
   - If `delta.content` exists and is not empty — returns `StreamChunk::Text { delta: content.clone() }`.
6. **Finish_reason/usage check**: if `choice.finish_reason.is_some()` OR `api_resp.usage.is_some()`:
   - Maps `api_resp.usage` to `Usage`.
   - Returns `StreamChunk::Done { message: Message::assistant(""), usage, finish_reason }`.
7. If none of the conditions matched — returns `None`.

**Note**: in the current implementation, `parse_sse_line` does not handle `ToolCallDelta` — if the model generates tool_calls via streaming, they are skipped (returns `None`). This is a limitation that may be relevant when using streaming with tools.

---

## retry.rs

The file [retry.rs](../../crates/llm/src/retry.rs) implements a generic retry mechanism with exponential backoff.

### Global jitter counter

```rust
static JITTER_SEQ: AtomicU64 = AtomicU64::new(0);
```

An atomic counter used as a deterministic source of "pseudo-randomness" for jitter. Does not require a `rand` dependency. Each call to `jitter()` atomically increments the counter, ensuring different values for concurrent tasks.

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

The main retry-logic function. Parameters:
- `f: F` — closure returning `Future<Output = PrResult<T>>`. Called on each attempt.
- `max_retries: u32` — maximum number of **retry** attempts (not counting the first).

Step-by-step algorithm:

1. Initializes `delay = 500ms` (initial delay before the first retry attempt).
2. Loop `for attempt in 0..=max_retries` (includes attempt 0 — the first attempt):
   - Calls `f().await`.
   - **Success**: returns `Ok(val)`.
   - **Retryable error** (`attempt < max_retries && e.is_retryable()`):
     1. Checks `e.retry_after_secs()` — if the error contains Retry-After (e.g. HTTP 429), uses it, but capped at 60 seconds.
     2. If Retry-After is absent, uses the current `delay`.
     3. Applies `jitter(wait)` to the chosen delay.
     4. Logs a warning with the attempt number and delay.
     5. `tokio::time::sleep(wait).await` — wait.
     6. Increases `delay = (delay * 2).min(60s)` — exponential growth, capped at 60 seconds.
   - **Non-retryable error** (or attempts exhausted): returns `Err(e)` immediately.
3. `unreachable!()` — if the loop ended without a return (should not occur).

**Delay schedule** (without jitter and Retry-After):
- After attempt 0: 500ms → next attempt 1
- After attempt 1: 1000ms → next attempt 2
- After attempt 2: 2000ms → next attempt 3
- Cap: 60 seconds

**Retryable error classification** is determined by the `e.is_retryable()` method from `PrError` (in the `pr_core` crate). From the tests, retryable errors include:
- HTTP 408 (Request Timeout)
- HTTP 429 (Too Many Requests)
- HTTP 5xx (server errors)
- Network errors (timeout, connect)

**Non-retryable** errors:
- HTTP 400 (Bad Request)
- HTTP 401 (Unauthorized)
- HTTP 403 (Forbidden)
- HTTP 404 (Not Found)
- `PrError::ResponseTooLarge`
- `PrError::Llm` (general parsing/format errors)

---

## factory.rs

The file [factory.rs](../../crates/llm/src/factory.rs) provides a factory function for creating a provider from configuration.

### List of known providers

```rust
const OPENAI_COMPATIBLE: &[&str] = &[
    "deepseek", "openai", "openrouter", "ollama", "vllm", "lmstudio", "openai-compatible",
];
```

### Function `build_provider(cfg: &LlmConfig) -> anyhow::Result<Arc<dyn LlmProvider>>`

Algorithm:

1. **API key check**: if `cfg.api_key.trim().is_empty()` — returns an error with instructions on setting up `config.toml`.
2. **base_url check**: if empty — returns an error.
3. **Provider name check**: if `cfg.provider` (lowercased) is not in `OPENAI_COMPATIBLE` — logs a `tracing::warn` warning, but does **not reject** the configuration. Unknown providers are assumed to also be OpenAI-compatible.
4. Creates `DeepSeekProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model)`.
5. Applies `.with_provider_name(cfg.provider.clone())` — so that `name()` returns the name from the configuration.
6. Wraps in `Arc::new(...)` and returns.

---

## concurrency.rs

The file [concurrency.rs](../../crates/llm/src/concurrency.rs) provides per-model concurrency throttles to prevent a multi-agent swarm from self-inflicting provider rate limits. Two orthogonal throttles are implemented:

### Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `DEFAULT_LANE_CONCURRENCY` | 3 | Default concurrent requests allowed per model lane |
| `DEFAULT_COOLDOWN` | 30 s | Default cooldown after a generic 5xx before retry |
| `RATE_LIMIT_COOLDOWN` | 60 s | Cooldown after an explicit HTTP 429 (stricter) |

### `ModelSemaphore`

A bounded per-model semaphore keyed by model id. Prevents a fan-out of sub-agents from saturating a single model with too many concurrent requests.

```rust
pub struct ModelSemaphore {
    lanes: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    permits: usize,  // default 3
}
```

| Method | Description |
|--------|-------------|
| `new(permits)` | Create with the given concurrency limit per lane (minimum 1). |
| `acquire(model, f)` | Acquire a permit for `model`, run `f` while holding it, then release. Each model gets its own independent semaphore lane, so different models do not block each other. |

Each model lane is created lazily on first use. Two models with `permits=1` can still run concurrently because their lanes are independent.

### `FallbackCooldown`

A 429/5xx-aware cooldown per model lane. After the provider signals throttling, the lane is placed in cooldown and refuses to be used until the window elapses — preventing the swarm from hammering a rate-limited endpoint round after round.

```rust
pub struct FallbackCooldown {
    expires: Arc<Mutex<HashMap<String, u64>>>,
    default: Duration,      // 30 s for generic 5xx
    rate_limit: Duration,   // 60 s for explicit 429
}
```

| Method | Description |
|--------|-------------|
| `new(default, rate_limit)` | Create with configurable cooldown windows. |
| `note_limit(model, is_rate_limit)` | Record that `model` was throttled. `is_rate_limit=true` selects the longer window (60 s) for HTTP 429; `false` uses the shorter window (30 s) for generic 5xx. |
| `is_cooldown(model)` | Whether `model` is currently in cooldown. Lazily prunes expired entries. |
| `wait_hint(model)` | Returns `Some(Duration)` if cooling down (time remaining), or `None` if available. |

### Usage pattern

The swarm agent runtime wraps LLM calls with both throttles:

```rust
semaphore.acquire(model, async {
    if cooldown.is_cooldown(model).await {
        // skip or fallback to another model
    }
    match provider.complete(&req).await {
        Ok(resp) => resp,
        Err(e) if e.is_rate_limit() => {
            cooldown.note_limit(model, true).await;
            Err(e)
        }
        Err(e) if e.is_server_error() => {
            cooldown.note_limit(model, false).await;
            Err(e)
        }
        Err(e) => Err(e),
    }
}).await
```

This ensures that the swarm naturally backs off when the provider is under load, without any central coordinator or shared state beyond these two primitives.