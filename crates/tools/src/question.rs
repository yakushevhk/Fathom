//! The `question` tool — ask the human operator mid-run (control plane).
//!
//! Like `spawn_agent`, the tool only validates and packages the request;
//! the agent runtime performs the actual operator round-trip and returns
//! the answer as the tool result. Headless runs without an operator get an
//! "proceed on your own" notice instead of blocking.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

pub struct QuestionTool;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct QuestionParams {
    /// The question for the operator. Must be specific and answerable in
    /// one short message.
    question: String,
}

#[async_trait]
impl Tool for QuestionTool {
    fn name(&self) -> &str {
        "question"
    }

    fn description(&self) -> &str {
        "Ask the human operator a question and wait for the answer. Use ONLY when you are genuinely blocked and cannot proceed reasonably on your own — ambiguous goals, missing credentials/access, a choice between materially different directions.

## Rules

- One concise, specific question per call.
- Do NOT use for things you can find out with your tools.
- Do NOT use more than 2-3 times per session.
- If no operator is connected (headless run), you will be told to proceed on your own.

## Parameters

- `question` (required): the question text."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(QuestionParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: QuestionParams = serde_json::from_value(args)?;
        let question = params.question.trim();
        if question.is_empty() {
            return Ok(ToolOutput::err("question must not be empty"));
        }
        if question.chars().count() > 500 {
            return Ok(ToolOutput::err(
                "question too long (max 500 chars) — make it concise",
            ));
        }
        // Marker for the runtime: it performs the operator round-trip and
        // replaces this output with the actual answer.
        Ok(ToolOutput::ok_with_meta(
            "question registered".to_string(),
            serde_json::json!({
                "question_request": true,
                "question": question,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;
    use pr_core::SearchConfig;

    fn ctx() -> ToolContext {
        ToolContext::new(std::path::PathBuf::from("/tmp"), SearchConfig::default())
    }

    #[tokio::test]
    async fn question_packages_marker() {
        let tool = QuestionTool;
        let out = tool
            .execute(serde_json::json!({"question": "Which region should I focus on?"}), &ctx())
            .await
            .unwrap();
        assert!(out.success);
        assert_eq!(out.metadata.as_ref().unwrap()["question_request"], true);
        assert_eq!(
            out.metadata.as_ref().unwrap()["question"],
            "Which region should I focus on?"
        );
    }

    #[tokio::test]
    async fn question_rejects_empty() {
        let tool = QuestionTool;
        let out = tool
            .execute(serde_json::json!({"question": "   "}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);
    }

    #[tokio::test]
    async fn question_rejects_too_long() {
        let tool = QuestionTool;
        let long = "x".repeat(501);
        let out = tool
            .execute(serde_json::json!({"question": long}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("too long"));
    }

    #[test]
    fn schema_is_valid() {
        let tool = QuestionTool;
        let schema = tool.schema();
        assert_eq!(schema.name, "question");
        assert!(schema.parameters.is_object());
    }
}
