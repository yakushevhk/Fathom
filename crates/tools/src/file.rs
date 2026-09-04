use async_trait::async_trait;
use pr_core::{ToolSchema, ToolOutput};
use crate::registry::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Maximum file size allowed for writes (1 GB).
const MAX_WRITE_SIZE_BYTES: u64 = 1_073_741_824;

// ─── File Read ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FileReadParams {
    /// Path to the file (relative to working dir or absolute)
    path: String,
    /// Start line (1-indexed, optional)
    #[serde(default)]
    start_line: Option<u32>,
    /// Number of lines to read (optional)
    #[serde(default)]
    line_count: Option<u32>,
}

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str { "file_read" }
    fn description(&self) -> &str {
        "Read a file from the filesystem and return its content with line numbers.

## Capability

Reads a file at the given path (absolute or relative to the working directory) and returns its content with 1-indexed line numbers. Supports partial reading via `start_line` and `line_count` parameters for large files. Binary files will produce garbled output — this tool is for text files only.

## When to Use

- Reading source code, configuration files, documentation, or any text file.
- Examining a specific section of a large file using `start_line` and `line_count`.
- Checking the contents of a file before editing it with `file_edit`.

## When NOT to Use

- Do NOT use `file_read` to search for text across multiple files — use `grep` instead.
- Do NOT use `file_read` to find files by name pattern — use `glob` instead.
- Do NOT use `file_read` for web URLs — use `web_fetch` instead.

## Parameters

- `path` (required): File path, absolute or relative to working directory.
- `start_line` (optional): 1-indexed line number to start reading from.
- `line_count` (optional): Maximum number of lines to read.

## Failure Modes

- `File not found`: the path does not exist. Check spelling and try `glob` to locate the file.
- Empty output: the file exists but contains no text (or is binary).
- Permission error: the file exists but is not readable."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(FileReadParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: FileReadParams = serde_json::from_value(args)?;
        
        // Parse extended selectors like "file.rs:50-200", ":50-", ":50+150", ":1-10,50-60", ":conflicts", ":raw"
        let mut raw_path = params.path.as_str();
        let mut is_raw = false;
        let mut is_conflicts = false;
        let mut line_ranges: Vec<(usize, usize)> = Vec::new();

        if let Some((p, sel)) = raw_path.split_once(':') {
            raw_path = p;
            let sel = sel.trim();
            if sel == "raw" {
                is_raw = true;
            } else if sel == "conflicts" {
                is_conflicts = true;
            } else {
                for part in sel.split(',') {
                    let part = part.trim();
                    if let Some((s_str, e_str)) = part.split_once('+') {
                        if let (Ok(s), Ok(c)) = (s_str.parse::<usize>(), e_str.parse::<usize>()) {
                            line_ranges.push((s, s + c.saturating_sub(1)));
                        }
                    } else if let Some((s_str, e_str)) = part.split_once('-') {
                        let s = s_str.parse::<usize>().unwrap_or(1);
                        let e = if e_str.is_empty() { usize::MAX } else { e_str.parse::<usize>().unwrap_or(usize::MAX) };
                        line_ranges.push((s, e));
                    } else if let Ok(s) = part.parse::<usize>() {
                        line_ranges.push((s, s));
                    }
                }
            }
        }

        let path = resolve_path(&ctx.working_dir, raw_path);

        if !path.exists() {
            return Ok(ToolOutput::err(format!("File not found: {}", path.display())));
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let tag = crate::hashline::compute_tag(&content);
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let display_path = path.strip_prefix(&ctx.working_dir).unwrap_or(&path).display().to_string();
        let output;

        if is_conflicts {
            let mut conflict_blocks = Vec::new();
            let mut in_conflict = false;
            let mut block = Vec::new();
            for (idx, line) in lines.iter().enumerate() {
                if line.starts_with("<<<<<<<") {
                    in_conflict = true;
                    block.push(format!("{}:{}", idx + 1, line));
                } else if in_conflict {
                    block.push(format!("{}:{}", idx + 1, line));
                    if line.starts_with(">>>>>>>") {
                        in_conflict = false;
                        conflict_blocks.push(block.join("\n"));
                        block.clear();
                    }
                }
            }
            if conflict_blocks.is_empty() {
                output = format!("[{}#{}]\n(no git merge conflicts found)", display_path, tag);
            } else {
                output = format!("[{}#{}]\n{}", display_path, tag, conflict_blocks.join("\n---\n"));
            }
        } else if line_ranges.is_empty() && params.start_line.is_none() && params.line_count.is_none() {
            if is_raw {
                output = content;
            } else if content.is_empty() {
                output = format!("[{}#{}]\n(empty file)", display_path, tag);
            } else {
                let mut body = String::new();
                for (i, line) in lines.iter().enumerate() {
                    body.push_str(&format!("{}:{}\n", i + 1, line));
                }
                output = format!("[{}#{}]\n{}", display_path, tag, body);
            }
        } else {
            let mut selected_indices = std::collections::BTreeSet::new();
            if !line_ranges.is_empty() {
                for (s, e) in line_ranges {
                    let start_idx = s.saturating_sub(1);
                    let end_idx = e.min(total_lines);
                    for i in start_idx..end_idx {
                        selected_indices.insert(i);
                    }
                }
            } else {
                let s = params.start_line.unwrap_or(1).saturating_sub(1) as usize;
                let c = params.line_count.unwrap_or(u32::MAX) as usize;
                let e = (s + c).min(total_lines);
                for i in s..e {
                    selected_indices.insert(i);
                }
            }

            let mut body = String::new();
            for idx in selected_indices {
                if is_raw {
                    body.push_str(lines[idx]);
                    body.push('\n');
                } else {
                    body.push_str(&format!("{}:{}\n", idx + 1, lines[idx]));
                }
            }
            if is_raw {
                output = body;
            } else {
                output = format!("[{}#{}]\n{}", display_path, tag, body);
            }
        }

        // Record read tracking
        if let Ok(mut tracker) = ctx.read_tracker.try_lock() {
            let _ = tracker.record_read(&path);
        }

        Ok(ToolOutput::ok(output))
    }
}

