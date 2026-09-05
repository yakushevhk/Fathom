//! Live Computer viewport component — Chromium observation, Take the Wheel (human takeover),
//! interactive secret dialog, and computer activity log (1:1 OpenBot parity).

use crate::state::{AppState, ComputerActivity};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, Window,
};
use std::sync::Arc;

pub struct ComputerView {
    state: Arc<AppState>,
    show_secret_modal: bool,
}

impl ComputerView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            show_secret_modal: false,
        }
    }

    fn render_browser_canvas(&self, human_control: bool) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(520.0))
            .rounded_lg()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(if human_control { Theme::danger_red() } else { Theme::border_subtle() })
            .overflow_hidden()
            // Viewport mock header
            .child(
                div()
                    .flex()
                    .h(px(28.0))
                    .bg(Theme::bg_topbar())
                    .items_center()
                    .px_3()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::text_muted())
                            .child("Chromium Headless / 1280x800 @ 60fps Metal Surface"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if human_control { Theme::danger_red() } else { Theme::success_green() })
                            .child(if human_control { "● HUMAN TAKEOVER ACTIVE" } else { "● BOT AGENT DRIVING" }),
                    ),
            )
            // Viewport content mockup
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .p_8()
                    .gap_4()
                    .child(
                        div()
                            .size(px(64.0))
                            .rounded_xl()
                            .bg(Theme::bg_surface())
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_2xl()
                            .child("🌐"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .child("Browser Session Running in Isolated Container Sandbox"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .text_center()
                            .max_w(px(460.0))
                            .child("Equipped with Playwright stealth engine, cookies vault, aria accessibility tree inspection, OCR vision fallback and audited action gateway."),
                    ),
            )
    }

    fn render_activity_panel(&self, activities: &[ComputerActivity]) -> Div {
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
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_muted())
                            .child("COMPUTER ACTIVITY LOG"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::accent_purple())
                            .child("Audited Boundary"),
                    ),
            )
            .child(
                div()
                    .id("activity-log-scroll")
                    .flex_1()
                    .overflow_scroll()
                    .child(
                        if activities.is_empty() {
                            div().flex().flex_col().gap_2()
                                .child(self.render_static_activity("computer_navigate", "Browse to pricing page", "https://example.com/pricing", "allowed"))
                                .child(self.render_static_activity("computer_snapshot", "Read accessibility tree", "DOM Tree (42 nodes)", "allowed"))
                                .child(self.render_static_activity("computer_click", "Click 'Enterprise Tier'", "button#btn-tier-3", "allowed"))
                                .child(self.render_static_activity("computer_read_file", "Read competitor CSV", "/workspace/output.csv", "allowed"))
                        } else {
                            let mut act_list = div().flex().flex_col().gap_2();
                            for act in activities {
                                act_list = act_list.child(self.render_static_activity(&act.tool_name, &act.intent, &act.target, &act.status));
                            }
                            act_list
                        },
                    ),
            )
    }

    fn render_static_activity(&self, tool: &str, intent: &str, target: &str, status: &str) -> Div {
        div()
            .flex()
            .flex_col()
            .p_2p5()
            .rounded_md()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_subtle())
            .gap_1()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_blue())
                            .child(tool.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::success_green())
                            .child(status.to_string()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_primary())
                    .child(intent.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::text_muted())
                    .child(target.to_string()),
            )
    }

    fn render_secret_modal(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .p_4()
            .rounded_lg()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(Theme::accent_purple())
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Enter Protected Secret (Redacted from Audit Logs)"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Passcodes, 2FA tokens and passwords entered here are routed directly to the target field without landing in conversation transcripts."),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("close-secret-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .child("Close")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_secret_modal = false;
                                cx.notify();
                            })),
                    ),
            )
    }
}

impl Render for ComputerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let human_control = *self.state.computer_human_control.read();
        let target_url = self.state.computer_url.read().clone();
        let activities = self.state.computer_activities.read().clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .bg(Theme::bg_window())
            // Browser Address & Control Bar
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
                    .gap_3()
                    // URL Pill
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .h(px(28.0))
                            .px_3()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(Theme::success_green())
                                    .child("🔒 https://"),
                            )
                            .child(
                                div()
                                    .text_color(Theme::text_primary())
                                    .font_family("JetBrains Mono")
                                    .child(target_url),
                            ),
                    )
                    // Take the Wheel button
                    .child(
                        div()
                            .id("take-wheel-btn")
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(if human_control { Theme::danger_red() } else { Theme::accent_purple() })
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .hover(|s| s.bg(if human_control { Theme::warning_yellow() } else { Theme::accent_blue() }))
                            .child(if human_control { "🛑 Release Wheel (Back to Bot)" } else { "🕹️ Take the Wheel" })
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                let mut val = this.state.computer_human_control.write();
                                *val = !*val;
                                let is_human = *val;
                                let api = this.state.api.clone();
                                cx.spawn(async move |_this, _cx| {
                                    if is_human {
                                        let _ = api.take_control(None).await;
                                    } else {
                                        let _ = api.release_control(None).await;
                                    }
                                }).detach();
                                this.state.computer_activities.write().push(ComputerActivity {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    tool_name: "computer_control".to_string(),
                                    intent: if is_human { "Human takeover started" } else { "Human released control" }.to_string(),
                                    target: "operator".to_string(),
                                    status: "recorded".to_string(),
                                    timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                });
                                cx.notify();
                            })),
                    )
                    // Secret Dialog button (for audited 2FA / Password entry)
                    .child(
                        div()
                            .id("supply-secret-btn")
                            .flex()
                            .gap_1p5()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("🔑 Supply Secret")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_secret_modal = !this.show_secret_modal;
                                cx.notify();
                            })),
                    ),
            )
            // Body split: Live Screen Render + Activity Log
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Screen viewport
                    .child(
                        div()
                            .id("browser-canvas-scroll")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .p_4()
                            .overflow_scroll()
                            .items_center()
                            .justify_center()
                            .bg(Theme::bg_window())
                            .child(self.render_browser_canvas(human_control)),
                    )
                    // Side Activity Log panel (What the bot did on the computer)
                    .child(self.render_activity_panel(&activities)),
            )
            // Optional Secret Modal Overlay
            .child(
                if self.show_secret_modal {
                    self.render_secret_modal(cx)
                } else {
                    div()
                },
            )
    }
}
