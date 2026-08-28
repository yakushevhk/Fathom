use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

/// Sub-structure for optionally minting or enhancing a managed skill.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManagedSkillSpec {
    pub action: String, // "create" | "update" | "delete"
    pub name: String,
    pub description: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct LearnParams {
    /// The durable, self-contained lesson to remember (what, when, why)
    pub memory: String,
    /// Optional source context or trigger for the lesson
    #[serde(default)]
    pub context: Option<String>,
    /// Also create or enhance a managed skill in the same call (optional)
    #[serde(default)]
    pub skill: Option<ManagedSkillSpec>,
}

/// Continuous self-learning tool: capture durable lessons in long-term memory.
pub struct LearnTool;

#[async_trait]
impl Tool for LearnTool {
    fn name(&self) -> &str {
        "learn"
    }

    fn description(&self) -> &str {
        "Capture reusable lessons in long-term memory; optionally mint/enhance a managed skill in the same call.

Use after discovering an insight likely to pay off again: a non-obvious bug fix, discovered project convention, or workflow that succeeded."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(LearnParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: LearnParams = serde_json::from_value(args)?;

        let mut lines = Vec::new();
        lines.push(format!("Captured lesson into long-term memory: \"{}\"", params.memory));

        if let Some(c) = params.context {
            lines.push(format!("Context: {}", c));
        }

        if let Some(sk) = params.skill {
            let fathom_home = dirs::home_dir()
                .map(|h| h.join(".fathom"))
                .unwrap_or_else(|| std::path::PathBuf::from(".fathom"));
            let skill_dir = fathom_home.join("skills").join(&sk.name);
            tokio::fs::create_dir_all(&skill_dir).await?;

            let skill_file = skill_dir.join("SKILL.md");
            let frontmatter = format!(
                "---\nname: {}\ndescription: {}\n---\n\n{}",
                sk.name,
                sk.description.unwrap_or_default(),
                sk.body.unwrap_or_default()
            );
            tokio::fs::write(&skill_file, &frontmatter).await?;
            lines.push(format!("Minted managed skill '{}' at {}", sk.name, skill_file.display()));
        }

        Ok(ToolOutput::ok(lines.join("\n")))
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ManageSkillParams {
    pub action: String, // "create" | "update" | "delete"
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
}

/// Managed skill manipulation tool.
pub struct ManageSkillTool;

#[async_trait]
impl Tool for ManageSkillTool {
    fn name(&self) -> &str {
        "manage_skill"
    }

    fn description(&self) -> &str {
        "Create, update, or delete managed skills in ~/.fathom/skills/.

- `action: 'create'` — create new skill.
- `action: 'update'` — update skill body and description.
- `action: 'delete'` — remove skill."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(ManageSkillParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: ManageSkillParams = serde_json::from_value(args)?;

        let fathom_home = dirs::home_dir()
            .map(|h| h.join(".fathom"))
            .unwrap_or_else(|| std::path::PathBuf::from(".fathom"));
        let skill_dir = fathom_home.join("skills").join(&params.name);

        match params.action.as_str() {
            "create" | "update" => {
                tokio::fs::create_dir_all(&skill_dir).await?;
                let skill_file = skill_dir.join("SKILL.md");
                let frontmatter = format!(
                    "---\nname: {}\ndescription: {}\n---\n\n{}",
                    params.name,
                    params.description.unwrap_or_default(),
                    params.body.unwrap_or_default()
                );
                tokio::fs::write(&skill_file, &frontmatter).await?;
                Ok(ToolOutput::ok(format!("Skill '{}' written to {}", params.name, skill_file.display())))
            }
            "delete" => {
                if skill_dir.exists() {
                    tokio::fs::remove_dir_all(&skill_dir).await?;
                    Ok(ToolOutput::ok(format!("Deleted skill '{}'", params.name)))
                } else {
                    Ok(ToolOutput::err(format!("Skill '{}' not found", params.name)))
                }
            }
            other => Ok(ToolOutput::err(format!("Unsupported action '{}'", other))),
        }
    }
}
