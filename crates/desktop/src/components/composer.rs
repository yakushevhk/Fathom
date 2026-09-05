//! Composer component for turn dispatch, mid-turn steering, and quick actions.

use crate::state::{AppState, ChatMessage};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct Composer {
    state: Arc<AppState>,
    input_text: String,
}

impl Composer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            input_text: String::new(),
        }
    }
}

impl Render for Composer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .border_t_1()
            .border_color(Theme::border_subtle())
            .bg(Theme::bg_surface())
            .p_3()
            .gap_2()
            // Quick Command Chips (@bot, /skill, /routine)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .text_color(Theme::text_muted())
                            .child("Quick Triggers:"),
                    )
                    .child(self.render_chip("@GeneralAssistant", "🤖 SDR Research", cx))
                    .child(self.render_chip("@RiskAnalyst", "🛡️ Compliance Check", cx))
                    .child(self.render_chip("/browser", "🌐 Launch Computer", cx))
                    .child(self.render_chip("/routine", "⚡ Standing Instruction", cx)),
            )
            // Composer main box & action buttons
            .child(
                div()
                    .flex()
                    .items_center()
                    .p_2()
                    .rounded_lg()
                    .bg(Theme::bg_elevated())
                    .border_1()
                    .border_color(Theme::border_subtle())
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(Theme::text_primary())
                            .child(
                                if self.input_text.is_empty() {
                                    div()
                                        .text_color(Theme::text_muted())
                                        .child("Hand work to coworker or direct browser computer (e.g. 'Research competitor pricing on G2 and compile spreadsheet')...")
                                } else {
                                    div().child(self.input_text.clone())
                                },
                            ),
                    )
                    // Action Buttons (Send / Steer / Stop)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("composer-send-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::accent_purple())
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_primary())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::accent_blue()))
                                    .child("Send Turn ↵")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let text = if this.input_text.is_empty() {
                                            "Investigate competitors and update knowledge base".to_string()
                                        } else {
                                            this.input_text.clone()
                                        };

                                        // Dispatch user turn
                                        this.state.add_message(ChatMessage {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            role: "user".to_string(),
                                            content: text.clone(),
                                            thinking: None,
                                            tool_name: None,
                                            tool_status: None,
                                            tool_input: None,
                                            tool_output: None,
                                            question: None,
                                            request_id: None,
                                            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                            expanded: false,
                                        });

                                        // Mock assistant reasoning & computer tool execution
                                        this.state.add_message(ChatMessage {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            role: "assistant".to_string(),
                                            content: "Executing task. Navigating browser to target domain and inspecting DOM accessibility tree.".to_string(),
                                            thinking: Some("1. Initialize Playwright sandbox.\n2. Verify policy gate against rule: AllowNavDomain\n3. Execute computer_navigate(\"https://news.ycombinator.com\")".to_string()),
                                            tool_name: Some("computer_navigate".to_string()),
                                            tool_status: Some("completed".to_string()),
                                            tool_input: Some(serde_json::json!({
                                                "url": "https://news.ycombinator.com",
                                                "intent": "Browse target research page"
                                            })),
                                            tool_output: Some(serde_json::json!({
                                                "status": 200,
                                                "title": "Hacker News",
                                                "elements_observed": 30
                                            })),
                                            question: None,
                                            request_id: None,
                                            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                            expanded: false,
                                        });

                                        this.input_text.clear();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("composer-steer-btn")
                                    .px_2p5()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .text_color(Theme::text_secondary())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                                    .child("Steer [s]")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        this.state.add_message(ChatMessage {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            role: "system".to_string(),
                                            content: "Mid-run steer instruction injected: Prioritize open-source repositories over proprietary vendors.".to_string(),
                                            thinking: Some("Updating DAG priority queues based on operator steer.".to_string()),
                                            tool_name: None,
                                            tool_status: None,
                                            tool_input: None,
                                            tool_output: None,
                                            question: None,
                                            request_id: None,
                                            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                            expanded: false,
                                        });
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }
}

impl Composer {
    fn render_chip(&self, label: &'static str, title: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let label_str = label.to_string();
        div()
            .id(SharedString::from(format!("chip-{}", label)))
            .px_2()
            .py_0p5()
            .rounded_md()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(Theme::border_subtle())
            .text_xs()
            .text_color(Theme::text_secondary())
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
            .child(format!("{} · {}", label, title))
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.input_text = format!("{} ", label_str);
                cx.notify();
            }))
    }
}
