use serde::{Deserialize, Serialize};

/// High-efficiency Accessibility Object Model (AOM) distillation representation.
/// Prunes invisible DOM elements and assigns short [ref=eN] selectors to interactive nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AomNode {
    pub id: String, // e.g. "e1", "e2"
    pub role: String, // "button", "link", "textbox", "checkbox", "heading"
    pub name: String, // accessible label / inner text
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub checked: Option<bool>,
    pub children: Vec<AomNode>,
}

/// Distill a raw HTML / DOM string into a token-compressed interactive AOM YAML outline.
pub fn distill_dom_to_aom_outline(raw_html: &str) -> String {
    let mut lines = Vec::new();
    let mut ref_counter = 1;

    // Simple heuristic parser extracting interactive HTML elements
    for line in raw_html.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.contains("<button") {
            let label = extract_tag_text(trimmed, "button");
            lines.push(format!("  - [ref=e{}] button \"{}\"", ref_counter, label));
            ref_counter += 1;
        } else if trimmed.contains("<a ") || trimmed.starts_with("<a>") {
            let label = extract_tag_text(trimmed, "a");
            if !label.is_empty() {
                lines.push(format!("  - [ref=e{}] link \"{}\"", ref_counter, label));
                ref_counter += 1;
            }
        } else if trimmed.contains("<input") {
            let input_type = extract_attr(trimmed, "type").unwrap_or_else(|| "text".to_string());
            let placeholder = extract_attr(trimmed, "placeholder").or_else(|| extract_attr(trimmed, "name")).unwrap_or_default();
            lines.push(format!("  - [ref=e{}] input:{} \"{}\"", ref_counter, input_type, placeholder));
            ref_counter += 1;
        } else if trimmed.contains("<textarea") {
            let placeholder = extract_attr(trimmed, "placeholder").unwrap_or_default();
            lines.push(format!("  - [ref=e{}] textarea \"{}\"", ref_counter, placeholder));
            ref_counter += 1;
        } else if trimmed.contains("<select") {
            let name = extract_attr(trimmed, "name").unwrap_or_default();
            lines.push(format!("  - [ref=e{}] select \"{}\"", ref_counter, name));
            ref_counter += 1;
        }
    }

    if lines.is_empty() {
        "AOM: (no interactive elements detected)".to_string()
    } else {
        format!("AOM Accessibility Snapshot (Interactive Elements):\n{}", lines.join("\n"))
    }
}

fn extract_tag_text(line: &str, tag: &str) -> String {
    if let Some(start) = line.find('>') {
        let close_tag = format!("</{}>", tag);
        if let Some(end) = line.find(&close_tag) {
            if end > start {
                return line[start + 1..end].trim().to_string();
            }
        }
    }
    String::new()
}

fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(pos) = line.find(&pattern) {
        let start = pos + pattern.len();
        if let Some(end) = line[start..].find('"') {
            return Some(line[start..start + end].to_string());
        }
    }
    None
}
