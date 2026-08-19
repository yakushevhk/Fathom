//! `spawn_agent` tool: lets a running agent delegate a sub-task to a child
//! agent (sub-sub-agents included, up to `config.agent.max_depth`).
//!
//! The tool only validates arguments and packages a *spawn request* into the
//! tool output metadata; the actual child agent is created and executed by
//! [`pr_agent::AgentRuntime`], which intercepts `spawn_request` metadata,
//! enforces the depth limit (it knows the caller's depth) and injects the
//! child's capped summary back as the tool result.

use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SpawnAgentParams {
    /// Task description for the sub-agent
    task: String,
    /// Agent role: researcher, analyst, verifier, writer
    #[serde(default = "default_role")]
    role: String,
    /// Additional context handed to the sub-agent: facts, constraints or
    /// findings from the parent conversation it needs to do its job.
    /// The child agent starts fresh and sees ONLY its task plus this context.
    #[serde(default)]
    context: Vec<String>,
    /// Run in background: the tool returns immediately; the child's result
    /// is delivered to the parent as a notice once it finishes (fleet E2,
    /// OpenCode pattern). Use for long-running side tasks.
    #[serde(default)]
    background: bool,
}

fn default_role() -> String {
    "researcher".to_string()
}

/// Delegation tool. Stateless: depth limits are enforced by the agent
/// runtime, which knows the calling agent's depth.
pub struct SpawnAgentTool;

#[async_trait]
impl Tool for SpawnAgentTool {
    fn name(&self) -> &str {
        "spawn_agent"
    }
    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a specific research task. The sub-agent runs with its own context and tool access, and its final summary is returned as this tool's result.

## Capability

Creates a new agent of the specified role that works on the given task. The sub-agent has access to the same tools (web_search, web_fetch, extract_contacts, save_contacts, file tools, ...) and runs autonomously until it completes or hits its iteration limit. Sub-agents can spawn their own sub-agents while the configured depth limit allows it.

## When to Use

- Delegating a well-defined sub-task that can be worked on independently.
- Parallelizing research across multiple topics or aspects of a question.
- Assigning specialized work to a role best suited for it (e.g., verification, analysis).

## When NOT to Use

- Do NOT use `spawn_agent` for tasks you can complete yourself in a few tool calls.
- Do NOT use `spawn_agent` for tasks that depend on the results of other in-progress tasks.
- If you are at maximum depth, spawning is refused — do the work yourself.

## Context Handoff (important)

Sub-agents do NOT see your conversation. Put everything the child needs into `task` and `context`: entity names, URLs already found, constraints, expected output format.

## Available Roles

- `researcher` (default): searches the web, fetches pages, compiles findings with source citations.
- `analyst`: cross-references findings, identifies patterns and contradictions, assesses confidence.
- `verifier`: fact-checks claims, looks for contradicting evidence, assigns verification status.
- `writer`: produces well-structured markdown reports from findings.

## Parameters

- `task` (required): A clear, self-contained description of what the sub-agent should accomplish.
- `role` (optional, default \"researcher\"): One of researcher, analyst, verifier, writer.
- `context` (optional): array of strings with background facts the child needs.
- `background` (optional, default false): launch the child in the background and keep working; its result arrives later as a notice.

## Failure Modes

- Depth limit reached: the agent hierarchy is too deep; do the work yourself.
- Child failure: the child's error message is returned as the tool result."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(SpawnAgentParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: SpawnAgentParams = match serde_json::from_value(args) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutput::err(format!("Invalid arguments: {e}"))),
        };

        if params.task.trim().is_empty() {
            return Ok(ToolOutput::err("spawn_agent requires a non-empty task"));
        }

        let role = match params.role.to_lowercase().as_str() {
            "researcher" | "analyst" | "verifier" | "writer" => params.role.to_lowercase(),
            other => {
                return Ok(ToolOutput::err(format!(
                    "Unknown role '{other}'. Valid roles: researcher, analyst, verifier, writer"
                )))
            }
        };

        // The agent runtime intercepts this metadata marker, checks the depth
        // limit against the calling agent's depth, runs the child and replaces
        // this tool result with the child's (budget-capped) summary.
        Ok(ToolOutput::ok_with_meta(
            format!("Spawn request created for {role} agent"),
            serde_json::json!({
                "spawn_request": true,
                "task": params.task,
                "role": role,
                "context": params.context,
                "background": params.background,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::ToolContext;
    use pr_core::SearchConfig;
    use std::path::PathBuf;

    fn ctx() -> ToolContext {
        ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default())
    }

    #[tokio::test]
    async fn test_spawn_request_metadata() {
        let tool = SpawnAgentTool;
        let out = tool
            .execute(
                serde_json::json!({
                    "task": "Find CEO contacts",
                    "role": "Researcher",
                    "context": ["Company: Acme", "Site: acme.com"]
                }),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.success);
        let meta = out.metadata.unwrap();
        assert_eq!(meta["spawn_request"], true);
        assert_eq!(meta["task"], "Find CEO contacts");
        assert_eq!(meta["role"], "researcher");
        assert_eq!(meta["context"][0], "Company: Acme");
    }

    #[tokio::test]
    async fn test_spawn_rejects_empty_task_and_bad_role() {
        let tool = SpawnAgentTool;
        let out = tool
            .execute(serde_json::json!({"task": "  "}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);

        let out = tool
            .execute(serde_json::json!({"task": "x", "role": "wizard"}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("Unknown role"));
    }

    #[test]
    fn test_spawn_schema() {
        let schema = SpawnAgentTool.schema();
        assert_eq!(schema.name, "spawn_agent");
        let props = &schema.parameters["properties"];
        assert!(props.get("task").is_some());
        assert!(props.get("role").is_some());
        assert!(props.get("context").is_some());
    }
}
