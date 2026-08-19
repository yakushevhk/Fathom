//! LLM provider factory.
//!
//! Builds the configured [`LlmProvider`] from [`pr_core::LlmConfig`].
//! All providers currently speak the OpenAI-compatible chat-completions
//! protocol, so any endpoint implementing it works (DeepSeek, OpenAI,
//! OpenRouter, vLLM, Ollama, LM Studio, ...) by pointing `base_url` at it.

use std::sync::Arc;

use pr_core::LlmConfig;

use crate::deepseek::DeepSeekProvider;
use crate::provider::LlmProvider;

/// Providers known to use the OpenAI-compatible protocol. Unknown provider
/// names are accepted too (they are assumed OpenAI-compatible) but traced,
/// so genuinely new protocols can be added here explicitly.
const OPENAI_COMPATIBLE: &[&str] = &[
    "deepseek",
    "openai",
    "openrouter",
    "ollama",
    "vllm",
    "lmstudio",
    "openai-compatible",
];

/// Build an LLM provider from configuration.
///
/// Errors when no API key is configured (the caller decides whether that is
/// fatal — e.g. `serve` can start without a key and refuse session creation).
pub fn build_provider(cfg: &LlmConfig) -> anyhow::Result<Arc<dyn LlmProvider>> {
    if cfg.api_key.trim().is_empty() {
        anyhow::bail!(
            "No LLM api_key configured. Set it in ~/.parallel-research/config.toml:\n\
            [llm]\napi_key = \"your-key\""
        );
    }
    if cfg.base_url.trim().is_empty() {
        anyhow::bail!("No LLM base_url configured");
    }

    if !OPENAI_COMPATIBLE.contains(&cfg.provider.to_lowercase().as_str()) {
        tracing::warn!(
            provider = %cfg.provider,
            "unknown LLM provider name; assuming OpenAI-compatible protocol"
        );
    }

    Ok(Arc::new(
        DeepSeekProvider::new(&cfg.base_url, &cfg.api_key, &cfg.model)
            .with_provider_name(cfg.provider.clone()),
    ))
}

/// Build the optional cheap/fast provider (`[llm] fast_model`), used for
/// high-volume auxiliary calls (entity extraction, memory classification,
/// search reranking). Same endpoint and credentials as the main provider;
/// returns `None` when `fast_model` is unset, letting callers fall back to
/// the primary model.
pub fn build_fast_provider(cfg: &LlmConfig) -> anyhow::Result<Option<Arc<dyn LlmProvider>>> {
    let model = cfg.fast_model.trim();
    if model.is_empty() || model == cfg.model.trim() {
        return Ok(None);
    }
    let mut fast = cfg.clone();
    fast.model = model.to_string();
    Ok(Some(build_provider(&fast)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(provider: &str, key: &str) -> LlmConfig {
        LlmConfig {
            provider: provider.to_string(),
            base_url: "https://api.example.com".to_string(),
            api_key: key.to_string(),
            model: "test-model".to_string(),
            fast_model: String::new(),
            max_tokens: 100,
            temperature: 0.5,
        }
    }

    #[test]
    fn test_build_provider_reports_configured_name() {
        let p = build_provider(&cfg("openrouter", "sk-x")).unwrap();
        assert_eq!(p.name(), "openrouter");
        assert_eq!(p.model(), "test-model");
    }

    #[test]
    fn test_build_provider_deepseek_default() {
        let p = build_provider(&cfg("deepseek", "sk-x")).unwrap();
        assert_eq!(p.name(), "deepseek");
    }

    #[test]
    fn test_build_provider_unknown_name_accepted() {
        let p = build_provider(&cfg("my-local-llm", "sk-x")).unwrap();
        assert_eq!(p.name(), "my-local-llm");
    }

    #[test]
    fn test_build_provider_requires_api_key() {
        assert!(build_provider(&cfg("deepseek", "")).is_err());
        assert!(build_provider(&cfg("deepseek", "   ")).is_err());
    }

    #[test]
    fn test_build_provider_requires_base_url() {
        let mut c = cfg("deepseek", "sk-x");
        c.base_url = String::new();
        assert!(build_provider(&c).is_err());
    }

    #[test]
    fn test_build_fast_provider_unset_is_none() {
        assert!(build_fast_provider(&cfg("deepseek", "sk-x")).unwrap().is_none());
    }

    #[test]
    fn test_build_fast_provider_same_model_is_none() {
        let mut c = cfg("deepseek", "sk-x");
        c.fast_model = "test-model".to_string();
        assert!(build_fast_provider(&c).unwrap().is_none());
    }

    #[test]
    fn test_build_fast_provider_uses_fast_model_id() {
        let mut c = cfg("deepseek", "sk-x");
        c.fast_model = "cheap-model".to_string();
        let p = build_fast_provider(&c).unwrap().expect("fast provider");
        assert_eq!(p.model(), "cheap-model");
        assert_eq!(p.name(), "deepseek");
    }
}
