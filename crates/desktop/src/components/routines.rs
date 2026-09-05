//! Routines component — standing instructions and scheduled autonomous tasks
//! with interactive routine creation modal, pause/resume toggle, and deletion (1:1 parity).

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, Div, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct RoutinesView {
    state: Arc<AppState>,
    show_add_modal: bool,
    new_name: String,
    new_cron: String,
    new_prompt: String,
}

impl RoutinesView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            show_add_modal: false,
            new_name: "Daily Market Sweep".to_string(),
            new_cron: "0 9 * * 1-5".to_string(),
            new_prompt: "Check competitor announcements on Twitter and G2, summarize to channel".to_string(),
        }
    }

    fn render_add_routine_modal(&self, cx: &mut Context<Self>) -> Div {
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
                    .child("Create Standing Autonomous Routine"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Routines run autonomously on the specified cron schedule without human prompting."),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
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
                                    .child("Routine Name:"),
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
                                    .text_color(Theme::text_primary())
                                    .child(self.new_name.clone()),
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
                                    .child("Cron Expression (UTC):"),
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
                                    .child(self.new_cron.clone()),
                            ),
                    ),
            )
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
                            .child("Prompt / Standing Instruction:"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .border_1()
                            .border_color(Theme::border_subtle())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .child(self.new_prompt.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("cancel-routine-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .cursor_pointer()
                            .child("Cancel")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.show_add_modal = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("confirm-routine-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .child("⚡ Register Routine")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                let name = this.new_name.clone();
                                let cron = this.new_cron.clone();
                                let prompt = this.new_prompt.clone();
                                let api = this.state.api.clone();
                                let state_clone = this.state.clone();
                                let s_cron = cron.clone();
                                let s_prompt = prompt.clone();
                                cx.spawn(async move |_this, _cx| {
                                    let _ = api.create_schedule("general_assistant", &s_cron, &s_prompt).await;
                                    if let Ok(schedules) = api.list_schedules().await {
                                        *state_clone.routines.write() = schedules;
                                    }
                                }).detach();
                                this.state.routines.write().push(crate::api::Routine {
                                    id: format!("sched_{}", &uuid::Uuid::new_v4().to_string()[..6]),
                                    name,
                                    cron,
                                    prompt,
                                    channel_id: "general".to_string(),
                                    enabled: true,
                                    last_run: None,
                                });
                                this.show_add_modal = false;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_routine_card(
        &self,
        id: &str,
        name: &str,
        cron: &str,
        prompt: &str,
        channel: &str,
        enabled: bool,
        last_run: &str,
        cx: &mut Context<Self>,
    ) -> Div {
        let r_id = id.to_string();
        let r_cron = cron.to_string();
        let r_prompt = prompt.to_string();

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
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id(SharedString::from(format!("toggle-routine-{}", id)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(if enabled { Theme::bg_card() } else { Theme::bg_elevated() })
                                    .border_1()
                                    .border_color(if enabled { Theme::success_green() } else { Theme::border_subtle() })
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(if enabled { Theme::success_green() } else { Theme::text_muted() })
                                    .cursor_pointer()
                                    .child(if enabled { "ACTIVE (Click to Pause)" } else { "PAUSED (Click to Run)" })
                                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                        let api = this.state.api.clone();
                                        let s_id = r_id.clone();
                                        let s_cron = r_cron.clone();
                                        let s_prompt = r_prompt.clone();
                                        let new_status = !enabled;
                                        let api_id = s_id.clone();
                                        let api_clone = api.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            let _ = api_clone.toggle_schedule(&api_id, "general_assistant", &s_cron, &s_prompt, new_status).await;
                                        }).detach();
                                        if let Some(r) = this.state.routines.write().iter_mut().find(|r| r.id == s_id) {
                                            r.enabled = new_status;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child({
                                let d_id = id.to_string();
                                div()
                                    .id(SharedString::from(format!("delete-routine-{}", d_id)))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .text_color(Theme::danger_red())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::danger_red()).text_color(Theme::text_primary()))
                                    .child("Delete ✕")
                                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                        let del_id = d_id.clone();
                                        let api = this.state.api.clone();
                                        let api_del = del_id.clone();
                                        cx.spawn(async move |_this, _cx| {
                                            let _ = api.delete_schedule(&api_del).await;
                                        }).detach();
                                        this.state.routines.write().retain(|r| r.id != del_id);
                                        cx.notify();
                                    }))
                            }),
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
                    .pt_2()
                    .border_t_1()
                    .border_color(Theme::border_subtle())
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(format!("Target Channel: #{}", channel)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(format!("Last Execution: {}", last_run)),
                    ),
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
            .bg(Theme::bg_window())
            .p_6()
            .gap_4()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
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
                                    .child("Automated cron schedules running autonomous background worker turns against designated channels."),
                            ),
                    )
                    .child(
                        div()
                            .id("new-routine-btn")
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
                                this.show_add_modal = !this.show_add_modal;
                                cx.notify();
                            })),
                    ),
            )
            .children(if self.show_add_modal {
                Some(self.render_add_routine_modal(cx))
            } else {
                None
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        if routines.is_empty() {
                            div().flex().flex_col().gap_3()
                                .child(self.render_routine_card("sched_1", "Morning Lead Qualification", "0 9 * * 1-5", "Scan newly posted RFP notices on target procurement boards and draft outreach summaries", "sdr-leads", true, "Today at 09:00", cx))
                                .child(self.render_routine_card("sched_2", "Hourly Security Perimeter Check", "0 * * * *", "Verify all public endpoints against CEL boundaries and ensure zero credential leaks", "sec-audit", true, "42m ago", cx))
                                .child(self.render_routine_card("sched_3", "Weekly Synthesis Digest", "0 18 * * 5", "Aggregate competitor updates, closed deals, and model token budgets into executive summary", "general", false, "Friday at 18:00", cx))
                        } else {
                            let mut list = div().flex().flex_col().gap_3();
                            for r in routines {
                                list = list.child(self.render_routine_card(&r.id, &r.name, &r.cron, &r.prompt, &r.channel_id, r.enabled, r.last_run.as_deref().unwrap_or("Never"), cx));
                            }
                            list
                        },
                    ),
            )
    }
}