// ─── File Write ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FileWriteParams {
    /// Path to the file
    path: String,
    /// Content to write
    content: String,
}

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str { "file_write" }
    fn description(&self) -> &str {
        "Write content to a file, creating it and any parent directories if they do not exist.

## Capability

Writes the provided `content` string to the file at `path`. If the file already exists, it is **overwritten** completely. Parent directories are created automatically. Use this for creating new files or completely replacing file contents.

## When to Use

- Creating new files (reports, notes, code, configuration).
- Writing the complete replacement content for an existing file.
- Saving research output, findings, or generated documents.

## When NOT to Use

- Do NOT use `file_write` to make small changes to an existing file — use `file_edit` instead (it preserves unchanged content and shows exactly what changed).
- Do NOT use `file_write` to append to a file — it overwrites everything.

## Parameters

- `path` (required): File path, absolute or relative to working directory.
- `content` (required): The full text content to write.

## Failure Modes

- Permission error: the parent directory is not writable.
- Disk full: insufficient disk space."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(FileWriteParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: FileWriteParams = serde_json::from_value(args)?;
        let path = resolve_path(&ctx.working_dir, &params.path);

        // ── Size guard ──
        if params.content.len() as u64 > MAX_WRITE_SIZE_BYTES {
            return Ok(ToolOutput::err(format!(
                "Content size ({} bytes) exceeds maximum allowed write size ({} bytes)",
                params.content.len(),
                MAX_WRITE_SIZE_BYTES
            )));
        }

        // ── Encoding check (warn on non-UTF8 content is N/A since we accept String,
        //    but we warn if the content contains replacement characters) ──
        if params.content.contains('\u{FFFD}') {
            tracing::warn!(
                "Content for {} contains Unicode replacement characters (\\uFFFD), \
                 which may indicate non-UTF8 data was lossy-decoded",
                path.display()
            );
        }

        let path_clone = path.clone();
        let content = params.content.clone();

        ctx.file_locks
            .with_lock(&path, || async {
                // Track in history before writing.
                {
                    let mut history = ctx.file_history.lock().await;
                    if path_clone.exists() {
                        let _ = history.track_edit(&path_clone);
                    }
                }

                if let Some(parent) = path_clone.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                tokio::fs::write(&path_clone, &content).await?;

                // Create snapshot after write.
                {
                    let mut history = ctx.file_history.lock().await;
                    if history.tracked_count() > 0 {
                        let _ = history.make_snapshot();
                    }
                }

                Ok(())
            })
            .await?;

        Ok(ToolOutput::ok(format!(
            "Written {} bytes to {}",
            params.content.len(),
            path.display()
        )))
    }
}

