use std::sync::Arc;
use async_trait::async_trait;
use futures::Stream;
use pr_core::{PrError, PrResult};
use crate::provider::LlmProvider;
use crate::types::{CompletionRequest, CompletionResponse, StreamChunk};

/// Cascade fallback provider that routes requests down a priority list
/// of providers on transient failures, rate limits, or outage errors.
pub struct CascadeProvider {
    providers: Vec<Arc<dyn LlmProvider>>,
}

impl CascadeProvider {
    pub fn new(providers: Vec<Arc<dyn LlmProvider>>) -> Self {
        Self { providers }
    }
}

#[async_trait]
impl LlmProvider for CascadeProvider {
    fn name(&self) -> &str {
        "cascade"
    }

    fn model(&self) -> &str {
        self.providers
            .first()
            .map(|p| p.model())
            .unwrap_or("unknown")
    }

    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> {
        if self.providers.is_empty() {
            return Err(PrError::Llm("CascadeProvider has no registered child providers".into()));
        }

        let mut last_err = None;

        for (idx, provider) in self.providers.iter().enumerate() {
            match provider.complete(req).await {
                Ok(resp) => {
                    if idx > 0 {
                        tracing::info!(
                            "CascadeProvider succeeded on fallback provider #{}: {} ({})",
                            idx + 1,
                            provider.name(),
                            provider.model()
                        );
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    tracing::warn!(
                        "CascadeProvider: provider #{}: {} ({}) failed: {}. Trying next fallback...",
                        idx + 1,
                        provider.name(),
                        provider.model(),
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| PrError::Llm("All cascade fallback providers failed".into())))
    }

    async fn stream(
        &self,
        req: &CompletionRequest,
    ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        if self.providers.is_empty() {
            return Err(PrError::Llm("CascadeProvider has no registered child providers".into()));
        }

        let mut last_err = None;

        for (idx, provider) in self.providers.iter().enumerate() {
            match provider.stream(req).await {
                Ok(stream) => {
                    if idx > 0 {
                        tracing::info!(
                            "CascadeProvider stream succeeded on fallback provider #{}: {} ({})",
                            idx + 1,
                            provider.name(),
                            provider.model()
                        );
                    }
                    return Ok(stream);
                }
                Err(e) => {
                    tracing::warn!(
                        "CascadeProvider: stream on provider #{}: {} failed: {}. Trying next...",
                        idx + 1,
                        provider.name(),
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| PrError::Llm("All cascade fallback providers failed streaming".into())))
    }
}
