use serde::{Deserialize, Serialize};

/// OpenAI-compatible message format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System { content: String },
    
    #[serde(rename = "user")]
    User { content: String },
    
    #[serde(rename = "assistant")]
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
    },
    
    #[serde(rename = "tool")]
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self::System { content: content.into() }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::User { content: content.into() }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: Some(content.into()),
            tool_calls: vec![],
        }
    }

    pub fn assistant_with_tools(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self::Assistant { content, tool_calls }
    }

    pub fn tool(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Tool {
            tool_call_id: call_id.into(),
            content: content.into(),
        }
    }
}

/// OpenAI-compatible tool call format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,  // always "function"
    pub function: ToolCallFunction,
}

impl ToolCall {
    pub fn new(id: impl Into<String>, name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            call_type: "function".to_string(),
            function: ToolCallFunction {
                name: name.into(),
                arguments: serde_json::to_string(&arguments).unwrap_or_default(),
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.function.name
    }

    pub fn arguments(&self) -> serde_json::Value {
        serde_json::from_str(&self.function.arguments).unwrap_or(serde_json::json!({}))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    /// JSON string of arguments (OpenAI format)
    pub arguments: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::user("hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_tool_call_format() {
        let tc = ToolCall::new("call_1", "web_search", serde_json::json!({"query": "test"}));
        assert_eq!(tc.name(), "web_search");
        assert_eq!(tc.arguments()["query"], "test");
        
        let json = serde_json::to_string(&tc).unwrap();
        assert!(json.contains("\"type\":\"function\""));
    }

    #[test]
    fn test_assistant_with_tools() {
        let tc = ToolCall::new("call_1", "shell", serde_json::json!({"command": "ls"}));
        let msg = Message::assistant_with_tools(Some("thinking".to_string()), vec![tc]);
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("tool_calls"));
    }
}
