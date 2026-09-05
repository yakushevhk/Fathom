//! Routines component — standing instructions and scheduled autonomous tasks.

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, Div, IntoElement, Render, Window,
};
use std::sync::Arc;

pub struct RoutinesView {
    state: Arc<AppState>,
}

impl RoutinesView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    fn render_routine_card(
        &self,
        name: &str,
        cron: &str,
        prompt: &str,
        channel: &str,
        enabled: bool,
        last_run: &str,
    ) -> Div {
        div()
            .flex()
            .flex_col()
            .p_4()
            .rounded_lg()
            .bg(Theme::bg_surface())
            .border_1()
            .border_color(Theme::border_subtle())
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_primary())
                                    .child(name.to_string()),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::accent_purple())
                                    .child(cron.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(if enabled { Theme::bg_card() } else { Theme::bg_elevated() })
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if enabled { Theme::success_green() } else { Theme::text_muted() })
                            .child(if enabled { "ACTIVE" } else { "PAUSED" }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_secondary())
                    .child(prompt.to_string()),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child(format!("Target Channel: #{}", channel))
                    .child(format!("Last Execution: {}", last_run)),
            )
    }
}

impl Render for RoutinesView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let routines = self.state.routines.read().clone();

        div()
            .id("routines-view-root")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_scroll()
            .bg(Theme::bg_window())
            .p_6()
            .gap_4()
            // Header
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child("Standing Instructions & Routines"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("Automated tasks triggered on schedule via shared PostgreSQL queue leases with attempt caps."),
                            ),
                    )
                    .child(
                        div()
                            .id("add-routine-btn")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::accent_blue()))
                            .child("+ New Routine")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.state.routines.write().push(crate::api::Routine {
                                    id: format!("routine_{}", &uuid::Uuid::new_v4().to_string()[..6]),
                                    name: "Daily Market Sweep".to_string(),
                                    cron: "0 9 * * 1-5".to_string(),
                                    prompt: "Scan Hacker News and TechCrunch for target AI keywords and post digest.".to_string(),
                                    channel_id: "general".to_string(),
                                    enabled: true,
                                    last_run: Some("Today at 09:00".to_string()),
                                });
                                cx.notify();
                            })),
                    ),
            )
            // Routine list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        if routines.is_empty() {
                            div().flex().flex_col().gap_3()
                                .child(self.render_routine_card("Morning Standup Digest", "0 9 * * 1-5", "Collect GitHub PRs and Jira tickets, summarize blockers for team", "general", true, "Today 09:00:02"))
                                .child(self.render_routine_card("Nightly Competitor Pricing Check", "0 0 * * *", "Browse pricing pages via Playwright, extract table deltas and commit to knowledge base", "risk_compliance", true, "Yesterday 00:00:15"))
                                .child(self.render_routine_card("Hourly Security Audit Sweep", "0 * * * *", "Inspect newly opened firewall rules and unauthorized OAuth tokens", "devops", false, "Paused"))
                        } else {
                            let mut list = div().flex().flex_col().gap_3();
                            for r in routines {
                                list = list.child(self.render_routine_card(&r.name, &r.cron, &r.prompt, &r.channel_id, r.enabled, r.last_run.as_deref().unwrap_or("Never")));
                            }
                            list
                        },
                    ),
            )
    }
}
