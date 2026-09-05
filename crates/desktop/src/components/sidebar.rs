//! Sidebar component with Coworker fleet, channels, search, navigation tabs,
//! interactive Agent Profile inspection/editing dialog, and Multi-Agent Handoff Matrix (1:1 OpenBot parity).

use crate::state::{AppState, NavigationTab};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct Sidebar {
    state: Arc<AppState>,
    selected_agent_for_dialog: Option<String>,
    show_handoff_matrix: bool,
}

impl Sidebar {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            selected_agent_for_dialog: None,
            show_handoff_matrix: false,
        }
    }

    fn render_agent_profile_dialog(&self, agent_id: &str, cx: &mut Context<Self>) -> Div {
        let (name, title, prompt) = match agent_id {
            "risk_analyst" => ("Risk Analyst", "🛡️ Compliance & Audit Officer", "Evaluate actions against CEL boundaries and ensure zero secret leaks."),
            "devops_engineer" => ("DevOps Engineer", "🐳 Infrastructure & Automation", "Manage containers, deployments, health probes and process trees."),
            _ => ("General Assistant", "🤖 General Autonomous Worker", "Full computer use, research, web automation, data synthesis and lead finding."),
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child(format!("Agent Profile: {}", name)),
                    )
                    .child(
                        div()
                            .id("close-agent-dialog-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_card())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .child("✕ Close")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.selected_agent_for_dialog = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(Theme::accent_blue())
                    .child(title),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_muted())
                            .child("STANDING INSTRUCTIONS / SYSTEM PROMPT:"),
                    )
                    .child(
                        div()
                            .p_2()
                            .rounded_md()
                            .bg(Theme::bg_window())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::text_secondary())
                            .child(prompt),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_2()
                    .border_t_1()
                    .border_color(Theme::border_subtle())
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Autonomous Loop: Enabled"),
                    )
                    .child(
                        div()
                            .id("open-handoff-btn")
                            .px_2p5()
                            .py_1()
                            .rounded_sm()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .child("Configure Handoff Matrix ➔")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_handoff_matrix = !this.show_handoff_matrix;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_handoff_matrix(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .p_4()
            .rounded_lg()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_focus())
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::accent_purple())
                            .child("Multi-Agent Handoff Matrix (OpenBot handoff-panel parity)"),
                    )
                    .child(
                        div()
                            .id("close-handoff-matrix-btn")
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .child("✕")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_handoff_matrix = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Directional delegation permissions. Controls which peer agents a coworker is permitted to address via `message_bot` or escalate via `ask_person`."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .pt_1()
                    .child(self.render_handoff_row("General Assistant ➔ Risk Analyst", true, cx))
                    .child(self.render_handoff_row("Risk Analyst ➔ General Assistant", true, cx))
                    .child(self.render_handoff_row("General Assistant ➔ DevOps Engineer", true, cx))
                    .child(self.render_handoff_row("DevOps Engineer ➔ Human Operator (ask_person)", true, cx)),
            )
    }

    fn render_handoff_row(&self, title: &str, granted: bool, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p_2()
            .rounded_md()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(Theme::border_subtle())
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(Theme::text_primary())
                    .child(title.to_string()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("grant-btn-{}", title)))
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(if granted { Theme::success_green() } else { Theme::danger_red() })
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::bg_window())
                    .cursor_pointer()
                    .child(if granted { "GRANTED ✓" } else { "DENIED ✗" })
                    .on_click(cx.listener(|_this, _event: &ClickEvent, _window, cx| {
                        cx.notify();
                    })),
            )
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
                    .overflow_hidden()
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
                                                .text_xs()
                                                .text_color(Theme::accent_purple())
                                                .child("#"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .overflow_hidden()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                                        .text_color(Theme::text_primary())
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
                                .child(self.render_clickable_coworker("general_assistant", "General Assistant", "SDR & Web Research", "🟢 Online", cx))
                                .child(self.render_clickable_coworker("risk_analyst", "Risk Analyst", "Compliance & Audit", "🟢 Online", cx))
                                .child(self.render_clickable_coworker("devops_engineer", "DevOps Engineer", "Docker & Kubernetes", "🟢 Idle", cx))
                        } else {
                            let mut cow_elements = div().flex().flex_col().gap_1();
                            for cw in coworkers {
                                cow_elements = cow_elements.child(
                                    self.render_clickable_coworker(&cw.id, &cw.name, &cw.title, "🟢 Online", cx),
                                );
                            }
                            cow_elements
                        },
                    ),
            )
            // Optional Agent Profile Dialog modal
            .children(self.selected_agent_for_dialog.as_ref().map(|agent_id| {
                div().p_3().child(self.render_agent_profile_dialog(agent_id, cx))
            }))
            // Optional Handoff Matrix modal
            .children(if self.show_handoff_matrix {
                Some(div().p_3().child(self.render_handoff_matrix(cx)))
            } else {
                None
            })
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

    fn render_clickable_coworker(
        &self,
        id: &str,
        name: &str,
        role: &str,
        status: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let agent_id = id.to_string();

        div()
            .id(SharedString::from(format!("coworker-item-{}", id)))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1p5()
            .rounded_md()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_subtle())
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_elevated_hover()))
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
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
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
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.selected_agent_for_dialog = Some(agent_id.clone());
                cx.notify();
            }))
    }
}
