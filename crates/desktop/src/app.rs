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
    div, prelude::*, App, Context, Entity, FocusHandle, Focusable, IntoElement, Render, Window,
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
    focus_handle: FocusHandle,
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
        let composer = cx.new(|_| Composer::new(state.clone()));
        let computer = cx.new(|_| ComputerView::new(state.clone()));
        let governance = cx.new(|_| GovernanceView::new(state.clone()));
        let routines = cx.new(|_| RoutinesView::new(state.clone()));
        let skills = cx.new(|_| SkillsView::new(state.clone()));
        let vault = cx.new(|_| VaultView::new(state.clone()));
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

            // Real-time SSE event subscription stream
            use futures::StreamExt;
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
            focus_handle,
        }
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
                                            .p_6()
                                            .child("Engine & Local Settings")
                                    }
                                }
                            ),
                    ),
            )
    }
}
