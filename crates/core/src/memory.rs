//! File-backed memory store inspired by Hermes (MEMORY.md / USER.md).
//!
//! Entries are delimited by `§` in the backing files. Each file has a character
//! budget that is enforced on write. Typed memories use YAML frontmatter for
//! metadata (name, description, type).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ── Constants ────────────────────────────────────────────────────────────────

const MEMORY_FILENAME: &str = "MEMORY.md";
const USER_FILENAME: &str = "USER.md";
const DEFAULT_MAX_MEMORY_CHARS: usize = 2200;
const DEFAULT_MAX_USER_CHARS: usize = 1375;
const ENTRY_DELIMITER: char = '\u{00A7}'; // §

// ── MemoryType ───────────────────────────────────────────────────────────────

/// Typed memory categories (inspired by OpenClaude).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    /// User preferences, role, identity.
    User,
    /// Corrections and confirmed approaches.
    Feedback,
    /// Ongoing work, goals, project context.
    Project,
    /// Pointers to external resources.
    Reference,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Feedback => write!(f, "feedback"),
            Self::Project => write!(f, "project"),
            Self::Reference => write!(f, "reference"),
        }
    }
}

impl std::str::FromStr for MemoryType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "user" => Ok(Self::User),
            "feedback" => Ok(Self::Feedback),
            "project" => Ok(Self::Project),
            "reference" => Ok(Self::Reference),
            _ => Err(anyhow::anyhow!("unknown memory type: {}", s)),
        }
    }
}

// ── Frontmatter ──────────────────────────────────────────────────────────────

/// YAML-like frontmatter metadata for typed memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
}

impl Frontmatter {
    /// Serialize to a simple YAML-like frontmatter block.
    pub fn to_frontmatter_string(&self) -> String {
        format!(
            "---\nname: {}\ndescription: {}\ntype: {}\n---",
            escape_yaml_value(&self.name),
            escape_yaml_value(&self.description),
            self.memory_type,
        )
    }
}

/// Escape a string for safe embedding in a YAML value position.
/// Wraps in quotes if it contains special characters.
fn escape_yaml_value(s: &str) -> String {
    if s.is_empty()
        || s.contains(':')
        || s.contains('#')
        || s.contains('"')
        || s.contains('\'')
        || s.starts_with(' ')
        || s.ends_with(' ')
    {
        format!(
            "\"{}\"",
            s.replace('\\', "\\\\").replace('"', "\\\"")
        )
    } else {
        s.to_string()
    }
}

/// Parse YAML-like frontmatter from a string. Expects `---` delimiters.
/// Returns (frontmatter, body_after_frontmatter).
pub fn parse_frontmatter(input: &str) -> Option<(Frontmatter, &str)> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end = after_first.find("---")?;
    let yaml_block = &after_first[..end].trim();
    let body_start = 3 + end + 3;
    let body = trimmed[body_start..].trim();

    // Simple key-value parser (no full YAML dep needed)
    let mut name = String::new();
    let mut description = String::new();
    let mut memory_type = MemoryType::Project;

    for line in yaml_block.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("name:") {
            name = unquote_yaml(val.trim());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = unquote_yaml(val.trim());
        } else if let Some(val) = line.strip_prefix("type:") {
            if let Ok(mt) = val.trim().parse::<MemoryType>() {
                memory_type = mt;
            }
        }
    }

    if name.is_empty() {
        return None;
    }

    Some((
        Frontmatter {
            name,
            description,
            memory_type,
        },
        body,
    ))
}

/// Remove surrounding quotes from a YAML value if present.
fn unquote_yaml(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ── MemoryEntry ──────────────────────────────────────────────────────────────

/// A single memory entry with content and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub content: String,
    pub created_at: DateTime<Utc>,
}

impl MemoryEntry {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            created_at: Utc::now(),
        }
    }
}

// ── TypedMemoryEntry ─────────────────────────────────────────────────────────

/// A memory entry with typed metadata (frontmatter).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedMemoryEntry {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

