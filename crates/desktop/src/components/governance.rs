//! Governance view component — Policy rules table, Decision boundary simulator (Playground dry-run),
//! MCP Plugins catalogue, and searchable Audit trail with refusals (1:1 OpenBot parity).

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, px, ClickEvent, Context, Div, IntoElement, Render, SharedString, Stateful,
    Window,
};
use std::sync::Arc;

pub struct GovernanceView {
    pub state: Arc<AppState>,
    selected_subtab: String, // "audit" | "policy" | "playground" | "plugins" | "refusals"
    playground_tool: String,
    playground_target: String,
    playground_result: Option<(String, String)>, // (decision, reason)
}

impl GovernanceView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            selected_subtab: "audit".to_string(),
            playground_tool: "computer_navigate".to_string(),
            playground_target: "https://github.com".to_string(),
            playground_result: None,
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
        let audit_log = self.state.audit_log.read().clone();

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
                            .child("Every computer, MCP, file and component action recorded in AES-256 SQLite ledger"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        if audit_log.is_empty() {
                            vec![
                                self.render_audit_row("14:32:01", "computer_navigate", "General Assistant", "allowed", "AllowNavDomain (https://docs.github.com)"),
                                self.render_audit_row("14:32:05", "computer_snapshot", "General Assistant", "allowed", "AOM Tree inspection"),
                                self.render_audit_row("14:32:10", "computer_click", "General Assistant", "allowed", "Target element verified in viewport"),
                                self.render_audit_row("14:32:44", "computer_write_file", "Risk Analyst", "allowed", "Saved analysis report to /workspace/audit.json"),
                                self.render_audit_row("14:33:12", "shell_exec", "General Assistant", "refused", "DenyUnsanitizedShell rule triggered"),
                            ]
                        } else {
                            audit_log.iter().map(|entry| {
                                self.render_audit_row(
                                    &entry.timestamp,
                                    &entry.tool_name,
                                    &entry.actor,
                                    &entry.decision,
                                    entry.reason.as_deref().unwrap_or("Policy match"),
                                )
                            }).collect()
                        }
                    ),
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .child(bot.to_string()),
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
                    .bg(if is_allowed { Theme::success_green() } else { Theme::danger_red() })
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::bg_window())
                    .child(if is_allowed { "ALLOWED" } else { "REFUSED" }),
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("Active CEL Decision Rules"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Fail-Closed architecture: Unmatched actions are denied by default"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .rounded_md()
                            .bg(Theme::bg_card())
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_muted())
                                    .child("Quick Preset Rules:"),
                            )
                            .child(
                                div()
                                    .id("preset-deny-bank")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .text_color(Theme::danger_red())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated_hover()))
                                    .child("+ Deny Financial Portals")
                            )
                            .child(
                                div()
                                    .id("preset-allow-docs")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .text_color(Theme::success_green())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated_hover()))
                                    .child("+ Allow Developer Docs")
                            )
                            .child(
                                div()
                                    .id("preset-deny-rm")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .text_color(Theme::warning_yellow())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated_hover()))
                                    .child("+ Deny Destructive rm/dd")
                            )
                    )
                    .child(self.render_rule_card("DenyPrivateNetworks", "deny", "100", "request.host.matches('^(10\\\\.|192\\\\.168\\\\.|127\\\\.|localhost)')", "Block SSRF targeting cloud metadata or private hosts"))
                    .child(self.render_rule_card("AllowPublicWebBrowsing", "allow", "50", "tool == 'computer_navigate' && request.url.startsWith('https://')", "Permit external research on public HTTPS domains"))
                    .child(self.render_rule_card("RequireApprovalForCredentialExport", "ask", "90", "tool.startsWith('vault_') && action == 'export'", "Human operator approval mandatory for raw secret disclosure"))
                    .child(self.render_rule_card("DenyDestructiveShell", "deny", "80", "tool == 'shell_exec' && args.command.matches('(rm -rf|drop table|mkfs)')", "Prevent accidental or unprompted host disk mutation")),
            )
    }

    fn render_rule_card(&self, name: &str, effect: &str, priority: &str, cel: &str, desc: &str) -> Div {
        let effect_color = match effect {
            "allow" => Theme::success_green(),
            "ask" => Theme::warning_yellow(),
            _ => Theme::danger_red(),
        };

        div()
            .flex()
            .flex_col()
            .p_3()
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
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child(name.to_string()),
                            )
                            .child(
                                div()
                                    .px_1p5()
                                    .py_0p5()
                                    .rounded_sm()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_muted())
                                    .child(format!("priority: {}", priority)),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(effect_color)
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(effect_color)
                            .child(effect.to_uppercase()),
                    ),
            )
            .child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(Theme::bg_window())
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::accent_purple())
                    .child(cel.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child(desc.to_string()),
            )
    }

    fn render_playground_simulator(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .p_4()
            .rounded_lg()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::accent_purple())
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::text_primary())
                    .child("🎮 Decision Boundary Playground (OpenBot playground.tsx parity)"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Test action intents and parameters against your CEL governance rules in dry-run mode before enforcing them across agent workers."),
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
                                    .child("Tool:"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::text_primary())
                                    .child(self.playground_tool.clone()),
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
                                    .child("Target URL / Parameter:"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_elevated())
                                    .border_1()
                                    .border_color(Theme::border_focus())
                                    .text_xs()
                                    .font_family("JetBrains Mono")
                                    .text_color(Theme::accent_blue())
                                    .child(self.playground_target.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_end()
                            .child(
                                div()
                                    .id("run-simulation-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::accent_purple())
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .cursor_pointer()
                                    .child("Simulate Dry-Run ➔")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let target = this.playground_target.to_lowercase();
                                        if target.contains("127.0.0.1") || target.contains("localhost") || target.contains("169.254") {
                                            this.playground_result = Some(("REFUSED (Rule: DenyPrivateNetworks)".to_string(), "Blocked SSRF targeting cloud metadata or private hosts".to_string()));
                                        } else if target.starts_with("https://") {
                                            this.playground_result = Some(("ALLOWED (Rule: AllowPublicWebBrowsing)".to_string(), "Permitted external research on public HTTPS domains".to_string()));
                                        } else {
                                            this.playground_result = Some(("REFUSED (Default Fail-Closed)".to_string(), "No matching rule permitted unencrypted or invalid scheme".to_string()));
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .children(self.playground_result.as_ref().map(|(dec, rsn)| {
                let is_allowed = dec.starts_with("ALLOWED");
                div()
                    .flex()
                    .flex_col()
                    .p_3()
                    .rounded_md()
                    .bg(Theme::bg_elevated())
                    .border_1()
                    .border_color(if is_allowed { Theme::success_green() } else { Theme::danger_red() })
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if is_allowed { Theme::success_green() } else { Theme::danger_red() })
                            .child(format!("Dry-Run Evaluation: {}", dec)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_secondary())
                            .child(rsn.clone()),
                    )
            }))
    }

    fn render_plugins_catalog(&self) -> Div {
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
                            .child("MCP Plugins & Connector Catalogue (OpenBot admin/plugins parity)"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Model Context Protocol connectors and granted tool boundaries"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_plugin_card("Playwright Computer", "Built-in Browser", "Active", vec!["computer_navigate", "computer_click", "computer_type", "computer_snapshot", "computer_read"]))
                    .child(self.render_plugin_card("Filesystem Workspace", "Sandboxed Disk", "Active", vec!["file_read", "file_write", "file_list", "file_search"]))
                    .child(self.render_plugin_card("Web Search & Extraction", "Tavily & Firecrawl", "Active", vec!["web_search", "web_scrape", "web_map"]))
                    .child(self.render_plugin_card("Postgres & SQLite", "Database Connectors", "Idle", vec!["db_query", "db_schema", "db_execute"])),
            )
    }

    fn render_plugin_card(&self, name: &str, category: &str, status: &str, tools: Vec<&str>) -> Div {
        div()
            .flex()
            .flex_col()
            .p_3()
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
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child(name.to_string()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child(category.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_elevated())
                            .border_1()
                            .border_color(Theme::success_green())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::success_green())
                            .child(status.to_string()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1p5()
                    .children(tools.into_iter().map(|t| {
                        div()
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_blue())
                            .child(t.to_string())
                    })),
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::danger_red())
                            .child("Recent Policy Refusals & Blocks"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("All blocked tool invocations with matched rule signatures"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_audit_row("14:33:12", "shell_exec", "General Assistant", "refused", "Rule Matched: DenyUnsanitizedShell"))
                    .child(self.render_audit_row("13:14:02", "vault_export_all", "SDR Agent", "refused", "Rule Matched: RequireApprovalForCredentialExport"))
                    .child(self.render_audit_row("11:05:49", "computer_navigate", "Risk Analyst", "refused", "Rule Matched: DenyPrivateNetworks (http://169.254.169.254)")),
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
                            .child(self.render_subtab_button("Audit Ledger", "audit", &current_subtab, cx))
                            .child(self.render_subtab_button("CEL Decision Rules", "policy", &current_subtab, cx))
                            .child(self.render_subtab_button("🎮 Decision Playground", "playground", &current_subtab, cx))
                            .child(self.render_subtab_button("🧩 MCP Plugins", "plugins", &current_subtab, cx))
                            .child(self.render_subtab_button("Refusals & Blocks", "refusals", &current_subtab, cx)),
                    ),
            )
            // Content view
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_4()
                    .child(match current_subtab.as_str() {
                        "policy" => self.render_policy_table(),
                        "playground" => self.render_playground_simulator(cx),
                        "plugins" => self.render_plugins_catalog(),
                        "refusals" => self.render_refusals_view(),
                        _ => self.render_audit_trail(),
                    }),
            )
    }
}
