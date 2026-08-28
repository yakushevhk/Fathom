use std::collections::HashMap;
use std::path::PathBuf;
use pr_core::{PrError, PrResult};

/// Compute a 4-character hex snapshot tag for file content (CRC-16 based).
pub fn compute_tag(content: &str) -> String {
    let mut crc: u16 = 0xFFFF;
    for byte in content.as_bytes() {
        crc ^= (*byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    format!("{:04X}", crc)
}

/// Global or patch-scoped named register bank for cut/paste operations.
#[derive(Debug, Clone, Default)]
pub struct RegisterBank {
    pub registers: HashMap<String, Vec<String>>,
    pub anonymous: Vec<String>,
}

impl RegisterBank {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: Option<&str>, content: Vec<String>) {
        if let Some(n) = name {
            self.registers.insert(n.trim_start_matches('@').to_string(), content);
        } else {
            self.anonymous = content;
        }
    }

    pub fn get(&self, name: Option<&str>) -> Option<&Vec<String>> {
        if let Some(n) = name {
            self.registers.get(n.trim_start_matches('@'))
        } else {
            Some(&self.anonymous)
        }
    }
}

/// A parsed hashline patch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashlineOp {
    /// Replace line range N..=M with body lines.
    PutRange { start: usize, end: usize, body: Vec<String> },
    /// Replace syntactic/indent block starting at line N with body lines.
    PutBlock { start: usize, body: Vec<String> },
    /// Paste register at target gap or over range.
    PutFromRegister {
        start: usize,
        end: Option<usize>,
        register: Option<String>,
        before: bool,
    },
    /// Insert body lines before line N (1-based; <1 is file head).
    InsertBefore { line: usize, body: Vec<String> },
    /// Insert body lines after line N (1-based; >$ is file tail).
    InsertAfter { line: usize, body: Vec<String> },
    /// Append body lines to file tail.
    AppendTail { body: Vec<String> },
    /// Delete inclusive lines N..=M, optionally storing into a register.
    CutRange { start: usize, end: usize, register: Option<String> },
    /// Delete syntactic/indent block starting at line N, optionally storing into a register.
    CutBlock { start: usize, register: Option<String> },
    /// Remove/delete file entirely.
    RemoveFile,
    /// Move/rename file to destination.
    MoveFile { dest: PathBuf },
}

impl HashlineOp {
    pub fn start_line(&self) -> usize {
        match self {
            HashlineOp::PutRange { start, .. } => *start,
            HashlineOp::PutBlock { start, .. } => *start,
            HashlineOp::PutFromRegister { start, .. } => *start,
            HashlineOp::InsertBefore { line, .. } => *line,
            HashlineOp::InsertAfter { line, .. } => *line,
            HashlineOp::AppendTail { .. } => usize::MAX,
            HashlineOp::CutRange { start, .. } => *start,
            HashlineOp::CutBlock { start, .. } => *start,
            HashlineOp::RemoveFile => 0,
            HashlineOp::MoveFile { .. } => 0,
        }
    }
}

/// A parsed section of a hashline patch script targeting a specific file.
#[derive(Debug, Clone)]
pub struct HashlineSection {
    pub path: PathBuf,
    pub expected_tag: String,
    pub ops: Vec<HashlineOp>,
}

/// Resolve end of a syntactic/indent block starting at 1-based start line.
pub fn resolve_block_end(lines: &[String], start_1based: usize) -> usize {
    if start_1based == 0 || start_1based > lines.len() {
        return start_1based;
    }
    let start_idx = start_1based - 1;
    let start_line = &lines[start_idx];
    let start_indent = start_line.chars().take_while(|c| c.is_whitespace()).count();

    // Check if line contains opening brace / bracket
    let mut brace_count = 0i32;
    for ch in start_line.chars() {
        if ch == '{' || ch == '(' || ch == '[' {
            brace_count += 1;
        } else if ch == '}' || ch == ')' || ch == ']' {
            brace_count -= 1;
        }
    }

    if brace_count > 0 {
        // Find matching closing brace
        let mut curr_idx = start_idx + 1;
        while curr_idx < lines.len() {
            let l = &lines[curr_idx];
            for ch in l.chars() {
                if ch == '{' || ch == '(' || ch == '[' {
                    brace_count += 1;
                } else if ch == '}' || ch == ')' || ch == ']' {
                    brace_count -= 1;
                }
            }
            if brace_count <= 0 {
                return curr_idx + 1;
            }
            curr_idx += 1;
        }
        return lines.len();
    }

    // Indent-based block resolution (Python, YAML, Markdown headings, comments)
    let is_markdown_heading = start_line.trim_start().starts_with('#');
    let mut curr_idx = start_idx + 1;
    while curr_idx < lines.len() {
        let l = &lines[curr_idx];
        if l.trim().is_empty() {
            curr_idx += 1;
            continue;
        }
        if is_markdown_heading && l.trim_start().starts_with('#') {
            return curr_idx;
        }
        let indent = l.chars().take_while(|c| c.is_whitespace()).count();
        if indent <= start_indent {
            return curr_idx;
        }
        curr_idx += 1;
    }

    lines.len()
}