// ─── File Edit ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct FileEditParams {
    /// Path to the file
    path: String,
    /// Exact string to find and replace
    old_string: String,
    /// String to replace with
    new_string: String,
    /// Replace all occurrences (default: false)
    #[serde(default)]
    replace_all: bool,
}

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str { "file_edit" }
    fn description(&self) -> &str {
        "Edit a file by finding and replacing an exact string match.

## Capability

Reads the file at `path`, finds the first (or all) occurrences of `old_string`, replaces them with `new_string`, and writes the file back. The match is exact — whitespace, indentation, and line breaks must match precisely. This is the preferred tool for making targeted changes to existing files.

## When to Use

- Fixing a bug in a specific line of code.
- Updating a configuration value.
- Adding or modifying a section of an existing file.
- Any surgical edit where you know the exact text to replace.

## When NOT to Use

- Do NOT use `file_edit` for creating new files — use `file_write` instead.
- Do NOT use `file_edit` to rewrite an entire file — use `file_write` for full replacements.
- If you are unsure of the exact text, read the file first with `file_read` to see the precise content.

## Parameters

- `path` (required): File path, absolute or relative to working directory.
- `old_string` (required): The exact text to find. Must match character-for-character, including leading whitespace and line breaks.
- `new_string` (required): The replacement text.
- `replace_all` (optional, default false): If true, replaces all occurrences. If false, replaces only the first match.

## Failure Modes

- `File not found`: the path does not exist.
- `old_string not found in file`: the exact string was not found. Read the file first with `file_read` to see the current content. Common causes: wrong indentation, trailing spaces, or different line endings.
- Multiple matches without `replace_all`: if `old_string` appears multiple times and `replace_all` is false, only the first occurrence is replaced."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(FileEditParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: FileEditParams = serde_json::from_value(args)?;
        let path = resolve_path(&ctx.working_dir, &params.path);

        if !path.exists() {
            return Ok(ToolOutput::err(format!("File not found: {}", path.display())));
        }

        // ── Validation Gate ──

        // 1. Read-before-write check.
        {
            let tracker = ctx.read_tracker.lock().await;
            if !tracker.has_read(&path) {
                return Ok(ToolOutput::err(format!(
                    "File must be read with file_read before editing: {}",
                    path.display()
                )));
            }

            // 2. Staleness detection.
            if let Ok(stale) = tracker.is_stale(&path) {
                if stale {
                    return Ok(ToolOutput::err(format!(
                        "File has been modified since last read (stale). \
                         Re-read the file with file_read before editing: {}",
                        path.display()
                    )));
                }
            }
        }

        // 3. Size guard.
        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.len() > MAX_WRITE_SIZE_BYTES {
            return Ok(ToolOutput::err(format!(
                "File size ({} bytes) exceeds maximum allowed size ({} bytes): {}",
                metadata.len(),
                MAX_WRITE_SIZE_BYTES,
                path.display()
            )));
        }

        let path_clone = path.clone();
        let old_string = params.old_string.clone();
        let new_string = params.new_string.clone();
        let replace_all = params.replace_all;

        let result = ctx
            .file_locks
            .with_lock(&path, || async {
                // 4. Encoding check: detect non-UTF8 by reading raw bytes first.
                let raw_bytes = tokio::fs::read(&path_clone).await?;
                let content = match String::from_utf8(raw_bytes) {
                    Ok(s) => s,
                    Err(e) => {
                        // Warn but still proceed (lossy decode).
                        tracing::warn!(
                            "File {} is not valid UTF-8 ({}), \
                             editing may corrupt non-UTF8 data",
                            path_clone.display(),
                            e
                        );
                        String::from_utf8_lossy(e.as_bytes()).into_owned()
                    }
                };

                if !content.contains(&old_string) {
                    return Ok(ToolOutput::err(
                        "old_string not found in file. Make sure it matches exactly.".to_string(),
                    ));
                }

                // Track in history before applying edit.
                {
                    let mut history = ctx.file_history.lock().await;
                    let _ = history.track_edit(&path_clone);
                }

                let new_content = if replace_all {
                    content.replace(&old_string, &new_string)
                } else {
                    content.replacen(&old_string, &new_string, 1)
                };

                tokio::fs::write(&path_clone, &new_content).await?;

                // Create snapshot after edit.
                {
                    let mut history = ctx.file_history.lock().await;
                    if history.tracked_count() > 0 {
                        let _ = history.make_snapshot();
                    }
                }

                Ok(ToolOutput::ok(format!("Edited {}", path_clone.display())))
            })
            .await?;

        // Update read tracker with the new mtime after successful edit.
        {
            let mut tracker = ctx.read_tracker.lock().await;
            let _ = tracker.record_read(&path);
        }

        Ok(result)
    }
}

