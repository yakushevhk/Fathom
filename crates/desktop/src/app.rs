//! Main GPUI Application View shell assembling Topbar, Sidebar, Chat/Composer,
//! Computer Viewport, Governance, Routines, Skills and Credentials Vault.

use crate::components::chat::ChatView;
use crate::components::composer::Composer;
use crate::components::computer::ComputerView;
use crate::components::governance::GovernanceView;
use crate::components::routines::RoutinesView;
use crate::components::sidebar::Sidebar;
use crate::components::skills::SkillsView;
use crate::components::topbar::Topbar;
use crate::components::vault::VaultView;
use crate::state::{AppState, NavigationTab};
use crate::theme::Theme;
use gpui::{
    div, px, prelude::*, App, Context, Div, Entity, FocusHandle, Focusable, IntoElement, Render, Window,
};
use std::sync::Arc;

pub struct DesktopApp {
    state: Arc<AppState>,
    topbar: Entity<Topbar>,
    sidebar: Entity<Sidebar>,
    chat: Entity<ChatView>,
    composer: Entity<Composer>,
    computer: Entity<ComputerView>,
    governance: Entity<GovernanceView>,
    routines: Entity<RoutinesView>,
    skills: Entity<SkillsView>,
    vault: Entity<VaultView>,
    settings: Entity<crate::components::settings::SettingsView>,
    focus_handle: gpui::FocusHandle,
}