impl TypedMemoryEntry {
    /// Serialize to a full entry string with frontmatter.
    pub fn to_entry_string(&self) -> String {
        format!(
            "{}\n{}",
            self.frontmatter.to_frontmatter_string(),
            self.body,
        )
    }
}

// ── MemoryOp ─────────────────────────────────────────────────────────────────

/// A batch operation on the memory store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOp {
    pub action: MemoryAction,
    pub target: MemoryTarget,
    pub content: Option<String>,
    pub old_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryAction {
    Add,
    Replace,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryTarget {
    Memory,
    User,
}

impl std::fmt::Display for MemoryTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory => write!(f, "memory"),
            Self::User => write!(f, "user"),
        }
    }
}

// ── MemoryStore ──────────────────────────────────────────────────────────────

/// File-backed memory store using MEMORY.md and USER.md.
///
/// Entries are joined with the `§` delimiter in each file. Character budgets
/// are enforced on write to keep prompt injection bounded.
pub struct MemoryStore {
    memory_path: PathBuf,
    user_path: PathBuf,
    pub max_memory_chars: usize,
    pub max_user_chars: usize,
}

impl MemoryStore {
    /// Create a new MemoryStore rooted at `~/.fathom/memory/`.
    pub fn new(home_dir: &Path) -> Self {
        let base = home_dir.join(".fathom").join("memory");
        Self {
            memory_path: base.join(MEMORY_FILENAME),
            user_path: base.join(USER_FILENAME),
            max_memory_chars: DEFAULT_MAX_MEMORY_CHARS,
            max_user_chars: DEFAULT_MAX_USER_CHARS,
        }
    }

    /// Create with custom budgets (for testing).
    pub fn with_budgets(
        home_dir: &Path,
        max_memory_chars: usize,
        max_user_chars: usize,
    ) -> Self {
        let base = home_dir.join(".fathom").join("memory");
        Self {
            memory_path: base.join(MEMORY_FILENAME),
            user_path: base.join(USER_FILENAME),
            max_memory_chars,
            max_user_chars,
        }
    }

    /// Get the memory file path.
    pub fn memory_path(&self) -> &Path {
        &self.memory_path
    }

    /// Get the user file path.
    pub fn user_path(&self) -> &Path {
        &self.user_path
    }

    // ── Load ─────────────────────────────────────────────────────────────

    /// Load all memory entries from MEMORY.md.
    pub fn load_memory(&self) -> Vec<MemoryEntry> {
        Self::load_entries_from_file(&self.memory_path)
    }

    /// Load all user entries from USER.md.
    pub fn load_user(&self) -> Vec<MemoryEntry> {
        Self::load_entries_from_file(&self.user_path)
    }