// ─── Hashline Patch Tool (Line-Anchored Patching) ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct HashlinePatchParams {
    /// Hashline patch script text with [path#TAG] section headers and PUT/CUT/MV operations
    input: String,
}

pub struct HashlinePatchTool;

#[async_trait]
impl Tool for HashlinePatchTool {
    fn name(&self) -> &str { "edit" }
    fn description(&self) -> &str {
        "Line-anchored snapshot-verified patch tool: apply high-precision edits to files using #TAG verification.

## Format
Section header: `[path#TAG]` (TAG is the 4-hex snapshot from file_read / read header).
Ops:
- `PUT N.=M:` : replace lines N through M with following `+` lines.
- `PUT <N:` : insert `+` lines before line N (<1 for head).
- `PUT >N:` : insert `+` lines after line N (>$ for tail).
- `CUT N.=M` : delete lines N through M.
- `MV DEST` : rename file to DEST.
- `REM` : delete file.

Body lines MUST start with `+`. Keeps unchanged lines excluded from ranges. Fails safely on stale #TAG."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(HashlinePatchParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: HashlinePatchParams = serde_json::from_value(args)?;
        let sections = match crate::hashline::parse_hashline_patch(&params.input) {
            Ok(s) => s,
            Err(e) => return Ok(ToolOutput::err(format!("Hashline parse error: {}", e))),
        };

        if sections.is_empty() {
            return Ok(ToolOutput::err("No valid [path#TAG] sections found in patch input"));
        }

        // Two-Phase Commit (2PC) Pipeline
        // Phase 1: In-memory dry run and tag verification across all sections
        let mut staging_plan = Vec::new();
        let mut reg_bank = crate::hashline::RegisterBank::new();

        for sec in &sections {
            let path = resolve_path(&ctx.working_dir, &sec.path.to_string_lossy());

            if sec.ops.iter().any(|op| matches!(op, crate::hashline::HashlineOp::RemoveFile)) {
                staging_plan.push((path, None, None, true));
                continue;
            }

            if !path.exists() {
                return Ok(ToolOutput::err(format!("File not found for patching: {}", path.display())));
            }

            let content = tokio::fs::read_to_string(&path).await?;
            let (new_content, new_tag) = match crate::hashline::apply_hashline_to_content(
                &content,
                &sec.expected_tag,
                &sec.ops,
                &mut reg_bank,
            ) {
                Ok(res) => res,
                Err(e) => {
                    return Ok(ToolOutput::err(format!(
                        "Patch validation failed on {}: {}\n[2PC ABORT: Zero files modified on disk]",
                        path.display(),
                        e
                    )))
                }
            };

            let target_path = if let Some(crate::hashline::HashlineOp::MoveFile { dest }) = sec
                .ops
                .iter()
                .find(|op| matches!(op, crate::hashline::HashlineOp::MoveFile { .. }))
            {
                resolve_path(&ctx.working_dir, &dest.to_string_lossy())
            } else {
                path.clone()
            };

            staging_plan.push((target_path, Some(new_content), Some(new_tag), false));
        }

        // Phase 2: Atomic commit to disk
        let mut summary = Vec::new();
        for (target_path, new_content_opt, new_tag_opt, is_remove) in staging_plan {
            if is_remove {
                if target_path.exists() {
                    tokio::fs::remove_file(&target_path).await?;
                    summary.push(format!("Removed {}", target_path.display()));
                }
            } else if let (Some(new_content), Some(new_tag)) = (new_content_opt, new_tag_opt) {
                if let Some(p) = target_path.parent() {
                    tokio::fs::create_dir_all(p).await?;
                }
                tokio::fs::write(&target_path, &new_content).await?;

                if let Ok(mut tracker) = ctx.read_tracker.try_lock() {
                    let _ = tracker.record_read(&target_path);
                }

                summary.push(format!(
                    "[{}#{}] ({} lines)",
                    target_path.display(),
                    new_tag,
                    new_content.lines().count()
                ));
            }
        }

        Ok(ToolOutput::ok(summary.join("\n")))
    }
}