impl Focusable for DesktopApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl DesktopApp {
    pub fn new(state: Arc<AppState>, cx: &mut Context<Self>) -> Self {
        let topbar = cx.new(|_| Topbar::new(state.clone()));
        let sidebar = cx.new(|_| Sidebar::new(state.clone()));
        let chat = cx.new(|_| ChatView::new(state.clone()));
        let composer = cx.new(|cx| Composer::new(state.clone(), cx));
        let computer = cx.new(|_| ComputerView::new(state.clone()));
        let governance = cx.new(|_| GovernanceView::new(state.clone()));
        let routines = cx.new(|_| RoutinesView::new(state.clone()));
        let skills = cx.new(|_| SkillsView::new(state.clone()));
        let vault = cx.new(|_| VaultView::new(state.clone()));
        let settings = cx.new(|_| crate::components::settings::SettingsView::new(state.clone()));
        let focus_handle = cx.focus_handle();

        // Observe child views so navigation & state changes re-render the app shell
        cx.observe(&sidebar, |_this, _sidebar, cx| {
            cx.notify();
        }).detach();
        cx.observe(&composer, |_this, _composer, cx| {
            cx.notify();
        }).detach();
        cx.observe(&chat, |_this, _chat, cx| {
            cx.notify();
        }).detach();
        cx.observe(&computer, |_this, _comp, cx| {
            cx.notify();
        }).detach();
        // Check daemon and start real-time SSE event consumption loop
        let state_clone = state.clone();
        cx.spawn(async move |this, cx| {
            let running = state_clone.daemon.is_running() || state_clone.api.health().await.unwrap_or(false);
            *state_clone.is_engine_running.write() = running;
            let _ = this.update(&mut *cx, |_this, cx| {
                cx.notify();
            });

            // Bootstrap remote state from daemon API
            if let Ok(coworkers) = state_clone.api.list_coworkers().await {
                if !coworkers.is_empty() {
                    *state_clone.coworkers.write() = coworkers;
                }
            }
            if let Ok(channels) = state_clone.api.list_channels(None).await {
                if !channels.is_empty() {
                    *state_clone.channels.write() = channels;
                }
            }
            if let Ok(routines) = state_clone.api.list_schedules().await {
                if !routines.is_empty() {
                    *state_clone.routines.write() = routines;
                }
            }
            if let Ok(creds) = state_clone.api.list_credentials().await {
                if !creds.is_empty() {
                    *state_clone.credentials.write() = creds;
                }
            }
            if let Ok(comps) = state_clone.api.list_computers().await {
                if !comps.is_empty() {
                    *state_clone.computer_sessions.write() = comps;
                }
            }
            if let Ok(policy) = state_clone.api.get_policy().await {
                *state_clone.policy.write() = Some(policy);
            }
            if let Ok(audit) = state_clone.api.get_audit_log().await {
                if !audit.is_empty() {
                    *state_clone.audit_log.write() = audit;
                }
            }
            let _ = this.update(&mut *cx, |_this, cx| {
                cx.notify();
            });

            // Real-time SSE event subscription stream with durable outer reconnect loop
            use futures::StreamExt;
            loop {
                let mut event_source = state_clone.api.subscribe_events(None);
                while let Some(event_result) = event_source.next().await {
                    match event_result {
                        Ok(reqwest_eventsource::Event::Message(msg)) => {
                            if let Ok(agent_event) = serde_json::from_str::<pr_core::AgentEvent>(&msg.data) {
                                state_clone.apply_agent_event(&agent_event);
                                let _ = this.update(&mut *cx, |_this, cx| {
                                    cx.notify();
                                });
                            }
                        }
                        Ok(reqwest_eventsource::Event::Open) => {
                            tracing::info!("SSE stream connected to Fathom daemon");
                        }
                        Err(err) => {
                            tracing::debug!("SSE connection event: {:?}", err);
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }).detach();
        Self {
            state,
            topbar,
            sidebar,
            chat,
            composer,
            computer,
            governance,
            routines,
            skills,
            vault,
            settings,
            focus_handle,
        }
    }

    fn render_agent_hub_drawer(&self) -> Div {
        let workers = self.state.subagents.read().clone();

        div()
            .flex()
            .flex_col()
            .w(px(320.0))
            .h_full()
            .bg(Theme::bg_surface())
            .border_l_1()
            .border_color(Theme::border_subtle())
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
                            .text_color(Theme::text_primary())
                            .child("⚡ AGENT HUB (omp Alt+A Parity)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::accent_purple())
                            .child(format!("{} active", workers.len())),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Parallel subagents spawned in isolated worktrees with live steering and token budgets."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(workers.into_iter().map(|w| {
                        let is_running = w.status == "running";
                        div()
                            .flex()
                            .flex_col()
                            .p_2p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .gap_1p5()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(Theme::text_primary())
                                            .child(w.name),
                                    )
                                    .child(
                                        div()
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(Theme::bg_card())
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(if is_running { Theme::success_green() } else { Theme::text_muted() })
                                            .child(w.status.to_uppercase()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::accent_blue())
                                    .child(w.role),
                            )
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .items_center()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .font_family("JetBrains Mono")
                                    .child(format!("{} tokens", w.tokens_used))
                                    .child(format!("{}s elapsed", w.duration_secs)),
                            )
                    })),
            )
    }
}

impl Render for DesktopApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_window())
            .text_color(Theme::text_primary())
            // Topbar
            .child(self.topbar.clone())
            // App Body 3-Pane Structure
            .child(
                div()
                    .flex()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    // Left Column: Coworker Fleet & Navigation
                    .child(self.sidebar.clone())
                    // Center / Main Pane based on active navigation tab
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(
                                match active_tab {
                                    NavigationTab::Channels => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.chat.clone())
                                            .child(self.composer.clone())
                                    }
                                    NavigationTab::Computer => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.computer.clone())
                                    }
                                    NavigationTab::Governance => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.governance.clone())
                                    }
                                    NavigationTab::Routines => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.routines.clone())
                                    }
                                    NavigationTab::Skills => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.skills.clone())
                                    }
                                    NavigationTab::Vault => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.vault.clone())
                                    }
                                    NavigationTab::Settings => {
                                        div()
                                            .flex()
                                            .flex_col()
                                            .flex_1()
                                            .h_full()
                                            .overflow_hidden()
                                            .child(self.settings.clone())
                                    }
                                }
                            ),
                    )
                    // Right slide-out: Agent Hub Roster Drawer (Alt+A)
                    .children(if *self.state.agent_hub_open.read() {
                        Some(self.render_agent_hub_drawer())
                    } else {
                        None
                    }),
            )
    }
}
