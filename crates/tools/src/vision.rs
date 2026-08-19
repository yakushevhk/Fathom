//! Vision / image analysis via a Qwen VL model behind an OpenAI-compatible
//! API (router.y7.hk by default).
//!
//! `analyze_image` accepts either a local file path or an http(s) URL. Local
//! files are read, base64-encoded, and embedded as `data:` URLs in the
//! request; remote URLs are passed through unchanged.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::registry::{Tool, ToolContext};

/// Default OpenAI-compatible API base for vision models.
pub const DEFAULT_VISION_API_BASE: &str = "https://router.y7.hk/v1";
/// Default API key (router.y7.hk house key).
pub const DEFAULT_VISION_API_KEY: &str = "sk-haus";
/// Default vision model.
pub const DEFAULT_VISION_MODEL: &str = "qwen-vl-max";

/// Environment variables that override the defaults.
pub const VISION_API_BASE_ENV: &str = "PARALLEL_VISION_API_BASE";
pub const VISION_API_KEY_ENV: &str = "PARALLEL_VISION_API_KEY";
pub const VISION_MODEL_ENV: &str = "PARALLEL_VISION_MODEL";

/// Reject images larger than this (keeps request payloads sane).
const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
/// Timeout for the vision API call.
const VISION_TIMEOUT_SECS: u64 = 120;

pub fn vision_api_base_from_env() -> String {
    std::env::var(VISION_API_BASE_ENV).unwrap_or_else(|_| DEFAULT_VISION_API_BASE.to_string())
}

pub fn vision_api_key_from_env() -> String {
    std::env::var(VISION_API_KEY_ENV).unwrap_or_else(|_| DEFAULT_VISION_API_KEY.to_string())
}

pub fn vision_model_from_env() -> String {
    std::env::var(VISION_MODEL_ENV).unwrap_or_else(|_| DEFAULT_VISION_MODEL.to_string())
}

/// Vision analysis tool backed by an OpenAI-compatible chat-completions
/// endpoint with `image_url` content support.
#[derive(Debug, Clone)]
pub struct VisionTool {
    /// API base URL (e.g. `https://router.y7.hk/v1`). When empty, the value
    /// from [`ToolContext`] (env-backed) is used at execution time.
    pub api_base: String,
    /// API key. When empty, the value from [`ToolContext`] is used.
    pub api_key: String,
    /// Vision model name (e.g. `qwen-vl-max`).
    pub model: String,
}

impl VisionTool {
    /// Create a tool that resolves endpoint/key from the `ToolContext` at
    /// execution time.
    pub fn new() -> Self {
        Self {
            api_base: String::new(),
            api_key: String::new(),
            model: vision_model_from_env(),
        }
    }

    /// Create a tool with an explicit configuration.
    pub fn with_config(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: api_key.into(),
            model: model.into(),
        }
    }
}

impl Default for VisionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AnalyzeImageParams {
    /// Image to analyze: a local file path or an http(s) URL
    image: String,
    /// What to describe or analyze (default: detailed description + OCR)
    #[serde(default = "default_prompt")]
    prompt: String,
    /// Maximum tokens in the model's reply (default: 1024)
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
}

fn default_prompt() -> String {
    "Describe this image in detail. Include all visible text (OCR), objects, people, layout, and anything noteworthy."
        .to_string()
}

fn default_max_tokens() -> u32 {
    1024
}

/// Guess a MIME type from a file extension (defaults to image/png).
pub(crate) fn mime_from_extension(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/png",
    }
}

/// Resolve the image argument into something the API can fetch: http(s)/data
/// URLs are passed through; local files become base64 `data:` URLs.
pub(crate) fn resolve_image_source(image: &str, working_dir: &Path) -> anyhow::Result<String> {
    if image.starts_with("http://") || image.starts_with("https://") || image.starts_with("data:")
    {
        return Ok(image.to_string());
    }
    let path = Path::new(image);
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };
    let meta = std::fs::metadata(&full)
        .map_err(|e| anyhow::anyhow!("cannot read image {}: {e}", full.display()))?;
    if meta.len() > MAX_IMAGE_BYTES {
        anyhow::bail!(
            "image {} is too large ({} bytes, limit {} bytes)",
            full.display(),
            meta.len(),
            MAX_IMAGE_BYTES
        );
    }
    let bytes = std::fs::read(&full)
        .map_err(|e| anyhow::anyhow!("failed to read image {}: {e}", full.display()))?;
    let mime = mime_from_extension(&full);
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

