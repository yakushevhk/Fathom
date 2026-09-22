use crate::registry::{Tool, ToolContext};
use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Subagent specification within a batch swarm task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SwarmSubtask {
    /// Distinct subagent task instruction
    pub task: String,
    /// Stable CamelCase or snake_case identifier (optional)
    #[serde(default)]
    pub name: Option<String>,
    /// Specific agent archetype (e.g. "scout", "coder", "reviewer", "verifier", "writer")
    #[serde(default = "default_agent_type")]
    pub agent: String,
    /// Invocation-specific JSON Schema for structured result extraction
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
    /// Permissive or strict schema validation mode (default: "permissive")
    #[serde(default = "default_schema_mode")]
    pub schema_mode: String,
}

fn default_agent_type() -> String {
    "scout".to_string()
}

fn default_schema_mode() -> String {
    "permissive".to_string()
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct TaskBatchParams {
    /// Shared background context, constraints, and contracts applying to all subagents
    pub context: String,
    /// Array of subagent tasks to dispatch concurrently into the swarm
    pub tasks: Vec<SwarmSubtask>,
}
/// Swarm batch task delegation tool implementing the batch task[] protocol.
pub struct TaskBatchTool;

#[async_trait]
impl Tool for TaskBatchTool {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Delegate work to a fleet of background subagents by passing multiple items in a single `tasks[]` batch.

## Contract
- Parallelize independent slices across specific agent archetypes:
  - `scout`: Fast read-only exploratory analysis and research.
  - `coder`: Direct code modification and surgical implementation.
  - `reviewer`: Code review and security/quality analysis.
  - `verifier`: Test execution and contract verification.
- `context`: Shared goals, constraints, and contracts passed to all subagents.
- `tasks[]`: Array of self-contained tasks with optional output schemas.
Returns batch job handles and coordination IDs immediately."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(TaskBatchParams).schema)
                .unwrap_or_default(),
        }
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let params: TaskBatchParams = serde_json::from_value(args)?;

        if params.tasks.is_empty() {
            return Ok(ToolOutput::err("No tasks provided in tasks[] array"));
        }

        let mut job_entries = Vec::new();
        let mut spawn_metadata = Vec::new();

        for (i, item) in params.tasks.iter().enumerate() {
            let subagent_name = item
                .name
                .clone()
                .unwrap_or_else(|| format!("subagent_{}_{}", item.agent, i + 1));
            let job_id = uuid::Uuid::now_v7().to_string();

            job_entries.push(format!(
                "- `{}` [role: {}, job_id: {}]: {}",
                subagent_name, item.agent, job_id, item.task
            ));

            spawn_metadata.push(serde_json::json!({
                "job_id": job_id,
                "name": subagent_name,
                "role": item.agent,
                "task": item.task,
                "context": params.context,
                "output_schema": item.output_schema,
                "schema_mode": item.schema_mode
            }));
        }

        let response_msg = format!(
            "Spawned {} background subagents in swarm:\n{}\nUse `hub` (op: 'jobs' / 'wait') to monitor status or coordinate via IRC.",
            params.tasks.len(),
            job_entries.join("\n")
        );

        let mut output = ToolOutput::ok(response_msg);
        output.metadata = Some(serde_json::json!({
            "swarm_batch_spawn": spawn_metadata
        }));

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::SearchConfig;
    use std::path::PathBuf;

    fn ctx() -> ToolContext {
        ToolContext::new(PathBuf::from("/tmp"), SearchConfig::default())
    }

    #[test]
    fn swarm_subtask_defaults() {
        let t: SwarmSubtask = serde_json::from_str(r#"{"task":"do x"}"#).unwrap();
        assert_eq!(t.agent, "scout");
        assert_eq!(t.schema_mode, "permissive");
        assert!(t.name.is_none());
        assert!(t.output_schema.is_none());
    }

    #[test]
    fn swarm_subtask_explicit_fields() {
        let t: SwarmSubtask = serde_json::from_str(
            r#"{"task":"t","name":"N","agent":"coder","schema_mode":"strict","output_schema":{"type":"object"}}"#,
        )
        .unwrap();
        assert_eq!(t.agent, "coder");
        assert_eq!(t.name.as_deref(), Some("N"));
        assert_eq!(t.schema_mode, "strict");
    }

    #[tokio::test]
    async fn empty_tasks_array_is_error_output() {
        let tool = TaskBatchTool;
        let out = tool
            .execute(serde_json::json!({"context": "c", "tasks": []}), &ctx())
            .await
            .unwrap();
        assert!(!out.success);
    }

    #[tokio::test]
    async fn batch_spawn_returns_job_handles() {
        let tool = TaskBatchTool;
        let out = tool
            .execute(
                serde_json::json!({
                    "context": "shared ctx",
                    "tasks": [
                        {"task": "scan repo", "agent": "scout", "name": "s1"},
                        {"task": "fix bug", "agent": "coder"}
                    ]
                }),
                &ctx(),
            )
            .await
            .unwrap();
        assert!(out.success);
        assert!(out.content.contains("Spawned 2"));
        assert!(out.content.contains("s1"));
        assert!(out.content.contains("subagent_coder_2"));
        let meta = out.metadata.unwrap();
        let spawns = meta["swarm_batch_spawn"].as_array().unwrap();
        assert_eq!(spawns.len(), 2);
        assert_eq!(spawns[0]["name"], "s1");
        assert_eq!(spawns[1]["role"], "coder");
        assert_eq!(spawns[0]["context"], "shared ctx");
        assert!(!spawns[0]["job_id"].as_str().unwrap().is_empty());
    }

    #[test]
    fn tool_name_and_schema() {
        let tool = TaskBatchTool;
        assert_eq!(tool.name(), "task");
        let schema = tool.schema();
        assert_eq!(schema.name, "task");
        assert!(schema.parameters.is_object());
    }
}
