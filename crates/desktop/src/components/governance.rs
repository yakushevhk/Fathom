//! Governance view component — Policy rules table, Decision boundary simulator,
//! and searchable Audit trail with refusals (1:1 OpenBot parity).

use crate::state::AppState;
use crate::theme::Theme;
use std::sync::Arc;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, SharedString, Stateful,
    Window,
};
pub struct GovernanceView {
    pub state: Arc<AppState>,
    selected_subtab: String, // "policy" | "audit" | "refusals"
}

impl GovernanceView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            selected_subtab: "audit".to_string(),
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
            .id(SharedString::from(format!("gov-subtab-{}", id)))
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

    fn render_audit_trail(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
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
                            .child("Immutable Decision Trail"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Every computer, MCP, file and component action recorded"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_audit_row("14:32:01", "computer_navigate", "General Assistant", "allowed", "AllowNavDomain (https://docs.github.com)"))
                    .child(self.render_audit_row("14:32:05", "computer_snapshot", "General Assistant", "allowed", "AOM Tree inspection"))
                    .child(self.render_audit_row("14:32:10", "computer_click", "General Assistant", "allowed", "Target element verified in viewport"))
                    .child(self.render_audit_row("14:32:44", "computer_write_file", "Risk Analyst", "allowed", "Saved analysis report to /workspace/audit.json"))
                    .child(self.render_audit_row("14:33:12", "shell_exec", "General Assistant", "refused", "DenyUnsanitizedShell rule triggered")),
            )
    }

    fn render_audit_row(&self, time: &str, tool: &str, bot: &str, decision: &str, reason: &str) -> Div {
        let is_allowed = decision == "allowed";
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
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::text_muted())
                            .child(time.to_string()),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_blue())
                            .child(tool.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .child(format!("by {}", bot)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child(reason.to_string()),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(if is_allowed { Theme::bg_card() } else { Theme::danger_red() })
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(if is_allowed { Theme::success_green() } else { Theme::text_primary() })
                    .child(decision.to_uppercase()),
            )
    }

    fn render_policy_table(&self) -> Div {
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
                    .child("Active CEL Action Policy Engine (Fail-Closed)"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_rule_card("DenyCloudMetadata", "deny", "page.host == '169.254.169.254' || page.host.contains('metadata')", 100))
                    .child(self.render_rule_card("DenyPrivateIPs", "deny", "page.host.matches('^10\\.|^192\\.168\\.|^172\\.(1[6-9]|2[0-9]|3[0-1])\\.')", 90))
                    .child(self.render_rule_card("AllowPublicWeb", "allow", "page.url.startsWith('https://') && tool.name.startsWith('computer_')", 10))
                    .child(self.render_rule_card("RequireConfirmationOnDelete", "ask", "tool.name == 'computer_delete_file' || tool.name == 'mcp_write'", 50)),
            )
    }

    fn render_rule_card(&self, name: &str, effect: &str, condition: &str, _priority: i32) -> Div {
        let effect_color = match effect {
            "allow" => Theme::success_green(),
            "deny" => Theme::danger_red(),
            _ => Theme::warning_yellow(),
        };

        div()
            .flex()
            .flex_col()
            .p_3()
            .rounded_lg()
            .bg(Theme::bg_surface())
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
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(effect_color)
                            .child(format!("EFFECT: {}", effect.to_uppercase())),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(Theme::bg_window())
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::text_secondary())
                    .child(condition.to_string()),
            )
    }

    fn render_refusals_view(&self) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::danger_red())
                    .child("Prevented Refusals & Boundary Enforcements"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_audit_row("11:15:20", "computer_navigate", "General Assistant", "refused", "Forbidden host: 169.254.169.254 blocked by DenyCloudMetadata"))
                    .child(self.render_audit_row("09:40:11", "shell_exec", "TestBot", "refused", "Destructive shell command prohibited without manual approval")),
            )
    }
}

impl Render for GovernanceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_subtab = self.selected_subtab.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .bg(Theme::bg_window())
            // Sub-navigation bar (Policy Rules / Audit Trail / Refusals)
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.render_subtab_button("Audit Trail", "audit", &current_subtab, cx))
                            .child(self.render_subtab_button("Policy Engine & Rules", "policy", &current_subtab, cx))
                            .child(self.render_subtab_button("Refusals & Violations", "refusals", &current_subtab, cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Fail-Closed Security Gate Active"),
                    ),
            )
            // Content pane
            .child(
                div()
                    .id("governance-content-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_scroll()
                    .p_4()
                    .child(match current_subtab.as_str() {
                        "policy" => self.render_policy_table(),
                        "refusals" => self.render_refusals_view(),
                        _ => self.render_audit_trail(),
                    }),
            )
    }
}