/// Build the OpenAI-compatible chat-completions request body.
pub(crate) fn build_request_body(model: &str, prompt: &str, image_url: &str, max_tokens: u32) -> Value {
    json!({
        "model": model,
        "max_tokens": max_tokens,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": prompt },
                { "type": "image_url", "image_url": { "url": image_url } }
            ]
        }]
    })
}

/// Extract the assistant reply from a chat-completions response.
pub(crate) fn extract_reply(body: &Value) -> Option<String> {
    let content = body
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

#[async_trait]
impl Tool for VisionTool {
    fn name(&self) -> &str {
        "analyze_image"
    }
    fn description(&self) -> &str {
        "Analyze an image (local file or URL) with a vision-language model and return a textual description.

## Capability

Sends the image to a Qwen VL model through an OpenAI-compatible vision API. Local files are base64-encoded inline (up to 20 MB); http(s) URLs are passed directly. The model describes content, layout, and transcribes any visible text (OCR).

## When to Use

- Reading screenshots, charts, diagrams, or scanned documents.
- OCR of images found during research (pass the image URL directly).
- Describing photos or visual evidence referenced by sources.

## When NOT to Use

- Text-only documents (HTML, Markdown): use `file_read` or `web_fetch`.
- PDFs: use `pdf_extract` instead.

## Parameters

- `image` (required): local path (resolved against the working directory) or http(s) URL.
- `prompt` (optional): the question/instruction for the model. Default asks for a detailed description with OCR.
- `max_tokens` (optional, default 1024): reply length cap.

## Failure Modes

- File not found / unreadable: check the path.
- HTTP errors from the vision API: reported with status and a snippet of the response body."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(AnalyzeImageParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: AnalyzeImageParams = serde_json::from_value(args)?;

        let api_base = if self.api_base.is_empty() {
            ctx.vision_api_base.clone()
        } else {
            self.api_base.clone()
        };
        let api_key = if self.api_key.is_empty() {
            ctx.vision_api_key.clone()
        } else {
            self.api_key.clone()
        };
        if api_base.is_empty() || api_key.is_empty() {
            return Ok(ToolOutput::err(
                "vision API is not configured (missing api_base or api_key)",
            ));
        }

        let image_url = match resolve_image_source(&params.image, &ctx.working_dir) {
            Ok(u) => u,
            Err(e) => return Ok(ToolOutput::err(e.to_string())),
        };

        let body = build_request_body(&self.model, &params.prompt, &image_url, params.max_tokens);
        let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

        let resp = match ctx
            .http_client
            .post(&url)
            .bearer_auth(&api_key)
            .json(&body)
            .timeout(Duration::from_secs(VISION_TIMEOUT_SECS))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::err(format!("vision request failed: {e}"))),
        };

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Ok(ToolOutput::err(format!(
                "vision API returned HTTP {status}: {}",
                truncate_str(&text, 500)
            )));
        }

        let parsed: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                return Ok(ToolOutput::err(format!(
                    "invalid vision API response: {e}; body: {}",
                    truncate_str(&text, 300)
                )))
            }
        };

        match extract_reply(&parsed) {
            Some(reply) => Ok(ToolOutput::ok_with_meta(
                reply,
                json!({ "model": self.model, "image": params.image }),
            )),
            None => Ok(ToolOutput::err(format!(
                "vision API returned no analysis: {}",
                truncate_str(&text, 500)
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> ToolContext {
        ToolContext::new(std::env::temp_dir(), pr_core::SearchConfig::default())
    }

    #[test]
    fn test_mime_from_extension() {
        assert_eq!(mime_from_extension(Path::new("a.png")), "image/png");
        assert_eq!(mime_from_extension(Path::new("a.JPG")), "image/jpeg");
        assert_eq!(mime_from_extension(Path::new("a.jpeg")), "image/jpeg");
        assert_eq!(mime_from_extension(Path::new("a.webp")), "image/webp");
        assert_eq!(mime_from_extension(Path::new("a.gif")), "image/gif");
        assert_eq!(mime_from_extension(Path::new("a.unknown")), "image/png");
    }

    #[test]
    fn test_resolve_image_source_passthrough() {
        let wd = Path::new("/tmp");
        assert_eq!(
            resolve_image_source("https://example.com/x.png", wd).unwrap(),
            "https://example.com/x.png"
        );
        assert_eq!(
            resolve_image_source("http://example.com/x.png", wd).unwrap(),
            "http://example.com/x.png"
        );
        assert_eq!(
            resolve_image_source("data:image/png;base64,AAA", wd).unwrap(),
            "data:image/png;base64,AAA"
        );
    }

    #[test]
    fn test_resolve_image_source_local_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("pic.png");
        // 1x1 transparent PNG
        std::fs::write(&file_path, b"\x89PNG fake bytes").unwrap();

        let resolved = resolve_image_source("pic.png", tmp.path()).unwrap();
        assert!(resolved.starts_with("data:image/png;base64,"));

        let resolved_abs = resolve_image_source(file_path.to_str().unwrap(), tmp.path()).unwrap();
        assert!(resolved_abs.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_resolve_image_source_missing_file() {
        let err = resolve_image_source("/nonexistent/definitely_missing.png", Path::new("/tmp"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("cannot read image"));
    }

    #[test]
    fn test_build_request_body() {
        let body = build_request_body("qwen-vl-max", "describe", "https://x/y.png", 512);
        assert_eq!(body["model"], "qwen-vl-max");
        assert_eq!(body["max_tokens"], 512);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "describe");
        assert_eq!(content[1]["type"], "image_url");
        assert_eq!(content[1]["image_url"]["url"], "https://x/y.png");
    }

    #[test]
    fn test_extract_reply() {
        let body = serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "A cat." } }]
        });
        assert_eq!(extract_reply(&body), Some("A cat.".to_string()));

        let empty = serde_json::json!({
            "choices": [{ "message": { "content": "   " } }]
        });
        assert_eq!(extract_reply(&empty), None);

        assert_eq!(extract_reply(&serde_json::json!({})), None);
    }

    #[test]
    fn test_analyze_image_params_defaults() {
        let params: AnalyzeImageParams =
            serde_json::from_value(serde_json::json!({"image": "x.png"})).unwrap();
        assert_eq!(params.max_tokens, 1024);
        assert!(params.prompt.contains("Describe this image"));
    }

    #[test]
    fn test_vision_tool_default_config_resolution() {
        let tool = VisionTool::new();
        assert!(tool.api_base.is_empty());
        assert!(tool.api_key.is_empty());
        assert!(!tool.model.is_empty());
        let ctx = test_ctx();
        // Context defaults come from env or the built-in constants.
        assert!(!ctx.vision_api_base.is_empty());
        assert!(!ctx.vision_api_key.is_empty());
    }

    #[tokio::test]
    async fn test_analyze_image_missing_file_returns_err_output() {
        let tool = VisionTool::new();
        let ctx = test_ctx();
        let out = tool
            .execute(
                serde_json::json!({"image": "/nonexistent/nope.png"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("cannot read image"));
    }

    #[tokio::test]
    async fn test_analyze_image_unreachable_api_returns_err_output() {
        // Local file that exists, but the API endpoint is unreachable.
        let tmp = tempfile::TempDir::new().unwrap();
        let file_path = tmp.path().join("pic.png");
        std::fs::write(&file_path, b"not really a png").unwrap();

        let tool = VisionTool::with_config("http://127.0.0.1:1/v1", "test-key", "qwen-vl-max");
        let mut ctx = test_ctx();
        ctx.working_dir = tmp.path().to_path_buf();

        let out = tool
            .execute(
                serde_json::json!({"image": file_path.to_str().unwrap()}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("vision request failed"));
    }
}
