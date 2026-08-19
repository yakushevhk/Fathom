//! SKILL.md-based skill system.
//!
//! Skills are discovered by scanning `~/.parallel-research/skills/` for
//! directories containing a `SKILL.md` file. Each skill has a name,
//! description, and full content that can be injected into the system prompt.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};

// ── Skill ────────────────────────────────────────────────────────────────────

/// A discovered or created skill.
#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub file_path: PathBuf,
    pub created_at: DateTime<Utc>,
}

impl Skill {
    /// Parse a SKILL.md file into a Skill.
    ///
    /// Expected format:
    /// ```markdown
    /// # Skill Name
    ///
    /// Description line.
    ///
    /// ## Instructions
    /// ...
    /// ```
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading skill file {}", path.display()))?;

        let (name, description) = parse_skill_header(&content);

        // Use directory name as fallback for name.
        let name = if name.is_empty() {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unnamed".to_string())
        } else {
            name
        };

        Ok(Self {
            name,
            description,
            content,
            file_path: path.to_path_buf(),
            created_at: Utc::now(),
        })
    }
}

/// Parse the first `# Name` and the first non-empty, non-heading line after it
/// as the description.
fn parse_skill_header(content: &str) -> (String, String) {
    let mut name = String::new();
    let mut description = String::new();
    let mut found_heading = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if !found_heading {
            if let Some(heading) = trimmed.strip_prefix("# ") {
                name = heading.trim().to_string();
                found_heading = true;
            }
        } else if description.is_empty() {
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            description = trimmed.to_string();
            break;
        }
    }

    (name, description)
}

// ── SkillRegistry ────────────────────────────────────────────────────────────

/// Registry that discovers and manages skills from `~/.parallel-research/skills/`.
pub struct SkillRegistry {
    skills_dir: PathBuf,
    skills: Vec<Skill>,
}

impl SkillRegistry {
    /// Create a new registry rooted at `~/.parallel-research/skills/`.
    pub fn new(home_dir: &Path) -> Self {
        let skills_dir = home_dir.join(".parallel-research").join("skills");
        Self {
            skills_dir,
            skills: Vec::new(),
        }
    }

