//! Glassmorphic Metal Topbar component with traffic lights, breadcrumbs,
//! model badges, status indicators and navigation controls.

use crate::state::{AppState, NavigationTab};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, Context, IntoElement, Render, Window,
};
use std::sync::Arc;

pub struct Topbar {
    state: Arc<AppState>,
}

impl Topbar {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl Render for Topbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_running = *self.state.is_engine_running.read();
        let active_tab = self.state.active_tab();

        let tab_title = match active_tab {
            NavigationTab::Channels => "Channels & Coworkers",
            NavigationTab::Computer => "Live Computer Viewport",
            NavigationTab::Governance => "Governance & Audit",
            NavigationTab::Routines => "Routines & Automation",
            NavigationTab::Skills => "Skills & Tools",
            NavigationTab::Vault => "Credentials Vault",
            NavigationTab::Settings => "Settings & Engine",
        };

        div()
            .flex()
            .h(px(40.0))
            .w_full()
            .bg(Theme::bg_topbar())
            .border_b_1()
            .border_color(Theme::border_subtle())
            .items_center()
            .justify_between()
            .px_4()
            // Left: macOS Traffic lights & Breadcrumbs
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(div().size(px(11.0)).rounded_full().bg(Theme::traffic_red()))
                            .child(div().size(px(11.0)).rounded_full().bg(Theme::traffic_yellow()))
                            .child(div().size(px(11.0)).rounded_full().bg(Theme::traffic_green())),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .child(
                                div()
                                    .text_color(Theme::text_muted())
                                    .child("Fathom"),
                            )
                            .child(
                                div()
                                    .text_color(Theme::text_muted())
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .text_color(Theme::text_primary())
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(tab_title),
                            ),
                    ),
            )
            // Right: Engine status, Model badge & Quick actions
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .rounded_md()
                            .text_xs()
                            .child(
                                div()
                                    .size(px(6.0))
                                    .rounded_full()
                                    .bg(if is_running { Theme::success_green() } else { Theme::danger_red() }),
                            )
                            .child(
                                div()
                                    .text_color(if is_running { Theme::success_green() } else { Theme::text_muted() })
                                    .child(if is_running { "Daemon Online :8080" } else { "Engine Offline" }),
                            ),
                    )
                    .child(
                        div()
                            .id("topbar-collab-btn")
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .text_color(Theme::accent_blue())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_card()))
                            .child("🔗 /collab")
                            .on_click(cx.listener(|this, _event: &gpui::ClickEvent, _window, cx| {
                                let sess_id = this.state.active_session_id.read().clone().unwrap_or_else(|| "sess-default".to_string());
                                this.state.add_message(crate::state::ChatMessage {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    role: "system".to_string(),
                                    content: serde_json::json!({
                                        "session_id": sess_id,
                                        "relay_url": "https://relay.fathom.internal/join#token=fth_live_99a",
                                        "role": "peer",
                                        "peer_count": 1,
                                        "is_active": true
                                    }).to_string(),
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
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .rounded_md()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("📦 package: fintech-production"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_0p5()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_focus())
                            .rounded_md()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::accent_purple())
                            .child("⚡ DeepSeek V3 / Sonnet 3.7"),
                    ),
            )
    }
}
