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
    /// Task description for the sub-agent (single spawn). Mutually exclusive
    /// with `tasks` (batch spawn).
    #[serde(default)]
    task: Option<String>,
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

    // ── Batch spawn fields ──────────────────────────────────────────────

    /// Batch of sub-tasks for parallel execution (mutually exclusive with
    /// `task`). Each task runs as a separate agent concurrently.
    #[serde(default)]
    tasks: Vec<BatchTask>,
    /// Optional JSON schema for the output (as a JSON object). When set,
    /// the sub-agent is instructed to produce output matching this schema.
    #[serde(default)]
    output_schema: Option<serde_json::Value>,
    /// If true, each sub-agent runs in isolated mode (no access to the
    /// parent's scratchpad, findings, or memory). Default is false.
    #[serde(default)]
    isolated: bool,
}

/// A single task in a batch spawn.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct BatchTask {
    /// Task description for this sub-agent.
    task: String,
    /// Agent role override (optional, defaults to the parent's `role`).
    #[serde(default)]
    role: Option<String>,
    /// Per-task context override (optional, defaults to the parent's
    /// context if any).
    #[serde(default)]
    context: Vec<String>,
    /// Run this task in the background.
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

        // ── Batch spawn (parallel fan-out) ──────────────────────────────
        if !params.tasks.is_empty() {
            if params.task.is_some() {
                return Ok(ToolOutput::err(
                    "spawn_agent: use either `task` (single) or `tasks` (batch), not both",
                ));
            }
            if params.tasks.len() > 8 {
                return Ok(ToolOutput::err(
                    "spawn_agent: batch size limited to 8 tasks per call",
                ));
            }

            let mut batch: Vec<serde_json::Value> = Vec::with_capacity(params.tasks.len());
            // Output schema instruction appended to each task.
            let schema_hint = params
                .output_schema
                .as_ref()
                .map(|s| {
                    format!(
                        "\n\nRespond with JSON matching this schema:\n{}",
                        serde_json::to_string_pretty(s).unwrap_or_default()
                    )
                })
                .unwrap_or_default();

            for t in &params.tasks {
                let role = t
                    .role
                    .clone()
                    .unwrap_or_else(|| params.role.clone())
                    .to_lowercase();
                let context = if t.context.is_empty() {
                    params.context.clone()
                } else {
                    t.context.clone()
                };
                let full_task = if schema_hint.is_empty() {
                    t.task.clone()
                } else {
                    format!("{}{}", t.task, schema_hint)
                };
                batch.push(serde_json::json!({
                    "task": full_task,
                    "role": role,
                    "context": context,
                    "background": t.background || params.background,
                }));
            }

            return Ok(ToolOutput::ok_with_meta(
                format!("Batch spawn created for {} task(s)", batch.len()),
                serde_json::json!({
                    "spawn_request": true,
                    "spawn_batch": batch,
                    "output_schema": params.output_schema,
                    "isolated": params.isolated,
                }),
            ));
        }

        // ── Single spawn ────────────────────────────────────────────────
        let Some(task) = params.task else {
            return Ok(ToolOutput::err(
                "spawn_agent requires `task` (single) or `tasks` (batch)",
            ));
        };

        if task.trim().is_empty() {
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

        // Optional output-schema instruction.
        let full_task = if let Some(schema) = &params.output_schema {
            format!(
                "{}\n\nRespond with JSON matching this schema:\n{}",
                task,
                serde_json::to_string_pretty(schema).unwrap_or_default()
            )
        } else {
            task
        };

        // The agent runtime intercepts this metadata marker, checks the depth
        // limit against the calling agent's depth, runs the child and replaces
        // this tool result with the child's (budget-capped) summary.
        Ok(ToolOutput::ok_with_meta(
            format!("Spawn request created for {role} agent"),
            serde_json::json!({
                "spawn_request": true,
                "task": full_task,
                "role": role,
                "context": params.context,
                "background": params.background,
                "output_schema": params.output_schema,
                "isolated": params.isolated,
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

    #[tokio::test]
    async fn test_batch_spawn_metadata() {
        let tool = SpawnAgentTool;
        let out = tool
            .execute(
                serde_json::json!({
                    "tasks": [
                        {"task": "Task A", "role": "researcher"},
                        {"task": "Task B", "role": "analyst"}
                    ],
                    "output_schema": {"type": "object", "properties": {"x": {"type": "string"}}}
                }),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.success);
        let meta = out.metadata.unwrap();
        assert_eq!(meta["spawn_request"], true);
        assert_eq!(meta["spawn_batch"].as_array().unwrap().len(), 2);
        assert!(meta["spawn_batch"][0]["task"].as_str().unwrap().contains("Task A"));
        assert!(meta["spawn_batch"][1]["task"].as_str().unwrap().contains("Task B"));
        assert_eq!(meta["spawn_batch"][1]["role"], "analyst");
        // output_schema is threaded through
        assert!(meta["output_schema"].is_object());
    }

    #[tokio::test]
    async fn test_batch_spawn_validates_both_or_neither() {
        let tool = SpawnAgentTool;
        // Both task AND tasks -> error
        let out = tool
            .execute(
                serde_json::json!({"task": "single", "tasks": [{"task": "A"}]}),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("not both"));

        // tasks over cap (9) -> error
        let many: Vec<serde_json::Value> = (0..9)
            .map(|i| serde_json::json!({"task": format!("T{i}")}))
            .collect();
        let out = tool
            .execute(serde_json::json!({"tasks": many}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);
        assert!(out.content.contains("8"));
    }

    #[tokio::test]
    async fn test_single_spawn_output_schema() {
        let tool = SpawnAgentTool;
        let out = tool
            .execute(
                serde_json::json!({
                    "task": "Extract JSON",
                    "output_schema": {"type": "object"}
                }),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.success);
        let meta = out.metadata.unwrap();
        assert!(meta["task"].as_str().unwrap().contains("Respond with JSON matching this schema"));
        assert!(meta["isolated"] == false);
    }

    #[test]
    fn test_spawn_schema() {
        let schema = SpawnAgentTool.schema();
        assert_eq!(schema.name, "spawn_agent");
        let props = &schema.parameters["properties"];
        assert!(props.get("task").is_some());
        assert!(props.get("role").is_some());
        assert!(props.get("context").is_some());
        assert!(props.get("tasks").is_some());
        assert!(props.get("output_schema").is_some());
    }
}