    fn load_entries_from_file(path: &Path) -> Vec<MemoryEntry> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        Self::parse_entries(&content)
    }

    /// Public access to file-based entry loading (for tool use).
    pub fn load_entries_from_path(path: &Path) -> Vec<MemoryEntry> {
        Self::load_entries_from_file(path)
    }

    fn parse_entries(content: &str) -> Vec<MemoryEntry> {
        if content.trim().is_empty() {
            return Vec::new();
        }
        content
            .split(ENTRY_DELIMITER)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| MemoryEntry::new(s))
            .collect()
    }

    // ── Add ──────────────────────────────────────────────────────────────

    /// Add an entry to MEMORY.md. Enforces character budget.
    pub fn add_memory(&mut self, content: &str) -> Result<()> {
        self.add_to_file(&self.memory_path.clone(), content, self.max_memory_chars)
    }

    /// Add an entry to USER.md. Enforces character budget.
    pub fn add_user(&mut self, content: &str) -> Result<()> {
        self.add_to_file(&self.user_path.clone(), content, self.max_user_chars)
    }

    fn add_to_file(&self, path: &Path, content: &str, max_chars: usize) -> Result<()> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("cannot add empty memory entry"));
        }

        let mut entries = Self::load_entries_from_file(path);

        // Check if the entry already exists (exact match).
        if entries.iter().any(|e| e.content == trimmed) {
            return Ok(()); // already present
        }

        entries.push(MemoryEntry::new(trimmed));
        let serialized = Self::serialize_entries(&entries);

        // Enforce budget: if over budget, drop oldest entries until within limit.
        let final_content = Self::enforce_budget(&serialized, max_chars);

        Self::atomic_write(path, &final_content)?;
        Ok(())
    }

    // ── Replace ──────────────────────────────────────────────────────────

    /// Replace an entry in MEMORY.md whose content contains `old_substr`.
    pub fn replace_memory(&mut self, old_substr: &str, new_content: &str) -> Result<()> {
        self.replace_in_file(
            &self.memory_path.clone(),
            old_substr,
            new_content,
            self.max_memory_chars,
        )
    }

    fn replace_in_file(
        &self,
        path: &Path,
        old_substr: &str,
        new_content: &str,
        max_chars: usize,
    ) -> Result<()> {
        let mut entries = Self::load_entries_from_file(path);
        let idx = entries
            .iter()
            .position(|e| e.content.contains(old_substr))
            .ok_or_else(|| anyhow::anyhow!("no entry containing '{}'", old_substr))?;

        entries[idx] = MemoryEntry::new(new_content.trim());
        let serialized = Self::serialize_entries(&entries);
        let final_content = Self::enforce_budget(&serialized, max_chars);
        Self::atomic_write(path, &final_content)?;
        Ok(())
    }

    // ── Remove ───────────────────────────────────────────────────────────

    /// Remove an entry from MEMORY.md whose content contains `substr`.
    pub fn remove_memory(&mut self, substr: &str) -> Result<()> {
        self.remove_from_file(&self.memory_path.clone(), substr)
    }

    fn remove_from_file(&self, path: &Path, substr: &str) -> Result<()> {
        let mut entries = Self::load_entries_from_file(path);
        let before = entries.len();
        entries.retain(|e| !e.content.contains(substr));

        if entries.len() == before {
            return Err(anyhow::anyhow!("no entry containing '{}'", substr));
        }

        let serialized = Self::serialize_entries(&entries);
        Self::atomic_write(path, &serialized)?;
        Ok(())
    }

    // ── Batch ────────────────────────────────────────────────────────────

    /// Execute a batch of memory operations atomically.
    ///
    /// All operations are collected and applied to the in-memory representation
    /// before writing to disk. If any operation fails, the entire batch is
    /// aborted (no partial writes).
    pub fn batch_operations(&mut self, ops: Vec<MemoryOp>) -> Result<()> {
        // Load current state.
        let mut memory_entries = self.load_memory();
        let mut user_entries = self.load_user();

        for op in &ops {
            let entries = match op.target {
                MemoryTarget::Memory => &mut memory_entries,
                MemoryTarget::User => &mut user_entries,
            };
            let max_chars = match op.target {
                MemoryTarget::Memory => self.max_memory_chars,
                MemoryTarget::User => self.max_user_chars,
            };

            match op.action {
                MemoryAction::Add => {
                    let content = op
                        .content
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("batch Add requires content"))?;
                    let trimmed = content.trim();
                    if trimmed.is_empty() {
                        return Err(anyhow::anyhow!("batch Add: empty content"));
                    }
                    if !entries.iter().any(|e| e.content == trimmed) {
                        entries.push(MemoryEntry::new(trimmed));
                    }
                    // Budget is enforced after all ops.
                    let _ = max_chars;
                }
                MemoryAction::Replace => {
                    let old = op
                        .old_text
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("batch Replace requires old_text"))?;
                    let new = op
                        .content
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("batch Replace requires content"))?;
                    let idx = entries
                        .iter()
                        .position(|e| e.content.contains(old))
                        .ok_or_else(|| {
                            anyhow::anyhow!("batch Replace: no entry containing '{}'", old)
                        })?;
                    entries[idx] = MemoryEntry::new(new.trim());
                }
                MemoryAction::Remove => {
                    let old = op
                        .old_text
                        .as_deref()
                        .ok_or_else(|| anyhow::anyhow!("batch Remove requires old_text"))?;
                    let before = entries.len();
                    entries.retain(|e| !e.content.contains(old));
                    if entries.len() == before {
                        return Err(anyhow::anyhow!(
                            "batch Remove: no entry containing '{}'",
                            old
                        ));
                    }
                }
            }
        }

        // Serialize and enforce budgets.
        let mem_serialized = Self::serialize_entries(&memory_entries);
        let user_serialized = Self::serialize_entries(&user_entries);

        let mem_final = Self::enforce_budget(&mem_serialized, self.max_memory_chars);
        let user_final = Self::enforce_budget(&user_serialized, self.max_user_chars);

        // Atomic write of both files.
        Self::atomic_write(&self.memory_path, &mem_final)?;
        Self::atomic_write(&self.user_path, &user_final)?;

        Ok(())
    }

    // ── System prompt block ──────────────────────────────────────────────

    /// Render memory and user entries as a system prompt block.
    pub fn to_system_prompt_block(&self) -> String {
        let memory_entries = self.load_memory();
        let user_entries = self.load_user();

        if memory_entries.is_empty() && user_entries.is_empty() {
            return String::new();
        }

        let mut block = String::from("\n## Memory\n\n");

        if !memory_entries.is_empty() {
            block.push_str("### Persistent Memory\n");
            for entry in &memory_entries {
                block.push_str(&format!("- {}\n", entry.content));
            }
            block.push('\n');
        }

        if !user_entries.is_empty() {
            block.push_str("### User Context\n");
            for entry in &user_entries {
                block.push_str(&format!("- {}\n", entry.content));
            }
            block.push('\n');
        }

        block
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    /// Serialize entries with the § delimiter.
    fn serialize_entries(entries: &[MemoryEntry]) -> String {
        entries
            .iter()
            .map(|e| e.content.as_str())
            .collect::<Vec<_>>()
            .join(&format!("{}\n", ENTRY_DELIMITER))
    }

    /// Truncate content to fit within `max_chars` by dropping oldest entries
    /// from the front. Returns the content as-is if already within budget.
    fn enforce_budget(content: &str, max_chars: usize) -> String {
        if content.len() <= max_chars {
            return content.to_string();
        }

        // Split into entries, keep newest (from the end).
        let entries: Vec<&str> = content
            .split(ENTRY_DELIMITER)
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut result_entries: Vec<&str> = Vec::new();
        let mut total = 0usize;

        for entry in entries.iter().rev() {
            let entry_cost = entry.len() + 2; // § + newline
            if total + entry_cost > max_chars && !result_entries.is_empty() {
                break;
            }
            result_entries.push(entry);
            total += entry_cost;
        }

        result_entries.reverse();
        result_entries
            .iter()
            .map(|s| *s)
            .collect::<Vec<_>>()
            .join(&format!("{}\n", ENTRY_DELIMITER))
    }

    /// Write to a temp file and atomically rename (prevents partial writes).
    fn atomic_write(path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }

        let tmp_path = path.with_extension("md.tmp");
        fs::write(&tmp_path, content)
            .with_context(|| format!("writing temp file {}", tmp_path.display()))?;

        fs::rename(&tmp_path, path)
            .with_context(|| format!("renaming {} -> {}", tmp_path.display(), path.display()))?;

        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pr-memory-test-{}", uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_new_store_creates_paths() {
        let dir = test_dir();
        let store = MemoryStore::new(&dir);
        assert!(store.memory_path().ends_with(".fathom/memory/MEMORY.md"));
        assert!(store.user_path().ends_with(".fathom/memory/USER.md"));
        cleanup(&dir);
    }

    #[test]
    fn test_add_and_load_memory() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("first entry").unwrap();
        store.add_memory("second entry").unwrap();

        let entries = store.load_memory();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "first entry");
        assert_eq!(entries[1].content, "second entry");
        cleanup(&dir);
    }

    #[test]
    fn test_add_and_load_user() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_user("prefers dark mode").unwrap();
        let entries = store.load_user();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "prefers dark mode");
        cleanup(&dir);
    }

    #[test]
    fn test_duplicate_entry_ignored() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("same entry").unwrap();
        store.add_memory("same entry").unwrap();

        let entries = store.load_memory();
        assert_eq!(entries.len(), 1);
        cleanup(&dir);
    }

    #[test]
    fn test_replace_memory() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("old value").unwrap();
        store.replace_memory("old", "new value").unwrap();

        let entries = store.load_memory();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "new value");
        cleanup(&dir);
    }

    #[test]
    fn test_remove_memory() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("keep this").unwrap();
        store.add_memory("remove this").unwrap();
        store.remove_memory("remove this").unwrap();

        let entries = store.load_memory();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "keep this");
        cleanup(&dir);
    }

    #[test]
    fn test_remove_nonexistent_fails() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("exists").unwrap();
        let result = store.remove_memory("does not exist");
        assert!(result.is_err());
        cleanup(&dir);
    }

    #[test]
    fn test_budget_enforcement() {
        let dir = test_dir();
        // Budget of 50 chars: each entry is 16 chars + § + newline = 18 chars.
        // 3 entries = 54 chars raw, should trim to 2 newest.
        let mut store = MemoryStore::with_budgets(&dir, 50, 50);

        store.add_memory("first_long_entry").unwrap();
        store.add_memory("second_long_entry").unwrap();
        store.add_memory("third_long_entry").unwrap();

        let entries = store.load_memory();
        // With a 50-char budget and 18-char entries, should keep ~2 newest.
        let raw = fs::read_to_string(store.memory_path()).unwrap();
        assert!(
            raw.len() <= 50,
            "file size {} exceeds budget of 50",
            raw.len()
        );
        // Oldest entry ("first_long_entry") should have been dropped.
        assert!(entries.iter().all(|e| e.content != "first_long_entry"));
        cleanup(&dir);
    }

    #[test]
    fn test_empty_content_rejected() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        let result = store.add_memory("");
        assert!(result.is_err());

        let result = store.add_memory("   ");
        assert!(result.is_err());
        cleanup(&dir);
    }

    #[test]
    fn test_load_empty_file() {
        let dir = test_dir();
        let store = MemoryStore::with_budgets(&dir, 5000, 5000);

        let entries = store.load_memory();
        assert!(entries.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn test_batch_operations() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("existing").unwrap();

        let ops = vec![
            MemoryOp {
                action: MemoryAction::Add,
                target: MemoryTarget::Memory,
                content: Some("batch added".to_string()),
                old_text: None,
            },
            MemoryOp {
                action: MemoryAction::Add,
                target: MemoryTarget::User,
                content: Some("user batch".to_string()),
                old_text: None,
            },
            MemoryOp {
                action: MemoryAction::Replace,
                target: MemoryTarget::Memory,
                content: Some("replaced".to_string()),
                old_text: Some("existing".to_string()),
            },
        ];

        store.batch_operations(ops).unwrap();

        let mem = store.load_memory();
        assert_eq!(mem.len(), 2);
        assert!(mem.iter().any(|e| e.content == "batch added"));
        assert!(mem.iter().any(|e| e.content == "replaced"));

        let user = store.load_user();
        assert_eq!(user.len(), 1);
        assert_eq!(user[0].content, "user batch");
        cleanup(&dir);
    }

    #[test]
    fn test_batch_remove() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("keep").unwrap();
        store.add_memory("drop this").unwrap();

        let ops = vec![MemoryOp {
            action: MemoryAction::Remove,
            target: MemoryTarget::Memory,
            content: None,
            old_text: Some("drop".to_string()),
        }];

        store.batch_operations(ops).unwrap();
        let mem = store.load_memory();
        assert_eq!(mem.len(), 1);
        assert_eq!(mem[0].content, "keep");
        cleanup(&dir);
    }

    #[test]
    fn test_system_prompt_block_empty() {
        let dir = test_dir();
        let store = MemoryStore::with_budgets(&dir, 5000, 5000);
        let block = store.to_system_prompt_block();
        assert!(block.is_empty());
        cleanup(&dir);
    }

    #[test]
    fn test_system_prompt_block_with_entries() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("fact about user").unwrap();
        store.add_user("prefers concise answers").unwrap();

        let block = store.to_system_prompt_block();
        assert!(block.contains("## Memory"));
        assert!(block.contains("fact about user"));
        assert!(block.contains("prefers concise answers"));
        assert!(block.contains("Persistent Memory"));
        assert!(block.contains("User Context"));
        cleanup(&dir);
    }

    #[test]
    fn test_entry_serialization_roundtrip() {
        let dir = test_dir();
        let mut store = MemoryStore::with_budgets(&dir, 5000, 5000);

        store.add_memory("entry one").unwrap();
        store.add_memory("entry two").unwrap();
        store.add_memory("entry three").unwrap();

        // Verify the file contains § delimiters.
        let raw = fs::read_to_string(store.memory_path()).unwrap();
        assert!(raw.contains('\u{00A7}'));

        let entries = store.load_memory();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].content, "entry one");
        assert_eq!(entries[1].content, "entry two");
        assert_eq!(entries[2].content, "entry three");
        cleanup(&dir);
    }

    #[test]
    fn test_frontmatter_parse() {
        let input = r#"---
name: test-memory
description: A test entry
type: feedback
---
This is the body content."#;

        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.name, "test-memory");
        assert_eq!(fm.description, "A test entry");
        assert_eq!(fm.memory_type, MemoryType::Feedback);
        assert_eq!(body, "This is the body content.");
    }

    #[test]
    fn test_frontmatter_parse_quoted_values() {
        let input = r#"---
name: "quoted: name"
description: 'single quoted'
type: reference
---
body"#;

        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.name, "quoted: name");
        assert_eq!(fm.description, "single quoted");
        assert_eq!(fm.memory_type, MemoryType::Reference);
        assert_eq!(body, "body");
    }

    #[test]
    fn test_frontmatter_no_frontmatter() {
        let input = "just plain text";
        assert!(parse_frontmatter(input).is_none());
    }

    #[test]
    fn test_frontmatter_roundtrip() {
        let fm = Frontmatter {
            name: "test".to_string(),
            description: "desc".to_string(),
            memory_type: MemoryType::Project,
        };
        let entry = TypedMemoryEntry {
            frontmatter: fm,
            body: "the body".to_string(),
            created_at: Utc::now(),
        };

        let serialized = entry.to_entry_string();
        let (parsed_fm, parsed_body) = parse_frontmatter(&serialized).unwrap();
        assert_eq!(parsed_fm.name, "test");
        assert_eq!(parsed_fm.memory_type, MemoryType::Project);
        assert_eq!(parsed_body, "the body");
    }

    #[test]
    fn test_memory_type_from_str() {
        assert_eq!("user".parse::<MemoryType>().unwrap(), MemoryType::User);
        assert_eq!(
            "feedback".parse::<MemoryType>().unwrap(),
            MemoryType::Feedback
        );
        assert_eq!(
            "project".parse::<MemoryType>().unwrap(),
            MemoryType::Project
        );
        assert_eq!(
            "reference".parse::<MemoryType>().unwrap(),
            MemoryType::Reference
        );
        assert!("invalid".parse::<MemoryType>().is_err());
    }

    #[test]
    fn test_escape_yaml_value() {
        assert_eq!(escape_yaml_value("simple"), "simple");
        assert_eq!(escape_yaml_value("has:colon"), "\"has:colon\"");
        assert_eq!(escape_yaml_value("has#hash"), "\"has#hash\"");
        assert_eq!(
            escape_yaml_value("has \"quotes\""),
            "\"has \\\"quotes\\\"\""
        );
    }

    #[test]
    fn test_atomic_write_creates_parent_dirs() {
        let dir = test_dir();
        let nested = dir.join("a").join("b").join("c").join("test.md");
        MemoryStore::atomic_write(&nested, "content").unwrap();
        assert_eq!(fs::read_to_string(&nested).unwrap(), "content");
        cleanup(&dir);
    }

    #[test]
    fn test_with_budgets_custom() {
        let dir = test_dir();
        let store = MemoryStore::with_budgets(&dir, 100, 50);
        assert_eq!(store.max_memory_chars, 100);
        assert_eq!(store.max_user_chars, 50);
        cleanup(&dir);
    }
}
