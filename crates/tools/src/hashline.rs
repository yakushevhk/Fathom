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

/// A parsed hashline patch operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashlineOp {
    /// Replace line range N..=M with body lines.
    PutRange { start: usize, end: usize, body: Vec<String> },
    /// Insert body lines before line N (1-based; <1 is file head).
    InsertBefore { line: usize, body: Vec<String> },
    /// Insert body lines after line N (1-based; >$ is file tail).
    InsertAfter { line: usize, body: Vec<String> },
    /// Append body lines to file tail.
    AppendTail { body: Vec<String> },
    /// Delete inclusive lines N..=M.
    CutRange { start: usize, end: usize },
    /// Remove/delete file entirely.
    RemoveFile,
    /// Move/rename file to destination.
    MoveFile { dest: PathBuf },
}

/// A parsed section of a hashline patch script targeting a specific file.
#[derive(Debug, Clone)]
pub struct HashlineSection {
    pub path: PathBuf,
    pub expected_tag: String,
    pub ops: Vec<HashlineOp>,
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
            if let Some((start_s, end_s)) = cut_spec.split_once(".=") {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid start line in CUT op: '{}'", cut_spec))
                })?;
                let end = end_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid end line in CUT op: '{}'", cut_spec))
                })?;
                current_op = Some(HashlineOp::CutRange { start, end });
            } else if let Some(start_s) = cut_spec.strip_suffix('*') {
                let start = start_s.parse::<usize>().map_err(|_| {
                    PrError::Tool(format!("Invalid start line in CUT block op: '{}'", cut_spec))
                })?;
                current_op = Some(HashlineOp::CutRange { start, end: start });
            } else if let Ok(single) = cut_spec.parse::<usize>() {
                current_op = Some(HashlineOp::CutRange { start: single, end: single });
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

            if let Some((start_s, end_s)) = spec.split_once(".=") {
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
                current_op = Some(HashlineOp::PutRange {
                    start,
                    end: start,
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

/// Apply hashline patch operations to an existing file's text content.
pub fn apply_hashline_to_content(
    original_content: &str,
    expected_tag: &str,
    ops: &[HashlineOp],
) -> PrResult<(String, String)> {
    let current_tag = compute_tag(original_content);
    if !expected_tag.is_empty() && !expected_tag.eq_ignore_ascii_case(&current_tag) {
        return Err(PrError::Tool(format!(
            "STALE TAG REJECTED: Expected snapshot tag '#{}', but target file tag is '#{}'. File has changed since last read. Re-read target lines to get latest #TAG and line numbers.",
            expected_tag, current_tag
        )));
    }

    let mut lines: Vec<String> = original_content.lines().map(|s| s.to_string()).collect();
    if original_content.ends_with('\n') && lines.is_empty() {
        // empty file with single newline
    }

    for op in ops {
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
            HashlineOp::CutRange { start, end } => {
                let s = (*start).saturating_sub(1);
                let e = (*end).min(lines.len());
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
    fn test_apply_put_range() {
        let content = "line 1\nline 2\nline 3\n";
        let tag = compute_tag(content);

        let patch = format!(
            "[test.txt#{}]\nPUT 2.=2:\n+line 2 modified\n",
            tag
        );
        let sections = parse_hashline_patch(&patch).unwrap();
        assert_eq!(sections.len(), 1);

        let (new_content, new_tag) = apply_hashline_to_content(content, &tag, &sections[0].ops).unwrap();
        assert_eq!(new_content, "line 1\nline 2 modified\nline 3\n");
        assert_ne!(new_tag, tag);
    }

    #[test]
    fn test_stale_tag_rejected() {
        let content = "line 1\nline 2\n";
        let patch = "[test.txt#DEAD]\nPUT 1.=1:\n+new line 1\n";
        let sections = parse_hashline_patch(patch).unwrap();
        let err = apply_hashline_to_content(content, "DEAD", &sections[0].ops).unwrap_err();
        assert!(err.to_string().contains("STALE TAG REJECTED"));
    }

    #[test]
    fn test_insert_before_and_after() {
        let content = "second\n";
        let tag = compute_tag(content);
        let ops = vec![
            HashlineOp::InsertBefore {
                line: 1,
                body: vec!["first".into()],
            },
            HashlineOp::InsertAfter {
                line: 2,
                body: vec!["third".into()],
            },
        ];
        let (res, _) = apply_hashline_to_content(content, &tag, &ops).unwrap();
        assert_eq!(res, "first\nsecond\nthird\n");
    }
}
