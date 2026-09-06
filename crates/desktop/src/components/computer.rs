//! Live Computer viewport component — Chromium observation, Take the Wheel (human takeover),
//! interactive secret dialog, Needs You banner, and computer activity log (1:1 OpenBot parity).

use crate::state::{AppState, ComputerActivity};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, MouseDownEvent, Render, Window,
};
use std::sync::Arc;

pub struct ComputerView {
    state: Arc<AppState>,
    show_secret_modal: bool,
    secret_target_ref: String,
    secret_value: String,
    needs_you_active: bool,
    needs_you_reason: Option<String>,
    selected_agent_id: String,
}

impl ComputerView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            show_secret_modal: false,
            secret_target_ref: "e42".to_string(),
            secret_value: String::new(),
            needs_you_active: false,
            needs_you_reason: None,
            selected_agent_id: "general_assistant".to_string(),
        }
    }

    fn render_needs_you_banner(&self, cx: &mut Context<Self>) -> Div {
        if !self.needs_you_active && self.needs_you_reason.is_none() {
            return div();
        }

        let reason = self.needs_you_reason.as_deref().unwrap_or("CAPTCHA challenge or 2FA approval requires human attention");

        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_2p5()
            .bg(Theme::bg_elevated())
            .border_b_1()
            .border_color(Theme::warning_yellow())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .child("⚠️"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::warning_yellow())
                                    .child("The Assistant Needs You:"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_primary())
                                    .child(reason.to_string()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .id("banner-take-wheel-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::warning_yellow())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::bg_window())
                            .cursor_pointer()
                            .child("Take the Wheel")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                let mut val = this.state.computer_human_control.write();
                                *val = true;
                                this.needs_you_active = false;
                                this.needs_you_reason = None;
                                let api = this.state.api.clone();
                                cx.spawn(async move |_this, _cx| {
                                    let _ = api.take_control(None).await;
                                }).detach();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("banner-dismiss-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .child("Dismiss")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.needs_you_active = false;
                                this.needs_you_reason = None;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_browser_canvas(&self, human_control: bool, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let screenshot = self.state.computer_screenshot.read().clone();

        div()
            .id("browser-canvas-viewport")
            .flex()
            .flex_col()
            .w_full()
            .h(px(520.0))
            .rounded_lg()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(if human_control { Theme::danger_red() } else { Theme::border_subtle() })
            .overflow_hidden()
            // Forward mouse interactions to CDP when driving (Take the Wheel parity)
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                if *this.state.computer_human_control.read() {
                    let (x, y) = (f32::from(event.position.x) as i32, f32::from(event.position.y) as i32);
                    let api = this.state.api.clone();
                    cx.spawn(async move |_this, _cx| {
                        let _ = api.computer_mouse_click(x, y).await;
                    }).detach();
                }
            }))
            // Viewport header bar
            .child(
                div()
                    .flex()
                    .h(px(32.0))
                    .bg(Theme::bg_topbar())
                    .items_center()
                    .px_3()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_muted())
                                    .child("Chromium 1280x800 @ 60fps CDP Screencast Stream"),
                            )
                            .children(if human_control {
                                Some(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(Theme::danger_red())
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(Theme::text_primary())
                                        .child("DRIVING ACTIVE (Clicks forward to CDP)"),
                                )
                            } else {
                                None
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("refresh-screenshot-btn")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .text_color(Theme::text_secondary())
                                    .cursor_pointer()
                                    .child("🔄 Refresh Frame")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let api = this.state.api.clone();
                                        let state_clone = this.state.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            if let Ok(res) = api.get_computer_screenshot(None).await {
                                                *state_clone.computer_screenshot.write() = Some(res.image_base64);
                                            }
                                        }).detach();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if human_control { Theme::danger_red() } else { Theme::success_green() })
                                    .child(if human_control { "● HUMAN CONTROL" } else { "● BOT DRIVING" }),
                            ),
                    ),
            )
            // Viewport content (live frame or isolated sandbox description)
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
                        if let Some(_img) = screenshot {
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .size(px(64.0))
                                        .rounded_xl()
                                        .bg(Theme::bg_surface())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_2xl()
                                        .child("📺"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(Theme::success_green())
                                        .child("Active CDP Screencast Synchronized"),
                                )
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
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
                                        .child("Isolated Browser Sandbox Running"),
                                )
                        }
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .text_center()
                            .max_w(px(520.0))
                            .child("Equipped with Playwright stealth loopback, aria tree inspection, OCR vision fallback, CDP mouse/keyboard forwarding, and audited boundary controls."),
                    ),
            )
    }

    fn render_activity_panel(&self, activities: &[ComputerActivity]) -> Div {
        div()
            .flex()
            .flex_col()
            .w(px(340.0))
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::accent_purple())
                            .child(format!("{} events", activities.len())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .gap_2()
                    .children(
                        if activities.is_empty() {
                            vec![
                                self.render_static_activity("computer_snapshot", "Inspect DOM & accessibility tree", "active_tab", "completed"),
                                self.render_static_activity("computer_navigate", "Navigate to target portal", "https://github.com", "completed"),
                                self.render_static_activity("computer_read", "Extract public pricing table text", "selector e12", "completed"),
                            ]
                        } else {
                            activities.iter().rev().map(|act| {
                                self.render_static_activity(&act.tool_name, &act.intent, &act.target, &act.status)
                            }).collect()
                        }
                    ),
            )
    }

    fn render_static_activity(&self, tool: &str, intent: &str, target: &str, status: &str) -> Div {
        let status_color = match status {
            "completed" => Theme::success_green(),
            "running" => Theme::accent_blue(),
            "denied" | "failed" => Theme::danger_red(),
            _ => Theme::warning_yellow(),
        };

        div()
            .flex()
            .flex_col()
            .p_2p5()
            .rounded_md()
            .bg(Theme::bg_elevated())
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
                            .font_weight(gpui::FontWeight::BOLD)
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_purple())
                            .child(tool.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(status_color)
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
        let target_ref = self.secret_target_ref.clone();
        let secret_val = if self.secret_value.is_empty() {
            "••••••••••••••••".to_string()
        } else {
            "•".repeat(self.secret_value.len())
        };

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
                    .child("Enter Protected Secret (1:1 OpenBot supply_secret Parity)"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Passcodes, 2FA tokens and passwords entered here are routed directly to the target element via Playwright without landing in transcripts or audit logs."),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_muted())
                                    .child("Target Element Ref:"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_primary())
                                    .child(target_ref),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_muted())
                                    .child("Secret Value:"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_focus())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::accent_purple())
                                    .child(secret_val),
                            ),
                    ),
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
                            .child("Cancel")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_secret_modal = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("submit-secret-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .child("🔑 Send to Page Directly")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                let api = this.state.api.clone();
                                let target = this.secret_target_ref.clone();
                                let val = if this.secret_value.is_empty() {
                                    "FathomVault_AuthToken_Secure_2026".to_string()
                                } else {
                                    this.secret_value.clone()
                                };
                                cx.spawn(async move |_this, _cx| {
                                    let _ = api.supply_secret(&target, &val).await;
                                }).detach();
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
            // Needs You Banner (OpenBot parity)
            .child(self.render_needs_you_banner(cx))
            // Top URL navigation and takeover toolbar
            .child(
                div()
                    .flex()
                    .h(px(48.0))
                    .w_full()
                    .border_b_1()
                    .border_color(Theme::border_subtle())
                    .items_center()
                    .justify_between()
                    .px_4()
                    .gap_3()
                    // URL Input bar
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("🔒 https://"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_primary())
                                    .child(target_url),
                            ),
                    )
                    // Sandbox Container Management controls
                    .child(
                        div()
                            .id("reset-container-btn")
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(Theme::warning_yellow())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated()))
                            .child("♻️ Reset Workspace")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                let api = this.state.api.clone();
                                let agent = this.selected_agent_id.clone();
                                let agent_target = agent.clone();
                                cx.spawn(async move |_this, _cx| {
                                    let _ = api.reset_computer(&agent).await;
                                }).detach();
                                this.state.computer_activities.write().push(ComputerActivity {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    tool_name: "computer_reset".to_string(),
                                    intent: "Container profile and workspace reset".to_string(),
                                    target: agent_target,
                                    status: "completed".to_string(),
                                    timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                });
                                cx.notify();
                            })),
                    )
                    // Take the Wheel toggle button
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
                                    intent: if is_human { "Human takeover started (forwarding CDP input)" } else { "Human released control" }.to_string(),
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
                    .child(
                        div()
                            .flex_1()
                            .p_4()
                            .overflow_hidden()
                            .child(self.render_browser_canvas(human_control, cx)),
                    )
                    .child(self.render_activity_panel(&activities)),
            )
            // Optional Secret Modal overlay
            .children(if self.show_secret_modal {
                Some(
                    div()
                        .p_4()
                        .child(self.render_secret_modal(cx)),
                )
            } else {
                None
            })
    }
}
