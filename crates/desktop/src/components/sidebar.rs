//! Sidebar component with Coworker fleet, channels, search, and navigation tabs.

use crate::state::{AppState, NavigationTab};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct Sidebar {
    state: Arc<AppState>,
}

impl Sidebar {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab();
        let channels = self.state.channels.read().clone();
        let active_channel_id = self.state.active_channel_id.read().clone();
        let coworkers = self.state.coworkers.read().clone();

        div()
            .flex()
            .flex_col()
            .w(px(260.0))
            .h_full()
            .bg(Theme::bg_surface())
            .border_r_1()
            .border_color(Theme::border_subtle())
            // Navigation tabs selector
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(Theme::border_subtle())
                    .child(self.render_nav_item("Channels & Coworkers", NavigationTab::Channels, active_tab, "💬", cx))
                    .child(self.render_nav_item("Live Computer", NavigationTab::Computer, active_tab, "🖥️", cx))
                    .child(self.render_nav_item("Governance & Policy", NavigationTab::Governance, active_tab, "🛡️", cx))
                    .child(self.render_nav_item("Routines & Cron", NavigationTab::Routines, active_tab, "⚡", cx))
                    .child(self.render_nav_item("Skills & Tools", NavigationTab::Skills, active_tab, "🧩", cx))
                    .child(self.render_nav_item("Credentials Vault", NavigationTab::Vault, active_tab, "🔑", cx))
            )
            // Channels / Coworker List section
            .child(
                div()
                    .id("sidebar-channels-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_scroll()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .mb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_muted())
                                    .child("ACTIVE CHANNELS"),
                            )
                            .child(
                                div()
                                    .id("new-channel-btn")
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .text_color(Theme::text_secondary())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                                    .child("+ New")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let new_id = format!("chan_{}", &uuid::Uuid::new_v4().to_string()[..8]);
                                        this.state.channels.write().push(crate::api::Channel {
                                            id: new_id.clone(),
                                            coworker_id: "general_assistant".to_string(),
                                            title: "Research & Synthesis".to_string(),
                                            session_id: None,
                                            created_at: chrono::Utc::now().to_rfc3339(),
                                            updated_at: chrono::Utc::now().to_rfc3339(),
                                        });
                                        this.state.set_active_channel(Some(new_id));
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        if channels.is_empty() {
                            div()
                                .p_2()
                                .text_xs()
                                .text_color(Theme::text_muted())
                                .child("No open channels. Click + New to begin.")
                        } else {
                            let mut channel_elements = div().flex().flex_col().gap_1();
                            for chan in channels {
                                let is_active = active_channel_id.as_deref() == Some(&chan.id);
                                let chan_id = chan.id.clone();
                                let title = chan.title.clone();

                                channel_elements = channel_elements.child(
                                    div()
                                        .id(SharedString::from(format!("channel-{}", chan_id)))
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .px_2p5()
                                        .py_2()
                                        .rounded_md()
                                        .bg(if is_active { Theme::bg_elevated() } else { Theme::bg_surface() })
                                        .border_1()
                                        .border_color(if is_active { Theme::border_focus() } else { Theme::border_subtle() })
                                        .cursor_pointer()
                                        .hover(|s| s.bg(Theme::bg_elevated_hover()))
                                        .child(
                                            div()
                                                .size(px(8.0))
                                                .rounded_full()
                                                .bg(if is_active { Theme::accent_purple() } else { Theme::text_muted() }),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(if is_active { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
                                                        .text_color(if is_active { Theme::text_primary() } else { Theme::text_secondary() })
                                                        .child(title),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Theme::text_muted())
                                                        .child(chan_id.clone()),
                                                ),
                                        )
                                        .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                            this.state.set_active_channel(Some(chan_id.clone()));
                                            this.state.set_active_tab(NavigationTab::Channels);
                                            cx.notify();
                                        })),
                                );
                            }
                            channel_elements
                        },
                    )
                    // Coworker Fleet roster
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .mt_4()
                            .mb_2()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_muted())
                                    .child("COWORKER FLEET"),
                            ),
                    )
                    .child(
                        if coworkers.is_empty() {
                            div().flex().flex_col().gap_1()
                                .child(self.render_static_coworker("General Assistant", "SDR & Web Research", "🟢 Online"))
                                .child(self.render_static_coworker("Risk Analyst", "Compliance & Audit", "🟢 Online"))
                                .child(self.render_static_coworker("DevOps Engineer", "Docker & Kubernetes", "🟢 Idle"))
                        } else {
                            let mut cow_elements = div().flex().flex_col().gap_1();
                            for cw in coworkers {
                                cow_elements = cow_elements.child(
                                    self.render_static_coworker(&cw.name, &cw.title, "🟢 Online"),
                                );
                            }
                            cow_elements
                        },
                    ),
            )
    }
}

impl Sidebar {
    fn render_nav_item(
        &self,
        label: &'static str,
        tab: NavigationTab,
        active_tab: NavigationTab,
        icon: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = tab == active_tab;
        let id_name = format!("nav-tab-{:?}", tab);

        div()
            .id(SharedString::from(id_name))
            .flex()
            .items_center()
            .gap_2()
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .bg(if is_active { Theme::bg_elevated() } else { Theme::bg_surface() })
            .border_1()
            .border_color(if is_active { Theme::border_focus() } else { Theme::border_subtle() })
            .text_xs()
            .font_weight(if is_active { gpui::FontWeight::SEMIBOLD } else { gpui::FontWeight::NORMAL })
            .text_color(if is_active { Theme::text_primary() } else { Theme::text_secondary() })
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
            .child(div().child(icon))
            .child(div().child(label))
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.state.set_active_tab(tab);
                cx.notify();
            }))
    }

    fn render_static_coworker(&self, name: &str, role: &str, status: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded_md()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_subtle())
            .child(
                div()
                    .size(px(24.0))
                    .rounded_md()
                    .bg(Theme::bg_elevated())
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .child("🤖"),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(role.to_string()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::success_green())
                    .child(status.to_string()),
            )
    }
}
