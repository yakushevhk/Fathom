use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

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
            parameters: serde_json::to_value(&schemars::schema_for!(TaskBatchParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: TaskBatchParams = serde_json::from_value(args)?;

        if params.tasks.is_empty() {
            return Ok(ToolOutput::err("No tasks provided in tasks[] array"));
        }

        let mut job_entries = Vec::new();
        let mut spawn_metadata = Vec::new();

        for (i, item) in params.tasks.iter().enumerate() {
            let subagent_name = item.name.clone().unwrap_or_else(|| format!("subagent_{}_{}", item.agent, i + 1));
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
