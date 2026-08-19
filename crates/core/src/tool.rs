use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Machine-readable failure class (`rate_limited`, `timeout`, `blocked`,
    /// `not_found`, `network`, `parse`, `other`). Lets callers make
    /// retry-vs-skip decisions without parsing prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl ToolOutput {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            success: true,
            content: content.into(),
            metadata: None,
            error_code: None,
        }
    }

    pub fn ok_with_meta(content: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self {
            success: true,
            content: content.into(),
            metadata: Some(metadata),
            error_code: None,
        }
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self {
            success: false,
            content: content.into(),
            metadata: None,
            error_code: None,
        }
    }

    /// Error with a machine-readable class.
    pub fn err_code(content: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            success: false,
            content: content.into(),
            metadata: None,
            error_code: Some(code.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_constructs_success() {
        let out = ToolOutput::ok("done");
        assert!(out.success);
        assert_eq!(out.content, "done");
        assert!(out.metadata.is_none());
        assert!(out.error_code.is_none());
    }

    #[test]
    fn ok_with_meta_includes_metadata() {
        let meta = serde_json::json!({"key": 42});
        let out = ToolOutput::ok_with_meta("result", meta.clone());
        assert!(out.success);
        assert_eq!(out.content, "result");
        assert_eq!(out.metadata, Some(meta));
        assert!(out.error_code.is_none());
    }

    #[test]
    fn err_constructs_failure() {
        let out = ToolOutput::err("boom");
        assert!(!out.success);
        assert_eq!(out.content, "boom");
        assert!(out.metadata.is_none());
        assert!(out.error_code.is_none());
    }

    #[test]
    fn err_code_sets_error_code() {
        let out = ToolOutput::err_code("rate limited", "rate_limited");
        assert!(!out.success);
        assert_eq!(out.content, "rate limited");
        assert_eq!(out.error_code.as_deref(), Some("rate_limited"));
    }

    #[test]
    fn ok_accepts_string() {
        let out = ToolOutput::ok(String::from("owned"));
        assert_eq!(out.content, "owned");
    }

    #[test]
    fn serde_roundtrip() {
        let out = ToolOutput::ok_with_meta("data", serde_json::json!({"n": 1}));
        let json = serde_json::to_string(&out).unwrap();
        let back: ToolOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.content, "data");
        assert!(back.success);
        assert_eq!(back.metadata, Some(serde_json::json!({"n": 1})));
    }

    #[test]
    fn serde_err_roundtrip() {
        let out = ToolOutput::err_code("not found", "not_found");
        let json = serde_json::to_string(&out).unwrap();
        let back: ToolOutput = serde_json::from_str(&json).unwrap();
        assert!(!back.success);
        assert_eq!(back.error_code.as_deref(), Some("not_found"));
    }

    #[test]
    fn tool_schema_serde() {
        let schema = ToolSchema {
            name: "test_tool".into(),
            description: "A test tool".into(),
            parameters: serde_json::json!({"type": "object"}),
        };
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("test_tool"));
        let back: ToolSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test_tool");
    }
}