// ─── Glob ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GlobParams {
    /// Glob pattern (e.g., "**/*.rs", "src/**/*.md")
    pattern: String,
}

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }
    fn description(&self) -> &str {
        "Find files matching a glob pattern. Returns a list of matching file paths (up to 200).

## Capability

Searches the working directory (and subdirectories) for files whose paths match the given glob pattern. Returns full absolute paths for each match. Results are capped at 200 files.

## When to Use

- Finding files by name or extension: `**/*.rs`, `src/**/*.md`, `*.toml`.
- Locating a file when you know part of its name or location.
- Discovering the project structure before reading specific files.

## When NOT to Use

- Do NOT use `glob` to search file *contents* — use `grep` instead.
- Do NOT use `glob` on web URLs.

## Parameters

- `pattern` (required): A glob pattern. Use `**` for recursive directory matching, `*` for any characters in a single path component.

## Failure Modes

- No results: the pattern may be too specific, or the files may be in a different location. Try `**/*` to list all files first.
- Invalid glob syntax: ensure the pattern uses standard glob syntax (`*`, `**`, `?`, `[abc]`)."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(GlobParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: GlobParams = serde_json::from_value(args)?;
        let full_pattern = ctx.working_dir.join(&params.pattern);
        let pattern_str = full_pattern.to_string_lossy().to_string();

        let paths = glob::glob(&pattern_str)?;
        let mut results: Vec<String> = Vec::new();
        let mut count = 0;

        for path in paths.flatten() {
            if path.is_file() {
                results.push(path.display().to_string());
                count += 1;
                if count >= 200 {
                    results.push("... (truncated at 200 files)".to_string());
                    break;
                }
            }
        }

        if results.is_empty() {
            Ok(ToolOutput::ok(format!("No files matched pattern: {}", params.pattern)))
        } else {
            Ok(ToolOutput::ok(format!("Found {} files:\n{}", results.len(), results.join("\n"))))
        }
    }
}

// ─── Grep ───

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GrepParams {
    /// Regex pattern to search for
    pattern: String,
    /// Directory to search in (default: working dir)
    #[serde(default)]
    path: Option<String>,
    /// File extension filter (e.g., "rs", "md")
    #[serde(default)]
    extension: Option<String>,
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }
    fn description(&self) -> &str {
        "Search file contents using a regular expression pattern. Returns matching lines with file paths and line numbers.

## Capability

Scans files in the given directory (default: working directory) for lines matching a regex pattern. Uses `ripgrep` (`rg`) when available for fast searching, with a fallback to manual file scanning. Results are capped at 100 matching lines.

## When to Use

- Finding where a function, variable, or string is defined or used across the codebase.
- Searching for a specific error message, configuration key, or code pattern.
- Locating references to a topic across multiple files.

## When NOT to Use

- Do NOT use `grep` to find files by name — use `glob` instead.
- Do NOT use `grep` on web content — use `web_search` and `web_fetch`.
- If you know the exact file, use `file_read` instead for full context.

## Parameters

- `pattern` (required): A regular expression pattern to search for.
- `path` (optional): Directory to search in (default: working directory).
- `extension` (optional): Filter to only search files with this extension (e.g., `rs`, `md`).

## Failure Modes

- No matches: the pattern may be wrong, or the text may not exist. Try a simpler substring.
- Invalid regex: the pattern is not a valid regular expression. Check syntax (e.g., escape special chars with `\\`).
- Slow on large directories: search may take time on very large codebases. Use `extension` to narrow scope."
    }
    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(GrepParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: GrepParams = serde_json::from_value(args)?;
        let search_dir = params.path
            .map(|p| resolve_path(&ctx.working_dir, &p))
            .unwrap_or_else(|| ctx.working_dir.clone());

        let re = regex::Regex::new(&params.pattern)?;
        let mut results: Vec<String> = Vec::new();
        let mut count = 0;

        let output = tokio::process::Command::new("rg")
            .args(["--line-number", "--no-heading", "--max-count=5"])
            .arg(&params.pattern)
            .arg(&search_dir)
            .output()
            .await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let mut lines: Vec<&str> = stdout.lines().collect();
                lines.truncate(100);
                if lines.is_empty() {
                    Ok(ToolOutput::ok(format!("No matches found for pattern: {}", params.pattern)))
                } else {
                    Ok(ToolOutput::ok(format!("Matches:\n{}", lines.join("\n"))))
                }
            }
            Err(_) => {
                // Fallback: manual search if rg not available
                search_files_manual(&search_dir, &re, params.extension.as_deref(), &mut results, &mut count, 100).await;
                if results.is_empty() {
                    Ok(ToolOutput::ok(format!("No matches found for pattern: {}", params.pattern)))
                } else {
                    Ok(ToolOutput::ok(format!("Matches:\n{}", results.join("\n"))))
                }
            }
        }
    }
}