/// Parse a full multi-file hashline patch script.
pub fn parse_hashline_patch(input: &str) -> PrResult<Vec<HashlineSection>> {
    let mut sections = Vec::new();
    let mut current_section: Option<HashlineSection> = None;
    let mut current_op: Option<HashlineOp> = None;

    let lines: Vec<&str> = input.lines().collect();
    let mut idx = 0;

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();

        // Check for file section header: [path#TAG]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(op) = current_op.take() {
                if let Some(sec) = current_section.as_mut() {
                    sec.ops.push(op);
                }
            }
            if let Some(sec) = current_section.take() {
                sections.push(sec);
            }

            let inner = &trimmed[1..trimmed.len() - 1];
            let parts: Vec<&str> = inner.rsplitn(2, '#').collect();
            if parts.len() != 2 {
                return Err(PrError::Tool(format!(
                    "Invalid section header '{}': expected [path#TAG] format with 4-hex tag",
                    trimmed
                )));
            }
            let expected_tag = parts[0].to_uppercase();
            let path_str = parts[1];

            current_section = Some(HashlineSection {
                path: PathBuf::from(path_str),
                expected_tag,
                ops: Vec::new(),
            });
            idx += 1;
            continue;
        }

        // Must be inside a section
        if current_section.is_none() {
            if trimmed.is_empty() {
                idx += 1;
                continue;
            }
            return Err(PrError::Tool(format!(
                "Found content before first [path#TAG] header: '{}'",
                line
            )));
        }

        // If line is a body row (starts with '+')
        if line.starts_with('+') {
            let body_text = &line[1..];
            match current_op.as_mut() {
                Some(HashlineOp::PutRange { body, .. })
                | Some(HashlineOp::PutBlock { body, .. })
                | Some(HashlineOp::InsertBefore { body, .. })
                | Some(HashlineOp::InsertAfter { body, .. })
                | Some(HashlineOp::AppendTail { body }) => {
                    body.push(body_text.to_string());
                }
                _ => {
                    return Err(PrError::Tool(format!(
                        "Unexpected body row '+' without an active ':' header at line {}: '{}'",
                        idx + 1,
                        line
                    )));
                }
            }
            idx += 1;
            continue;
        }

        // If we reach a non-body row, commit any prior pending op
        if let Some(op) = current_op.take() {
            if let Some(sec) = current_section.as_mut() {
                sec.ops.push(op);
            }
        }

        if trimmed.is_empty() {
            idx += 1;
            continue;
        }

        // Parse operation headers
        if trimmed == "REM" {
            current_op = Some(HashlineOp::RemoveFile);
        } else if let Some(dest) = trimmed.strip_prefix("MV ") {
            current_op = Some(HashlineOp::MoveFile {
                dest: PathBuf::from(dest.trim().trim_matches('"')),
            });
        } else if let Some(cut_spec) = trimmed.strip_prefix("CUT ") {
            let cut_spec = cut_spec.trim();
            // Check for register capture: CUT 5.=9 @fn or CUT 1* @fn
            let (spec_part, reg_part) = if let Some((s, r)) = cut_spec.split_once('@') {
                (s.trim(), Some(r.trim().to_string()))
            } else {
                (cut_spec, None)
            };

            if let Some((start_s, end_s)) = spec_part.split_once(".=") {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid start line in CUT op: '{}'", cut_spec))
                })?;
                let end = end_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid end line in CUT op: '{}'", cut_spec))
                })?;
                current_op = Some(HashlineOp::CutRange { start, end, register: reg_part });
            } else if let Some(start_s) = spec_part.strip_suffix('*') {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid start line in CUT block op: '{}'", cut_spec))
                })?;
                current_op = Some(HashlineOp::CutBlock { start, register: reg_part });
            } else if let Ok(single) = spec_part.parse::<usize>() {
                current_op = Some(HashlineOp::CutRange { start: single, end: single, register: reg_part });
            } else {
                return Err(PrError::Tool(format!("Unsupported CUT syntax: '{}'", trimmed)));
            }
        } else if let Some(put_spec) = trimmed.strip_prefix("PUT ") {
            let is_header = put_spec.ends_with(':');
            let spec = if is_header {
                &put_spec[..put_spec.len() - 1].trim()
            } else {
                put_spec.trim()
            };

            // Check for register paste: PUT >40 @name, PUT <1 @name, PUT 1.=3 @name
            if let Some((target_spec, reg_name)) = spec.split_once('@') {
                let target_spec = target_spec.trim();
                let register = Some(reg_name.trim().to_string());

                if let Some(target) = target_spec.strip_prefix('<') {
                    let line_num = target.parse::<usize>().unwrap_or(1);
                    current_op = Some(HashlineOp::PutFromRegister {
                        start: line_num,
                        end: None,
                        register,
                        before: true,
                    });
                } else if let Some(target) = target_spec.strip_prefix('>') {
                    let line_num = if target == "$" { usize::MAX } else { target.parse::<usize>().unwrap_or(1) };
                    current_op = Some(HashlineOp::PutFromRegister {
                        start: line_num,
                        end: None,
                        register,
                        before: false,
                    });
                } else if let Some((start_s, end_s)) = target_spec.split_once(".=") {
                    let start = start_s.parse::<usize>().unwrap_or(1);
                    let end = end_s.parse::<usize>().unwrap_or(start);
                    current_op = Some(HashlineOp::PutFromRegister {
                        start,
                        end: Some(end),
                        register,
                        before: true,
                    });
                } else if let Some(start_s) = target_spec.strip_suffix('*') {
                    let start = start_s.parse::<usize>().unwrap_or(1);
                    current_op = Some(HashlineOp::PutFromRegister {
                        start,
                        end: None,
                        register,
                        before: true,
                    });
                } else {
                    let line = target_spec.parse::<usize>().unwrap_or(1);
                    current_op = Some(HashlineOp::PutFromRegister {
                        start: line,
                        end: Some(line),
                        register,
                        before: true,
                    });
                }
            } else if let Some((start_s, end_s)) = spec.split_once(".=") {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid start line in PUT range op: '{}'", spec))
                })?;
                let end = end_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid end line in PUT range op: '{}'", spec))
                })?;
                current_op = Some(HashlineOp::PutRange {
                    start,
                    end,
                    body: Vec::new(),
                });
            } else if let Some(target) = spec.strip_prefix('<') {
                let line_num = target.parse::<usize>().unwrap_or(1);
                current_op = Some(HashlineOp::InsertBefore {
                    line: line_num,
                    body: Vec::new(),
                });
            } else if let Some(target) = spec.strip_prefix('>') {
                if target == "$" {
                    current_op = Some(HashlineOp::AppendTail { body: Vec::new() });
                } else if let Some(start_s) = target.strip_suffix('*') {
                    let line_num = start_s.parse::<usize>().map_err(|_| {
                        PrError::Tool(format!("Invalid block start in PUT >N* op: '{}'", target))
                    })?;
                    current_op = Some(HashlineOp::InsertAfter {
                        line: line_num,
                        body: Vec::new(),
                    });
                } else {
                    let line_num = target.parse::<usize>().map_err(|_| {
                        PrError::Tool(format!("Invalid line in PUT >N op: '{}'", target))
                    })?;
                    current_op = Some(HashlineOp::InsertAfter {
                        line: line_num,
                        body: Vec::new(),
                    });
                }
            } else if let Some(start_s) = spec.strip_suffix('*') {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid line in PUT N* op: '{}'", spec))
                })?;
                current_op = Some(HashlineOp::PutBlock {
                    start,
                    body: Vec::new(),
                });
            } else {
                return Err(PrError::Tool(format!("Unsupported PUT syntax: '{}'", trimmed)));
            }
        } else {
            return Err(PrError::Tool(format!(
                "Unrecognized operation line {}: '{}'",
                idx + 1,
                trimmed
            )));
        }

        idx += 1;
    }

    if let Some(op) = current_op.take() {
        if let Some(sec) = current_section.as_mut() {
            sec.ops.push(op);
        }
    }
    if let Some(sec) = current_section.take() {
        sections.push(sec);
    }

    Ok(sections)
}