    /// Create with a custom skills directory (for testing).
    pub fn with_dir(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            skills: Vec::new(),
        }
    }

    /// Get the skills directory path.
    pub fn skills_dir(&self) -> &Path {
        &self.skills_dir
    }

    /// Scan the skills directory for SKILL.md files and load them.
    pub fn discover(&mut self) -> Result<()> {
        self.skills.clear();

        if !self.skills_dir.exists() {
            return Ok(());
        }

        let skills_dir = self.skills_dir.clone();
        self.discover_recursive(&skills_dir)?;
        Ok(())
    }

    fn discover_recursive(&mut self, dir: &Path) -> Result<()> {
        let entries = fs::read_dir(dir)
            .with_context(|| format!("reading skills dir {}", dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories.
                self.discover_recursive(&path)?;
            } else if path.file_name().map(|n| n == "SKILL.md").unwrap_or(false) {
                match Skill::from_file(&path) {
                    Ok(skill) => {
                        tracing::debug!("discovered skill: {} at {}", skill.name, path.display());
                        self.skills.push(skill);
                    }
                    Err(e) => {
                        tracing::warn!("failed to load skill at {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(())
    }

    /// Create a new skill from a task/approach pair.
    ///
    /// Writes a SKILL.md file to the skills directory and adds it to the registry.
    pub fn create_from_experience(&mut self, task: &str, approach: &str) -> Result<()> {
        // Generate a slug from the task description.
        let slug = slugify(task);
        let skill_dir = self.skills_dir.join(&slug);
        fs::create_dir_all(&skill_dir)
            .with_context(|| format!("creating skill directory {}", skill_dir.display()))?;

        let skill_path = skill_dir.join("SKILL.md");
        let content = format!(
            "# {}\n\n{}\n\n## Approach\n\n{}\n",
            task.trim(),
            format!("Skill learned from: {}", task.trim()),
            approach.trim(),
        );

        fs::write(&skill_path, &content)
            .with_context(|| format!("writing skill file {}", skill_path.display()))?;

        let skill = Skill {
            name: task.trim().to_string(),
            description: format!("Skill learned from: {}", task.trim()),
            content,
            file_path: skill_path,
            created_at: Utc::now(),
        };

        self.skills.push(skill);
        Ok(())
    }

    /// Get a skill by name (case-insensitive).
    pub fn get_skill(&self, name: &str) -> Option<&Skill> {
        let name_lower = name.to_lowercase();
        self.skills.iter().find(|s| s.name.to_lowercase() == name_lower)
    }

    /// Get all discovered skills.
    pub fn all_skills(&self) -> &[Skill] {
        &self.skills
    }

    /// Render all skills as a system prompt block.
    pub fn to_system_prompt_block(&self) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        let mut block = String::from(
            "\n## Available Skills\n\nLoad a skill's full instructions with the `skill` tool before following its workflow.\n\n",
        );
        for skill in &self.skills {
            block.push_str(&format!(
                "### {}\n{}\n<location>{}</location>\n\n",
                skill.name,
                skill.description,
                skill.file_path.display()
            ));
        }
        block
    }
}

/// Convert a task string into a filesystem-safe slug.
fn slugify(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else if c.is_whitespace() || c == '/' {
                '-'
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pr-skill-test-{}", uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    fn create_skill_file(dir: &Path, name: &str, description: &str) -> PathBuf {
        let skill_dir = dir.join(slugify(name));
        fs::create_dir_all(&skill_dir).unwrap();
        let path = skill_dir.join("SKILL.md");
        fs::write(
            &path,
            format!("# {}\n\n{}\n\n## Instructions\n\nDo the thing.\n", name, description),
        )
        .unwrap();
        path
    }

    #[test]
    fn test_parse_skill_header() {
        let content = "# My Skill\n\nThis is a description.\n\n## Details\n";
        let (name, desc) = parse_skill_header(content);
        assert_eq!(name, "My Skill");
        assert_eq!(desc, "This is a description.");
    }

    #[test]
    fn test_parse_skill_header_no_description() {
        let content = "# My Skill\n\n## Details\n";
        let (name, desc) = parse_skill_header(content);
        assert_eq!(name, "My Skill");
        assert!(desc.is_empty());
    }

    #[test]
    fn test_parse_skill_header_no_heading() {
        let content = "Just some text\n";
        let (name, desc) = parse_skill_header(content);
        assert!(name.is_empty());
        assert!(desc.is_empty());
    }

    #[test]
    fn test_skill_from_file() {
        let dir = test_dir();
        let path = create_skill_file(&dir, "web-search", "Search the web effectively");

        let skill = Skill::from_file(&path).unwrap();
        assert_eq!(skill.name, "web-search");
        assert_eq!(skill.description, "Search the web effectively");
        assert!(skill.content.contains("# web-search"));
        assert_eq!(skill.file_path, path);
        cleanup(&dir);
    }

    #[test]
    fn test_skill_from_file_fallback_name() {
        let dir = test_dir();
        let skill_dir = dir.join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        let path = skill_dir.join("SKILL.md");
        // No heading — should use directory name as fallback.
        fs::write(&path, "Just content.\n").unwrap();

        let skill = Skill::from_file(&path).unwrap();
        assert_eq!(skill.name, "my-skill");
        cleanup(&dir);
    }

    #[test]
    fn test_registry_discover() {
        let dir = test_dir();
        create_skill_file(&dir, "skill-one", "First skill");
        create_skill_file(&dir, "skill-two", "Second skill");

        let mut registry = SkillRegistry::with_dir(dir.clone());
        registry.discover().unwrap();

        assert_eq!(registry.all_skills().len(), 2);
        assert!(registry.get_skill("skill-one").is_some());
        assert!(registry.get_skill("skill-two").is_some());
        assert!(registry.get_skill("nonexistent").is_none());
        cleanup(&dir);
    }

    #[test]
    fn test_registry_discover_empty_dir() {
        let dir = test_dir();
        let mut registry = SkillRegistry::with_dir(dir.clone());
        registry.discover().unwrap();
        assert!(registry.all_skills().is_empty());
        cleanup(&dir);
    }

    #[test]
    fn test_registry_discover_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/pr-skill-nonexistent-uuid-12345");
        let mut registry = SkillRegistry::with_dir(dir);
        registry.discover().unwrap();
        assert!(registry.all_skills().is_empty());
    }

    #[test]
    fn test_registry_discover_nested() {
        let dir = test_dir();
        let sub = dir.join("category");
        fs::create_dir_all(&sub).unwrap();
        let skill_dir = sub.join("nested-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Nested Skill\n\nA nested skill.\n",
        )
        .unwrap();

        let mut registry = SkillRegistry::with_dir(dir.clone());
        registry.discover().unwrap();
        assert_eq!(registry.all_skills().len(), 1);
        assert_eq!(registry.all_skills()[0].name, "Nested Skill");
        cleanup(&dir);
    }

    #[test]
    fn test_registry_create_from_experience() {
        let dir = test_dir();
        let mut registry = SkillRegistry::with_dir(dir.clone());

        registry
            .create_from_experience(
                "How to search effectively",
                "Use multiple query phrasings and prefer academic sources.",
            )
            .unwrap();

        assert_eq!(registry.all_skills().len(), 1);
        let skill = &registry.all_skills()[0];
        assert_eq!(skill.name, "How to search effectively");
        assert!(skill.content.contains("Use multiple query phrasings"));

        // Verify the file was written.
        assert!(skill.file_path.exists());
        let file_content = fs::read_to_string(&skill.file_path).unwrap();
        assert!(file_content.contains("# How to search effectively"));
        cleanup(&dir);
    }

    #[test]
    fn test_registry_get_skill_case_insensitive() {
        let dir = test_dir();
        create_skill_file(&dir, "My-Skill", "A skill");

        let mut registry = SkillRegistry::with_dir(dir.clone());
        registry.discover().unwrap();

        assert!(registry.get_skill("my-skill").is_some());
        assert!(registry.get_skill("MY-SKILL").is_some());
        assert!(registry.get_skill("My-Skill").is_some());
        cleanup(&dir);
    }

    #[test]
    fn test_system_prompt_block_empty() {
        let dir = test_dir();
        let registry = SkillRegistry::with_dir(dir.clone());
        let block = registry.to_system_prompt_block();
        assert!(block.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn test_system_prompt_block_with_skills() {
        let dir = test_dir();
        create_skill_file(&dir, "web-search", "Search the web");

        let mut registry = SkillRegistry::with_dir(dir.clone());
        registry.discover().unwrap();

        let block = registry.to_system_prompt_block();
        assert!(block.contains("## Available Skills"));
        assert!(block.contains("### web-search"));
        assert!(block.contains("Search the web"));
        cleanup(&dir);
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("web_search"), "web_search");
        assert_eq!(slugify("a/b/c"), "a-b-c");
        assert_eq!(slugify("  spaces  "), "spaces");
        assert_eq!(slugify("special!@#chars"), "special-chars");
        assert_eq!(slugify("multiple---dashes"), "multiple-dashes");
    }

    #[test]
    fn test_new_store_paths() {
        let dir = test_dir();
        let registry = SkillRegistry::new(&dir);
        assert!(registry
            .skills_dir()
            .ends_with(".parallel-research/skills"));
        cleanup(&dir);
    }
}
