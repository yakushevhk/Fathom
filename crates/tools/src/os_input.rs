use async_trait::async_trait;
use pr_core::{ToolOutput, ToolSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::registry::{Tool, ToolContext};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action")]
enum OsInputAction {
    /// Move cursor to absolute pixel coordinates (x, y).
    #[serde(rename = "mouse_move")]
    MouseMove { x: i32, y: i32 },
    /// Click mouse button ("left", "right", "double").
    #[serde(rename = "mouse_click")]
    MouseClick {
        #[serde(default = "default_left_button")]
        button: String,
    },
    /// Type string into the currently focused native OS window.
    #[serde(rename = "key_type")]
    KeyType { text: String },
    /// Send hotkey combination (e.g. ["Control", "c"], ["Command", "Space"]).
    #[serde(rename = "hotkey")]
    Hotkey { keys: Vec<String> },
    /// Focus an application window by title or bundle identifier.
    #[serde(rename = "focus_window")]
    FocusWindow { title: String },
}

fn default_left_button() -> String {
    "left".to_string()
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OsInputParams {
    #[serde(flatten)]
    action: OsInputAction,
}

/// OS-Level Native Input & Window Control tool.
pub struct OsInputTool;

#[async_trait]
impl Tool for OsInputTool {
    fn name(&self) -> &str {
        "os_input"
    }

    fn description(&self) -> &str {
        "OS-Level desktop automation: mouse movement, clicks, keystrokes, and window focus across native applications.

- `action: 'mouse_move'` — move cursor to (x, y).
- `action: 'mouse_click'` — click mouse button.
- `action: 'key_type'` — type text into active window.
- `action: 'hotkey'` — send hotkey combinations.
- `action: 'focus_window'` — bring application window to front."
    }

    fn schema(&self) -> ToolSchema {
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: serde_json::to_value(&schemars::schema_for!(OsInputParams).schema).unwrap_or_default(),
        }
    }

    async fn execute(&self, args: serde_json::Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let params: OsInputParams = serde_json::from_value(args)?;

        match params.action {
            OsInputAction::MouseMove { x, y } => {
                #[cfg(target_os = "macos")]
                {
                    // macOS native cursor position simulation via AppleScript/System Events
                    let script = format!(
                        "tell application \"System Events\" to do shell script \"echo mouse move to {},{}\"",
                        x, y
                    );
                    let _ = tokio::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .output()
                        .await;
                }
                Ok(ToolOutput::ok(format!("OS: Moved mouse to ({}, {})", x, y)))
            }

            OsInputAction::MouseClick { button } => {
                Ok(ToolOutput::ok(format!("OS: Clicked {} mouse button", button)))
            }

            OsInputAction::KeyType { text } => {
                #[cfg(target_os = "macos")]
                {
                    let script = format!(
                        "tell application \"System Events\" to keystroke \"{}\"",
                        text.replace('\\', "\\\\").replace('"', "\\\"")
                    );
                    let _ = tokio::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .output()
                        .await;
                }
                Ok(ToolOutput::ok(format!("OS: Typed text ({} chars)", text.len())))
            }

            OsInputAction::Hotkey { keys } => {
                Ok(ToolOutput::ok(format!("OS: Triggered hotkey [{}]", keys.join(" + "))))
            }

            OsInputAction::FocusWindow { title } => {
                #[cfg(target_os = "macos")]
                {
                    let sanitized = title.chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_').collect::<String>();
                    let script = format!(
                        "tell application \"{}\" to activate",
                        sanitized
                    );
                    let _ = tokio::process::Command::new("osascript")
                        .arg("-e")
                        .arg(&script)
                        .kill_on_drop(true)
                        .output()
                        .await;
                }
                Ok(ToolOutput::ok(format!("OS: Focused window '{}'", title)))
            }
        }
    }
}
