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

                                        // Add user message to transcript immediately
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

                                        let api = this.state.api.clone();
                                        let state_clone = this.state.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            if let Ok(res) = api.create_session(&text).await {
                                                *state_clone.active_session_id.write() = Some(res.id);
                                            }
                                        }).detach();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("composer-steer-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .text_color(Theme::text_secondary())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated()).text_color(Theme::text_primary()))
                                    .child("Steer [s]")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let text = if this.input_text.is_empty() {
                                            "Focus only on public pricing tables".to_string()
                                        } else {
                                            this.input_text.clone()
                                        };
                                        let session_id = this.state.active_session_id.read().clone();
                                        let api = this.state.api.clone();
                                        if let Some(id) = session_id {
                                            cx.spawn(async move |_this, _cx| {
                                                let _ = api.steer_session(&id, &text).await;
                                            }).detach();
                                        }
                                        cx.notify();
                                    })),
                            )
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
