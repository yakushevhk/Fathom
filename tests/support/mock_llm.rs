//! Deterministic offline mock of the LLM provider used by the E2E and
//! integration test suites. It emulates the three conversation roles the
//! coordinator drives: planner, researcher and synthesizer.

use async_trait::async_trait;
use futures::Stream;
use pr_core::{Message, PrResult};
use pr_llm::{CompletionRequest, CompletionResponse, LlmProvider, StreamChunk, Usage};

/// Scripted LLM provider.
///
/// Responses are selected by inspecting the system prompt of the request:
/// - "research planner"     -> `plan_response`
/// - "research synthesizer" -> `synthesis_response`
/// - anything else          -> `answer_response` (researcher agents)
///
/// Responses never contain tool calls, so researcher agents complete in a
/// single iteration without touching the network.
pub struct MockLlm {
    pub plan_response: String,
    pub synthesis_response: String,
    pub answer_response: String,
}

impl MockLlm {
    /// Planner cannot decompose -> coordinator falls back to a single agent.
    #[allow(dead_code)]
    pub fn single_agent() -> Self {
        Self {
            plan_response: "I cannot decompose this query into sub-tasks.".to_string(),
            synthesis_response: "# Research Report\n\nSingle-agent synthesis of the findings."
                .to_string(),
            answer_response:
                "Task completed. Consulted source: https://example.com/primary-source"
                    .to_string(),
        }
    }

    /// Planner decomposes into `tasks` parallel sub-tasks.
    #[allow(dead_code)]
    pub fn multi_agent(tasks: usize) -> Self {
        let task_list: Vec<String> = (1..=tasks)
            .map(|i| format!("Research sub-topic #{i} in depth"))
            .collect();
        Self {
            plan_response: serde_json::to_string(&task_list).unwrap(),
            synthesis_response: format!(
                "# Research Report\n\nSynthesis integrating {tasks} parallel sub-tasks."
            ),
            answer_response:
                "Sub-task completed. Consulted source: https://example.org/reference"
                    .to_string(),
        }
    }
}

#[async_trait]
impl LlmProvider for MockLlm {
    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }

    async fn complete(&self, req: &CompletionRequest) -> PrResult<CompletionResponse> {
        let system = req
            .messages
            .iter()
            .find_map(|m| match m {
                Message::System { content } => Some(content.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let text = if system.contains("research planner") {
            self.plan_response.clone()
        } else if system.contains("research synthesizer") {
            self.synthesis_response.clone()
        } else {
            self.answer_response.clone()
        };

        Ok(CompletionResponse {
            message: Message::assistant(text),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn stream(
        &self,
        _req: &CompletionRequest,
    ) -> PrResult<Box<dyn Stream<Item = PrResult<StreamChunk>> + Send + Unpin>> {
        // Tests run with stream = false; provide an empty stream for completeness.
        Ok(Box::new(futures::stream::empty()))
    }
}
