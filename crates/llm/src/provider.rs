use async_trait::async_trait;
use pr_core::PrResult;
use crate::types::{CompletionRequest, CompletionResponse, StreamChunk};
use futures::Stream;

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
