//! Rich Chat component with conversation transcript, expandable tool chips,
//! thinking trace drawer, and interactive human-in-the-loop approval/question prompts.

use crate::state::{AppState, ChatMessage};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct ChatView {
    state: Arc<AppState>,
}

impl ChatView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    fn render_empty_state(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .h_full()
            .p_8()
            .gap_3()
            .child(
                div()
                    .size(px(48.0))
                    .rounded_lg()
                    .bg(Theme::bg_elevated())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xl()
                    .child("⚡"),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child("Welcome to Fathom Autonomous AI Workers"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .text_center()
                    .max_w(px(440.0))
                    .child("Equipped with their own computer, browser, workspace, file system and tools. Every action decided before execution and audited in real-time."),
            )
    }

    fn render_message(&self, msg: &ChatMessage, cx: &mut Context<Self>) -> Div {
        let is_user = msg.role == "user";
        let is_tool = msg.role == "tool" || msg.tool_name.is_some();

        if is_tool {
            self.render_tool_chip(msg, cx)
        } else if let Ok(refused) = serde_json::from_str::<crate::components::gallery::RefusedCardData>(&msg.content) {
            crate::components::gallery::render_refused_card(&refused)
        } else if let Ok(record) = serde_json::from_str::<crate::components::gallery::RecordCardData>(&msg.content) {
            crate::components::gallery::render_record_card(&record)
        } else if let Ok(metrics) = serde_json::from_str::<crate::components::gallery::MetricsCardData>(&msg.content) {
            crate::components::gallery::render_metrics_card(&metrics)
        } else if let Ok(checklist) = serde_json::from_str::<crate::components::gallery::ChecklistCardData>(&msg.content) {
            crate::components::gallery::render_checklist_card(&checklist)
        } else if let Ok(notice) = serde_json::from_str::<crate::components::gallery::NoticeCardData>(&msg.content) {
            crate::components::gallery::render_notice_card(&notice)
        } else if let Ok(barchart) = serde_json::from_str::<crate::components::gallery::BarChartData>(&msg.content) {
            crate::components::gallery::render_barchart_card(&barchart)
        } else if let Ok(progress) = serde_json::from_str::<crate::components::gallery::ProgressChartData>(&msg.content) {
            crate::components::gallery::render_progress_card(&progress)
        } else if let Ok(confirm) = serde_json::from_str::<crate::components::gallery::ConfirmActionCardData>(&msg.content) {
            crate::components::gallery::render_confirm_action_card(&confirm, cx, |req_id, approved, this, cx| {
                let api = this.state.api.clone();
                let session_id = this.state.active_session_id.read().clone().unwrap_or_else(|| "default".to_string());
                let r = req_id.to_string();
                cx.spawn(async move |_this, _cx| {
                    let _ = api.approve_tool(&session_id, &r, approved).await;
                }).detach();
                cx.notify();
            })
        } else if let Ok(comp_status) = serde_json::from_str::<crate::components::gallery::ComputerStatusCardData>(&msg.content) {
            crate::components::gallery::render_computer_status_card(&comp_status)
        } else if let Ok(choice) = serde_json::from_str::<crate::components::gallery::ChoiceCardData>(&msg.content) {
            crate::components::gallery::render_choice_card(&choice, cx, |req_id, opt_id, this, cx| {
                let api = this.state.api.clone();
                let session_id = this.state.active_session_id.read().clone().unwrap_or_else(|| "default".to_string());
                let r = req_id.to_string();
                let opt = opt_id.to_string();
                cx.spawn(async move |_this, _cx| {
                    let _ = api.answer_question(&session_id, &r, &opt).await;
                }).detach();
                cx.notify();
            })
        } else {
            let req_id = msg.request_id.clone();
            let session_id = self.state.active_session_id.read().clone();

            div()
                .flex()
                .flex_col()
                .p_3()
                .rounded_lg()
                .bg(if is_user { Theme::bg_elevated() } else { Theme::bg_surface() })
                .border_1()
                .border_color(Theme::border_subtle())
                .gap_2()
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .text_xs()
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(if is_user { Theme::accent_blue() } else { Theme::accent_purple() })
                                .child(if is_user { "You (Operator)" } else { "Fathom Coworker" }),
                        )
                        .child(
                            div()
                                .text_color(Theme::text_muted())
                                .child(msg.timestamp.clone()),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::text_primary())
                        .child(msg.content.clone()),
                )
                // Human-in-the-loop Approval & Question Prompt Buttons
                .child(
                    if let Some(r_id) = req_id {
                        let sess = session_id.unwrap_or_else(|| "default".to_string());
                        let r_id_allow = r_id.clone();
                        let r_id_deny = r_id.clone();
                        let sess_allow = sess.clone();
                        let sess_deny = sess;

                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_1()
                            .p_2()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_focus())
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::warning_yellow())
                                    .child("Approval Required:"),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("approve-btn-{}", r_id_allow)))
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(Theme::success_green())
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::bg_window())
                                    .cursor_pointer()
                                    .child("✓ Allow")
                                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                        let api = this.state.api.clone();
                                        let s = sess_allow.clone();
                                        let r = r_id_allow.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            let _ = api.approve_tool(&s, &r, true).await;
                                        }).detach();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("deny-btn-{}", r_id_deny)))
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(Theme::danger_red())
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .cursor_pointer()
                                    .child("✗ Deny")
                                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                        let api = this.state.api.clone();
                                        let s = sess_deny.clone();
                                        let r = r_id_deny.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            let _ = api.approve_tool(&s, &r, false).await;
                                        }).detach();
                                        cx.notify();
                                    })),
                            )
                    } else {
                        div()
                    },
                )
        }
    }

    fn render_tool_chip(&self, msg: &ChatMessage, cx: &mut Context<Self>) -> Div {
        let tool_name = msg.tool_name.as_deref().unwrap_or("tool_call");
        let status = msg.tool_status.as_deref().unwrap_or("completed");
        let expanded = msg.expanded;
        let msg_id = msg.id.clone();
        let target_id = msg.id.clone();

        let status_color = match status {
            "completed" => Theme::success_green(),
            "running" => Theme::accent_blue(),
            "failed" => Theme::danger_red(),
            _ => Theme::warning_yellow(),
        };

        let status_icon = match status {
            "completed" => "✓",
            "running" => "⏳",
            "failed" => "✗",
            _ => "⚙️",
        };

        div()
            .flex()
            .flex_col()
            .rounded_md()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_subtle())
            .overflow_hidden()
            .child(
                div()
                    .id(SharedString::from(format!("tool-header-{}", msg_id)))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_elevated()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(status_color)
                                    .child(status_icon),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_primary())
                                    .child(tool_name.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(if expanded { "Hide ▲" } else { "Inspect Details ▼" }),
                    )
                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                        let mut msgs = this.state.messages.write();
                        if let Some(item) = msgs.iter_mut().find(|m| m.id == target_id) {
                            item.expanded = !item.expanded;
                        }
                        cx.notify();
                    })),
            )
            .child(
                if expanded {
                    div()
                        .flex()
                        .flex_col()
                        .p_3()
                        .border_t_1()
                        .border_color(Theme::border_subtle())
                        .bg(Theme::bg_surface())
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(Theme::text_muted())
                                .child("ARGUMENTS & CONTEXT:"),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(Theme::bg_window())
                                .text_xs()
                                .font_family("JetBrains Mono")
                                .text_color(Theme::text_secondary())
                                .child(match &msg.tool_input {
                                    Some(val) => serde_json::to_string_pretty(val).unwrap_or_default(),
                                    None => "{}".to_string(),
                                }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(Theme::text_muted())
                                .child("RESULT PAYLOAD:"),
                        )
                        .child(
                            div()
                                .p_2()
                                .rounded_md()
                                .bg(Theme::bg_window())
                                .text_xs()
                                .font_family("JetBrains Mono")
                                .text_color(Theme::success_green())
                                .child(match &msg.tool_output {
                                    Some(val) => serde_json::to_string_pretty(val).unwrap_or_default(),
                                    None => msg.content.clone(),
                                }),
                        )
                } else {
                    div()
                },
            )
    }

    fn render_thinking_drawer(&self, messages: &[ChatMessage]) -> Div {
        let latest_thinking = messages
            .iter()
            .rev()
            .find_map(|m| m.thinking.as_ref())
            .cloned()
            .unwrap_or_else(|| "Analyzing plan DAG, evaluating action policy, checking element permissions and verifying credentials...".to_string());

        div()
            .flex()
            .flex_col()
            .w(px(320.0))
            .h_full()
            .border_l_1()
            .border_color(Theme::border_subtle())
            .bg(Theme::bg_surface())
            .p_4()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::accent_purple())
                            .child("🧠 Live Thinking Trace"),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Real-time reasoning stream streamed directly from the LLM Gateway interaction layer."),
            )
            .child(
                div()
                    .id("thinking-drawer-scroll")
                    .flex_1()
                    .p_3()
                    .rounded_lg()
                    .bg(Theme::bg_elevated())
                    .border_1()
                    .border_color(Theme::border_subtle())
                    .overflow_scroll()
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::text_secondary())
                    .child(latest_thinking),
            )
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.state.messages.read().clone();
        let thinking_open = *self.state.thinking_drawer_open.read();
        let active_channel = self.state.active_channel_id.read().clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .bg(Theme::bg_window())
            // Chat header bar
            .child(
                div()
                    .flex()
                    .h(px(44.0))
                    .w_full()
                    .border_b_1()
                    .border_color(Theme::border_subtle())
                    .items_center()
                    .justify_between()
                    .px_4()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child(match &active_channel {
                                        Some(id) => format!("Channel: {}", id),
                                        None => "General Workspace Channel".to_string(),
                                    }),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("Autonomous Loop"),
                            ),
                    )
                    .child(
                        div()
                            .id("toggle-thinking-btn")
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(if thinking_open { Theme::accent_purple() } else { Theme::bg_elevated() })
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .text_color(if thinking_open { Theme::text_primary() } else { Theme::text_secondary() })
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("🧠 Thinking Trace [t]")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.state.toggle_thinking_drawer();
                                cx.notify();
                            })),
                    ),
            )
            // Main content split (Transcript + Optional Thinking Drawer)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Conversation transcript scroll area
                    .child(
                        div()
                            .id("chat-transcript-scroll")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_scroll()
                            .p_4()
                            .gap_3()
                            .child(
                                if messages.is_empty() {
                                    self.render_empty_state()
                                } else {
                                    let mut transcript = div().flex().flex_col().gap_3();
                                    for msg in messages.iter() {
                                        transcript = transcript.child(self.render_message(msg, cx));
                                    }
                                    transcript
                                },
                            ),
                    )
                    // Thinking Trace Drawer (Right-side slide-over panel)
                    .child(
                        if thinking_open {
                            self.render_thinking_drawer(&messages)
                        } else {
                            div()
                        },
                    ),
            )
    }
}