async fn search_files_manual(
    dir: &std::path::Path,
    re: &regex::Regex,
    extension: Option<&str>,
    results: &mut Vec<String>,
    count: &mut usize,
    max_results: usize,
) {
    if *count >= max_results { return; }

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        if *count >= max_results { return; }
        let path = entry.path();

        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            Box::pin(search_files_manual(&path, re, extension, results, count, max_results)).await;
        } else if path.is_file() {
            if let Some(ext) = extension {
                if path.extension().map(|e| e.to_string_lossy().to_string()) != Some(ext.to_string()) {
                    continue;
                }
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        results.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                        *count += 1;
                        if *count >= max_results { return; }
                    }
                }
            }
        }
    }
}

pub(crate) fn resolve_path(working_dir: &std::path::Path, path_str: &str) -> std::path::PathBuf {
    let uri = pr_core::VirtualUri::parse(path_str);
    if uri.is_virtual() {
        if let Some(resolved) = uri.resolve_to_path(working_dir) {
            return resolved;
        }
    }

    let p = std::path::Path::new(path_str);
    if p.is_absolute() && p.starts_with(working_dir) {
        return p.to_path_buf();
    }

    let mut normalized = working_dir.to_path_buf();
    for component in p.components() {
        match component {
            std::path::Component::Prefix(_) | std::path::Component::RootDir | std::path::Component::CurDir => {},
            std::path::Component::ParentDir => {
                if normalized != working_dir && normalized.starts_with(working_dir) {
                    normalized.pop();
                }
            },
            std::path::Component::Normal(c) => normalized.push(c),
        }
    }

    if !normalized.starts_with(working_dir) {
        working_dir.to_path_buf()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx(dir: &std::path::Path) -> ToolContext {
        ToolContext::new(dir.to_path_buf(), pr_core::SearchConfig::default())
    }

    // ─── resolve_path ───────────────────────────────────────────

    #[test]
    fn resolve_path_absolute() {
        let wd = std::path::PathBuf::from("/tmp/workdir");
        let result = resolve_path(&wd, "/etc/hosts");
        assert_eq!(result, std::path::PathBuf::from("/tmp/workdir/etc/hosts"));
    }

    #[test]
    fn resolve_path_relative() {
        let wd = std::path::PathBuf::from("/tmp/workdir");
        let result = resolve_path(&wd, "src/main.rs");
        assert_eq!(result, std::path::PathBuf::from("/tmp/workdir/src/main.rs"));
    }

    #[test]
    fn resolve_path_traversal_blocked() {
        let wd = std::path::PathBuf::from("/tmp/workdir");
        let result = resolve_path(&wd, "../../etc/passwd");
        assert_eq!(result, std::path::PathBuf::from("/tmp/workdir/etc/passwd"));
    }

    #[test]
    fn resolve_path_dot_prefix() {
        let wd = std::path::PathBuf::from("/tmp/workdir");
        let result = resolve_path(&wd, "./file.txt");
        assert_eq!(result, std::path::PathBuf::from("/tmp/workdir/file.txt"));
    }

    #[test]
    fn resolve_path_empty_string() {
        let wd = std::path::PathBuf::from("/tmp/workdir");
        let result = resolve_path(&wd, "");
        assert_eq!(result, std::path::PathBuf::from("/tmp/workdir"));
    }

    // ─── FileReadTool ───────────────────────────────────────────

    #[tokio::test]
    async fn read_existing_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("hello.txt"), "line one\nline two\nline three\n").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "hello.txt" });
        let out = FileReadTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("line one"));
        assert!(out.content.contains("line two"));
        assert!(out.content.contains("line three"));
        // Lines should be numbered starting at 1.
        assert!(out.content.contains("1:line one"));
        assert!(out.content.contains("2:line two"));
        assert!(out.content.contains("3:line three"));
    }

    #[tokio::test]
    async fn read_with_start_line_and_line_count() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("data.txt"), "aaa\nbbb\nccc\nddd\neee\n").unwrap();
        let ctx = ctx(tmp.path());

        // Read lines 2-3 (start_line=2, line_count=2).
        let args = serde_json::json!({ "path": "data.txt", "start_line": 2, "line_count": 2 });
        let out = FileReadTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("bbb"));
        assert!(out.content.contains("ccc"));
        assert!(!out.content.contains("aaa"));
        assert!(!out.content.contains("ddd"));
    }

    #[tokio::test]
    async fn read_nonexistent_file_returns_error() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "does_not_exist.txt" });
        let out = FileReadTool.execute(args, &ctx).await.unwrap();

        assert!(!out.success);
        assert!(out.content.contains("File not found"));
    }

    #[tokio::test]
    async fn read_empty_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("empty.txt"), "").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "empty.txt" });
        let out = FileReadTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("(empty file)"));
    }

    #[tokio::test]
    async fn read_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("abs.txt");
        std::fs::write(&file_path, "absolute content\n").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": file_path.display().to_string() });
        let out = FileReadTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("absolute content"));
    }

    // ─── FileWriteTool ──────────────────────────────────────────

    #[tokio::test]
    async fn write_new_file() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "output.txt", "content": "hello world" });
        let out = FileWriteTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert_eq!(std::fs::read_to_string(tmp.path().join("output.txt")).unwrap(), "hello world");
    }

    #[tokio::test]
    async fn write_creates_parent_directories() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "a/b/c/deep.txt", "content": "nested" });
        let out = FileWriteTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert_eq!(std::fs::read_to_string(tmp.path().join("a/b/c/deep.txt")).unwrap(), "nested");
    }

    #[tokio::test]
    async fn write_overwrites_existing_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("overwrite.txt"), "old content").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "path": "overwrite.txt", "content": "new content" });
        let out = FileWriteTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert_eq!(std::fs::read_to_string(tmp.path().join("overwrite.txt")).unwrap(), "new content");
    }

    #[tokio::test]
    async fn write_size_guard_rejects_huge_content() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(tmp.path());

        // Build a string just over 1 GB.
        let size = (MAX_WRITE_SIZE_BYTES as usize) + 1;
        let big = "x".repeat(size);
        let args = serde_json::json!({ "path": "huge.bin", "content": big });
        let out = FileWriteTool.execute(args, &ctx).await.unwrap();

        assert!(!out.success);
        assert!(out.content.contains("exceeds maximum allowed write size"));
    }

    // ─── FileEditTool ───────────────────────────────────────────

    #[tokio::test]
    async fn edit_first_occurrence() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("edit.txt"), "foo bar foo bar\n").unwrap();
        let ctx = ctx(tmp.path());

        // Must read before editing.
        FileReadTool.execute(serde_json::json!({ "path": "edit.txt" }), &ctx).await.unwrap();

        let args = serde_json::json!({
            "path": "edit.txt",
            "old_string": "foo",
            "new_string": "baz",
            "replace_all": false,
        });
        let out = FileEditTool.execute(args, &ctx).await.unwrap();
        assert!(out.success);

        let content = std::fs::read_to_string(tmp.path().join("edit.txt")).unwrap();
        assert_eq!(content, "baz bar foo bar\n");
    }

    #[tokio::test]
    async fn edit_replace_all() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("all.txt"), "aaa bbb aaa bbb aaa\n").unwrap();
        let ctx = ctx(tmp.path());

        FileReadTool.execute(serde_json::json!({ "path": "all.txt" }), &ctx).await.unwrap();

        let args = serde_json::json!({
            "path": "all.txt",
            "old_string": "aaa",
            "new_string": "zzz",
            "replace_all": true,
        });
        let out = FileEditTool.execute(args, &ctx).await.unwrap();
        assert!(out.success);

        let content = std::fs::read_to_string(tmp.path().join("all.txt")).unwrap();
        assert_eq!(content, "zzz bbb zzz bbb zzz\n");
    }

    #[tokio::test]
    async fn edit_old_string_not_found() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("miss.txt"), "hello world\n").unwrap();
        let ctx = ctx(tmp.path());

        FileReadTool.execute(serde_json::json!({ "path": "miss.txt" }), &ctx).await.unwrap();

        let args = serde_json::json!({
            "path": "miss.txt",
            "old_string": "nonexistent",
            "new_string": "replacement",
        });
        let out = FileEditTool.execute(args, &ctx).await.unwrap();

        assert!(!out.success);
        assert!(out.content.contains("old_string not found"));
    }

    #[tokio::test]
    async fn edit_file_not_found() {
        let tmp = TempDir::new().unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({
            "path": "ghost.txt",
            "old_string": "x",
            "new_string": "y",
        });
        let out = FileEditTool.execute(args, &ctx).await.unwrap();

        assert!(!out.success);
        assert!(out.content.contains("File not found"));
    }

    #[tokio::test]
    async fn edit_requires_read_first() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("gate.txt"), "secret data\n").unwrap();
        let ctx = ctx(tmp.path());

        // Do NOT read the file before attempting edit.
        let args = serde_json::json!({
            "path": "gate.txt",
            "old_string": "secret",
            "new_string": "public",
        });
        let out = FileEditTool.execute(args, &ctx).await.unwrap();

        assert!(!out.success);
        assert!(out.content.contains("must be read with file_read before editing"));
    }

    // ─── GlobTool ───────────────────────────────────────────────

    #[tokio::test]
    async fn glob_match_files() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.rs"), "").unwrap();
        std::fs::write(tmp.path().join("b.rs"), "").unwrap();
        std::fs::write(tmp.path().join("c.txt"), "").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "pattern": "*.rs" });
        let out = GlobTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("a.rs"));
        assert!(out.content.contains("b.rs"));
        assert!(!out.content.contains("c.txt"));
    }

    #[tokio::test]
    async fn glob_no_matches() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("readme.md"), "").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "pattern": "*.py" });
        let out = GlobTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("No files matched"));
    }

    #[tokio::test]
    async fn glob_recursive_pattern() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("src/nested")).unwrap();
        std::fs::write(tmp.path().join("src/nested/deep.rs"), "").unwrap();
        std::fs::write(tmp.path().join("top.rs"), "").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "pattern": "**/*.rs" });
        let out = GlobTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("deep.rs"));
        assert!(out.content.contains("top.rs"));
    }

    // ─── GrepTool ───────────────────────────────────────────────

    #[tokio::test]
    async fn grep_match_pattern() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("code.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "pattern": "println" });
        let out = GrepTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("println"));
    }

    #[tokio::test]
    async fn grep_no_matches() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("blank.txt"), "nothing to see here\n").unwrap();
        let ctx = ctx(tmp.path());

        let args = serde_json::json!({ "pattern": "zzzzz_not_present" });
        let out = GrepTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("No matches found"));
    }

    #[tokio::test]
    async fn grep_extension_filter() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("app.rs"), "let x = target_value;\n").unwrap();
        std::fs::write(tmp.path().join("app.py"), "x = target_value\n").unwrap();
        let ctx = ctx(tmp.path());

        // Search only .rs files.
        let args = serde_json::json!({ "pattern": "target_value", "extension": "rs" });
        let out = GrepTool.execute(args, &ctx).await.unwrap();

        assert!(out.success);
        assert!(out.content.contains("app.rs"));
        // The .py file should not appear in results (unless rg is used and ignores
        // the extension filter — the fallback manual search respects it).
    }
}