/// Apply hashline patch operations to an existing file's text content with register bank support.
/// Operations are applied bottom-up (descending line order) to prevent coordinate drift.
pub fn apply_hashline_to_content(
    original_content: &str,
    expected_tag: &str,
    ops: &[HashlineOp],
    register_bank: &mut RegisterBank,
) -> PrResult<(String, String)> {
    let current_tag = compute_tag(original_content);
    if !expected_tag.is_empty() && !expected_tag.eq_ignore_ascii_case(&current_tag) {
        return Err(PrError::Tool(format!(
            "STALE TAG REJECTED: Expected snapshot tag '#{}', but target file tag is '#{}'. File has changed since last read. Re-read target lines to get latest #TAG and line numbers.",
            expected_tag, current_tag
        )));
    }

    let mut lines: Vec<String> = original_content.lines().map(|s| s.to_string()).collect();

    // First pass: Resolve block ends and collect cuts into register bank
    for op in ops {
        match op {
            HashlineOp::CutRange { start, end, register } => {
                let s = (*start).saturating_sub(1);
                let e = (*end).min(lines.len());
                if s < lines.len() {
                    let actual_end = e.max(s);
                    let cut_lines = lines[s..actual_end].to_vec();
                    register_bank.set(register.as_deref(), cut_lines);
                }
            }
            HashlineOp::CutBlock { start, register } => {
                let block_end = resolve_block_end(&lines, *start);
                let s = (*start).saturating_sub(1);
                let e = block_end.min(lines.len());
                if s < lines.len() {
                    let actual_end = e.max(s);
                    let cut_lines = lines[s..actual_end].to_vec();
                    register_bank.set(register.as_deref(), cut_lines);
                }
            }
            _ => {}
        }
    }

    // Sort operations bottom-up (by descending start_line) to ensure zero line-shift corruption
    let mut sorted_ops = ops.to_vec();
    sorted_ops.sort_by(|a, b| b.start_line().cmp(&a.start_line()));

    for op in &sorted_ops {
        match op {
            HashlineOp::PutRange { start, end, body } => {
                let s = (*start).saturating_sub(1);
                let e = (*end).min(lines.len());
                if s > lines.len() {
                    return Err(PrError::Tool(format!(
                        "PUT range start {} exceeds line count {}",
                        start,
                        lines.len()
                    )));
                }
                let actual_end = e.max(s);
                lines.splice(s..actual_end, body.clone());
            }
            HashlineOp::PutBlock { start, body } => {
                let block_end = resolve_block_end(&lines, *start);
                let s = (*start).saturating_sub(1);
                let e = block_end.min(lines.len());
                let actual_end = e.max(s);
                lines.splice(s..actual_end, body.clone());
            }
            HashlineOp::PutFromRegister { start, end, register, before } => {
                let reg_content = register_bank
                    .get(register.as_deref())
                    .cloned()
                    .unwrap_or_default();
                if let Some(e_line) = end {
                    let s = (*start).saturating_sub(1);
                    let e = (*e_line).min(lines.len());
                    let actual_end = e.max(s);
                    lines.splice(s..actual_end, reg_content);
                } else if *before {
                    let pos = if *start <= 1 { 0 } else { (start - 1).min(lines.len()) };
                    for (offset, item) in reg_content.iter().enumerate() {
                        lines.insert(pos + offset, item.clone());
                    }
                } else {
                    let pos = if *start == usize::MAX { lines.len() } else { (*start).min(lines.len()) };
                    for (offset, item) in reg_content.iter().enumerate() {
                        lines.insert(pos + offset, item.clone());
                    }
                }
            }
            HashlineOp::InsertBefore { line, body } => {
                let pos = if *line <= 1 { 0 } else { (line - 1).min(lines.len()) };
                for (offset, item) in body.iter().enumerate() {
                    lines.insert(pos + offset, item.clone());
                }
            }
            HashlineOp::InsertAfter { line, body } => {
                let pos = (*line).min(lines.len());
                for (offset, item) in body.iter().enumerate() {
                    lines.insert(pos + offset, item.clone());
                }
            }
            HashlineOp::AppendTail { body } => {
                lines.extend(body.clone());
            }
            HashlineOp::CutRange { start, end, .. } => {
                let s = (*start).saturating_sub(1);
                let e = (*end).min(lines.len());
                if s < lines.len() {
                    let actual_end = e.max(s);
                    lines.drain(s..actual_end);
                }
            }
            HashlineOp::CutBlock { start, .. } => {
                let block_end = resolve_block_end(&lines, *start);
                let s = (*start).saturating_sub(1);
                let e = block_end.min(lines.len());
                if s < lines.len() {
                    let actual_end = e.max(s);
                    lines.drain(s..actual_end);
                }
            }
            HashlineOp::RemoveFile | HashlineOp::MoveFile { .. } => {}
        }
    }

    let mut result = lines.join("\n");
    if original_content.ends_with('\n') || result.is_empty() {
        result.push('\n');
    }
    let new_tag = compute_tag(&result);
    Ok((result, new_tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_tag_deterministic() {
        let content1 = "fn main() {\n    println!(\"hello\");\n}\n";
        let tag1 = compute_tag(content1);
        let tag2 = compute_tag(content1);
        assert_eq!(tag1, tag2);
        assert_eq!(tag1.len(), 4);

        let content2 = "fn main() {\n    println!(\"world\");\n}\n";
        let tag3 = compute_tag(content2);
        assert_ne!(tag1, tag3);
    }

    #[test]
    fn test_bottom_up_sorting_prevents_index_shift() {
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let tag = compute_tag(content);
        let mut reg_bank = RegisterBank::new();

        // Sequential operations: op 1 inserts 3 lines at line 1, op 2 modifies line 3
        let patch = format!(
            "[test.txt#{}]\nPUT 1.=1:\n+line 1a\n+line 1b\n+line 1c\nPUT 3.=3:\n+line 3 modified\n",
            tag
        );
        let sections = parse_hashline_patch(&patch).unwrap();
        let (new_content, _) = apply_hashline_to_content(content, &tag, &sections[0].ops, &mut reg_bank).unwrap();
        
        let lines: Vec<&str> = new_content.lines().collect();
        assert_eq!(lines[0], "line 1a");
        assert_eq!(lines[1], "line 1b");
        assert_eq!(lines[2], "line 1c");
        assert_eq!(lines[3], "line 2");
        assert_eq!(lines[4], "line 3 modified");
        assert_eq!(lines[5], "line 4");
        assert_eq!(lines[6], "line 5");
    }

    #[test]
    fn test_named_register_cut_and_paste() {
        let content = "fn greet() {\n    println!(\"hello\");\n}\n\nfn run() {\n}\n";
        let tag = compute_tag(content);
        let mut reg_bank = RegisterBank::new();

        let patch = format!(
            "[test.txt#{}]\nCUT 1.=3 @greet_fn\nPUT >5 @greet_fn\n",
            tag
        );
        let sections = parse_hashline_patch(&patch).unwrap();
        let (new_content, _) = apply_hashline_to_content(content, &tag, &sections[0].ops, &mut reg_bank).unwrap();
        assert!(reg_bank.get(Some("greet_fn")).is_some());
        assert!(!new_content.starts_with("fn greet"));
    }

    #[test]
    fn test_stale_tag_rejected() {
        let content = "line 1\nline 2\n";
        let mut reg_bank = RegisterBank::new();
        let patch = "[test.txt#DEAD]\nPUT 1.=1:\n+new line 1\n";
        let sections = parse_hashline_patch(patch).unwrap();
        let err = apply_hashline_to_content(content, "DEAD", &sections[0].ops, &mut reg_bank).unwrap_err();
        assert!(err.to_string().contains("STALE TAG REJECTED"));
    }
}
