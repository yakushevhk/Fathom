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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Usage;
    use futures::StreamExt;
    use pr_core::PrError;
    use std::pin::Pin;

    /// Minimal mock provider to exercise the trait contract.
    struct MockProvider {
        name: String,
        model: String,
        complete_result: PrResult<CompletionResponse>,
        stream_chunks: Vec<PrResult<StreamChunk>>,
    }

    impl MockProvider {
        fn ok(name: &str, model: &str) -> Self {
            Self {
                name: name.to_string(),
                model: model.to_string(),
                complete_result: Ok(CompletionResponse {
                    message: pr_core::Message::assistant("mock reply"),
                    usage: Some(Usage {
                        prompt_tokens: 1,
                        completion_tokens: 1,
                        total_tokens: 2,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: None,
                    }),
                    finish_reason: Some("stop".to_string()),
                }),
                stream_chunks: vec![],
            }
        }

        fn failing(name: &str, model: &str) -> Self {
            Self {
                name: name.to_string(),
                model: model.to_string(),
                complete_result: Err(PrError::Llm("mock failure".to_string())),
                stream_chunks: vec![],
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            &self.name
        }

        fn model(&self) -> &str {
            &self.model
        }

        async fn complete(&self, _req: &CompletionRequest) -> PrResult<CompletionResponse> {
            match &self.complete_result {
                Ok(resp) => Ok(CompletionResponse {
                    message: resp.message.clone(),
                    usage: resp.usage.clone(),
                    finish_reason: resp.finish_reason.clone(),
                }),
                Err(e) => Err(match e {
                    PrError::Llm(m) => PrError::Llm(m.clone()),
                    PrError::Tool(m) => PrError::Tool(m.clone()),
                    PrError::Agent(m) => PrError::Agent(m.clone()),
                    PrError::Persistence(m) => PrError::Persistence(m.clone()),
                    PrError::Config(m) => PrError::Config(m.clone()),
                    PrError::Timeout(s) => PrError::Timeout(*s),
                    PrError::MaxDepthReached(n) => PrError::MaxDepthReached(*n),
                    PrError::MaxAgentsReached(n) => PrError::MaxAgentsReached(*n),
                    PrError::MaxIterationsReached(n) => PrError::MaxIterationsReached(*n),
                    PrError::Cancelled => PrError::Cancelled,
                    PrError::Http { status, message, retry_after } => PrError::Http {
                        status: *status,
                        message: message.clone(),
                        retry_after: *retry_after,
                    },
                    PrError::ResponseTooLarge(m) => PrError::ResponseTooLarge(m.clone()),
                }),
            }
        }

        async fn stream(
            &self,
            _req: &CompletionRequest,
        ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
            let chunks: Vec<PrResult<StreamChunk>> = self
                .stream_chunks
                .iter()
                .map(|c| match c {
                    Ok(StreamChunk::Text { delta }) => Ok(StreamChunk::Text { delta: delta.clone() }),
                    Ok(StreamChunk::Reasoning { delta }) => Ok(StreamChunk::Reasoning { delta: delta.clone() }),
                    Ok(StreamChunk::ToolCallDelta { index, id, name, arguments_delta }) => {
                        Ok(StreamChunk::ToolCallDelta {
                            index: *index,
                            id: id.clone(),
                            name: name.clone(),
                            arguments_delta: arguments_delta.clone(),
                        })
                    }
                    Ok(StreamChunk::Done { message, usage, finish_reason }) => {
                        Ok(StreamChunk::Done {
                            message: message.clone(),
                            usage: usage.clone(),
                            finish_reason: finish_reason.clone(),
                        })
                    }
                    Ok(StreamChunk::Error { message }) => Ok(StreamChunk::Error { message: message.clone() }),
Err(e) => Err(match e {
                    PrError::Llm(m) => PrError::Llm(m.clone()),
                    PrError::Tool(m) => PrError::Tool(m.clone()),
                    PrError::Agent(m) => PrError::Agent(m.clone()),
                    PrError::Persistence(m) => PrError::Persistence(m.clone()),
                    PrError::Config(m) => PrError::Config(m.clone()),
                    PrError::Timeout(s) => PrError::Timeout(*s),
                    PrError::MaxDepthReached(n) => PrError::MaxDepthReached(*n),
                    PrError::MaxAgentsReached(n) => PrError::MaxAgentsReached(*n),
                    PrError::MaxIterationsReached(n) => PrError::MaxIterationsReached(*n),
                    PrError::Cancelled => PrError::Cancelled,
                    PrError::Http { status, message, retry_after } => PrError::Http {
                        status: *status,
                        message: message.clone(),
                        retry_after: *retry_after,
                    },
                    PrError::ResponseTooLarge(m) => PrError::ResponseTooLarge(m.clone()),
                }),
                })
                .collect();
            Ok(Box::new(futures::stream::iter(chunks)))
        }
    }

    #[test]
    fn provider_name_and_model() {
        let p = MockProvider::ok("mock", "mock-model-1");
        assert_eq!(p.name(), "mock");
        assert_eq!(p.model(), "mock-model-1");
    }

    #[test]
    fn provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockProvider>();
    }

    fn sample_request() -> CompletionRequest {
        CompletionRequest {
            messages: vec![pr_core::Message::user("ping")],
            tools: vec![],
            temperature: Some(0.0),
            max_tokens: Some(10),
            stream: false,
        }
    }

    #[tokio::test]
    async fn provider_complete_ok() {
        let p = MockProvider::ok("mock", "m-1");
        let resp = p.complete(&sample_request()).await.expect("should succeed");
        assert_eq!(resp.finish_reason.as_deref(), Some("stop"));
        match resp.message {
            pr_core::Message::Assistant { content, .. } => {
                assert_eq!(content.as_deref(), Some("mock reply"));
            }
            _ => panic!("expected assistant message"),
        }
        assert_eq!(resp.usage.as_ref().unwrap().total_tokens, 2);
    }

    #[tokio::test]
    async fn provider_complete_error() {
        let p = MockProvider::failing("mock", "m-2");
        let err = p.complete(&sample_request()).await.expect_err("should fail");
        match err {
            PrError::Llm(m) => assert_eq!(m, "mock failure"),
            other => panic!("expected Llm error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn provider_stream_text_chunks() {
        let mut p = MockProvider::ok("mock", "m-3");
        p.stream_chunks = vec![
            Ok(StreamChunk::Text { delta: "Hel".into() }),
            Ok(StreamChunk::Text { delta: "lo".into() }),
        ];
        let stream = p.stream(&sample_request()).await.expect("stream ok");
        let mut stream = Box::pin(stream);
        let mut collected = Vec::new();
        while let Some(chunk) = stream.next().await {
            collected.push(chunk);
        }
        assert_eq!(collected.len(), 2);
        match &collected[0] {
            Ok(StreamChunk::Text { delta }) => assert_eq!(delta, "Hel"),
            other => panic!("expected text chunk, got {:?}", other),
        }
        match &collected[1] {
            Ok(StreamChunk::Text { delta }) => assert_eq!(delta, "lo"),
            other => panic!("expected text chunk, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn provider_stream_error_chunk() {
        let mut p = MockProvider::failing("mock", "m-4");
        p.stream_chunks = vec![Err(PrError::Llm("stream error".into()))];
        let stream = p.stream(&sample_request()).await.expect("stream ok");
        let mut stream = Box::pin(stream);
        let chunk = stream.next().await.expect("one chunk");
        match chunk {
            Err(PrError::Llm(m)) => assert_eq!(m, "stream error"),
            other => panic!("expected error chunk, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn provider_stream_done_chunk() {
        let mut p = MockProvider::ok("mock", "m-5");
        p.stream_chunks = vec![Ok(StreamChunk::Done {
            message: pr_core::Message::assistant("done"),
            usage: None,
            finish_reason: Some("stop".into()),
        })];
        let stream = p.stream(&sample_request()).await.expect("stream ok");
        let mut stream = Box::pin(stream);
        let chunk = stream.next().await.expect("one chunk");
        match chunk {
            Ok(StreamChunk::Done { finish_reason, .. }) => {
                assert_eq!(finish_reason.as_deref(), Some("stop"));
            }
            other => panic!("expected done chunk, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn provider_stream_empty() {
        let p = MockProvider::ok("mock", "m-6");
        let stream = p.stream(&sample_request()).await.expect("stream ok");
        let mut stream = Box::pin(stream);
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn provider_dyn_dispatch() {
        let p: Box<dyn LlmProvider> = Box::new(MockProvider::ok("dyn", "d-1"));
        assert_eq!(p.name(), "dyn");
        assert_eq!(p.model(), "d-1");
        let resp = p.complete(&sample_request()).await.expect("succeeds");
        assert_eq!(resp.usage.as_ref().unwrap().prompt_tokens, 1);
    }

    /// The stream() return type must be Send (the dyn type has Send bound).
    /// This exercises the exact signature used by real implementations.
    #[tokio::test]
    async fn provider_stream_is_send() {
        use std::future::Future;
        let p = MockProvider::ok("mock", "m-7");
        let stream = p.stream(&sample_request()).await.expect("ok");
        let _pin: Pin<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> = Box::pin(stream);
        // Compile-time assertion that the future chain is Send
        fn assert_future_send<T: Future + Send>(_: T) {}
        assert_future_send(p.complete(&sample_request()));
    }
}
