//! Settings & Administration view component — Engine configurations, Standing Instructions,
//! Connected Accounts, and UI Components Gallery preview (1:1 parity).

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, SharedString, Stateful,
    Window,
};
use std::sync::Arc;

pub struct SettingsView {
    pub state: Arc<AppState>,
    selected_subtab: String, // "engine" | "instructions" | "accounts" | "components"
    standing_instructions: String,
}

impl SettingsView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            selected_subtab: "engine".to_string(),
            standing_instructions: "Default Worker Policy: Always verify tool action boundaries before proceeding. Never leak credentials or private keys. Prefer concise evidence-based summaries.".to_string(),
        }
    }

    fn render_subtab_button(
        &self,
        label: &'static str,
        id: &'static str,
        active_id: &str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let is_active = id == active_id;
        div()
            .id(SharedString::from(format!("settings-subtab-{}", id)))
            .px_3()
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
            .child(label)
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.selected_subtab = id.to_string();
                cx.notify();
            }))
    }

    fn render_engine_settings(&self) -> Div {
        let is_running = *self.state.is_engine_running.read();

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_4()
            .child(
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
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child("Local Fathom Daemon & Embedded Supervisor"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(if is_running { Theme::bg_card() } else { Theme::bg_elevated() })
                                    .border_1()
                                    .border_color(if is_running { Theme::success_green() } else { Theme::danger_red() })
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(if is_running { Theme::success_green() } else { Theme::danger_red() })
                                    .child(if is_running { "● RUNNING (Port 8080)" } else { "○ STOPPED" }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("The native daemon runs SQLite persistence, the event reactor, CEL policy evaluation, Playwright loopback, and local SSE streaming."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p_4()
                    .rounded_lg()
                    .bg(Theme::bg_surface())
                    .border_1()
                    .border_color(Theme::border_subtle())
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Storage & Database Location"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::accent_purple())
                                    .child("~/.fathom/fathom.db"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("(AES-256-GCM hardware ring AEAD authenticated)"),
                            ),
                    ),
            )
    }

    fn render_instructions_settings(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .p_4()
            .rounded_lg()
            .bg(Theme::bg_surface())
            .border_1()
            .border_color(Theme::border_subtle())
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Global Standing Instructions"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Injected into every agent prompt across all channels and routines before turn execution."),
            )
            .child(
                div()
                    .p_3()
                    .rounded_md()
                    .bg(Theme::bg_window())
                    .border_1()
                    .border_color(Theme::border_focus())
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::text_secondary())
                    .child(self.standing_instructions.clone()),
            )
    }

    fn render_connected_accounts(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Connected Accounts & OAuth Providers"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_account_row("GitHub", "Enterprise Git & CI/CD", "Connected as @yakushev", true))
                    .child(self.render_account_row("Google Workspace", "Drive, Gmail & Docs", "Connected", true))
                    .child(self.render_account_row("Slack", "Channel notifications & alerting", "Not connected", false)),
            )
    }

    fn render_account_row(&self, provider: &str, desc: &str, status: &str, active: bool) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p_3()
            .rounded_lg()
            .bg(Theme::bg_surface())
            .border_1()
            .border_color(Theme::border_subtle())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child(provider.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(desc.to_string()),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(Theme::bg_elevated())
                    .border_1()
                    .border_color(if active { Theme::success_green() } else { Theme::border_subtle() })
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if active { Theme::success_green() } else { Theme::text_muted() })
                    .child(status.to_string()),
            )
    }

    fn render_components_gallery(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("Generative UI Components Catalogue"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Components published for LLM autonomous generation in conversation transcripts."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_comp_tile("RecordCard", "Structured key-value record grid with tone badge", "card"))
                    .child(self.render_comp_tile("MetricsCard", "Headline metric with trend arrow and percentage delta", "card"))
                    .child(self.render_comp_tile("ChecklistCard", "Interactive task checklist with completion counters", "card"))
                    .child(self.render_comp_tile("BarChartCard", "Proportional horizontal bar charts with value labels", "chart"))
                    .child(self.render_comp_tile("ProgressChartCard", "Target progress indicators with percentage gauges", "chart"))
                    .child(self.render_comp_tile("ChoiceCard", "Interactive multi-option decision prompts", "decision")),
            )
    }

    fn render_comp_tile(&self, name: &str, desc: &str, kind: &str) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .p_3()
            .rounded_lg()
            .bg(Theme::bg_surface())
            .border_1()
            .border_color(Theme::border_subtle())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_purple())
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(desc.to_string()),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(Theme::bg_elevated())
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(Theme::accent_blue())
                    .child(kind.to_uppercase()),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_subtab = self.selected_subtab.clone();

        div()
            .id("settings-view-root")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .bg(Theme::bg_window())
            // Sub-navigation bar
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_subtab_button("Engine & Local", "engine", &current_subtab, cx))
                            .child(self.render_subtab_button("Standing Instructions", "instructions", &current_subtab, cx))
                            .child(self.render_subtab_button("Connected Accounts", "accounts", &current_subtab, cx))
                            .child(self.render_subtab_button("UI Components Preview", "components", &current_subtab, cx)),
                    ),
            )
            // Content view
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_6()
                    .child(match current_subtab.as_str() {
                        "instructions" => self.render_instructions_settings(),
                        "accounts" => self.render_connected_accounts(),
                        "components" => self.render_components_gallery(),
                        _ => self.render_engine_settings(),
                    }),
            )
    }
}
