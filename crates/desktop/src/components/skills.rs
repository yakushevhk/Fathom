//! Skills & Tools component — reusable agent instructions and tool capabilities.

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, Div, IntoElement, Render, Window,
};
use std::sync::Arc;

pub struct SkillsView {
    state: Arc<AppState>,
}

impl SkillsView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    fn render_skill_card(&self, name: &str, desc: &str, tools: &[&str]) -> Div {
        let mut tool_tags = div().flex().items_center().gap_1p5();
        for tool in tools {
            tool_tags = tool_tags.child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(Theme::bg_elevated())
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::accent_blue())
                    .child(tool.to_string()),
            );
        }

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
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(Theme::text_primary())
                    .child(name.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_secondary())
                    .child(desc.to_string()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mt_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_muted())
                            .child("Tools:"),
                    )
                    .child(tool_tags),
            )
    }
}

impl Render for SkillsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let skills = self.state.skills.read().clone();

        div()
            .id("skills-view-root")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_scroll()
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
                            .gap_1()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(Theme::text_primary())
                                    .child("Skills & Tool Authoring"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("Reusable instructions that narrow tool sets and grant specific capabilities to coworkers."),
                            ),
                    )
                    .child(
                        div()
                            .id("new-skill-btn")
                            .px_3()
                            .py_1p5()
                            .rounded_md()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::accent_blue()))
                            .child("+ Author Skill")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.state.skills.write().push(crate::api::Skill {
                                    id: format!("skill_{}", &uuid::Uuid::new_v4().to_string()[..6]),
                                    name: "Lead Qualification".to_string(),
                                    description: "Extract verified business emails and check LinkedIn profiles".to_string(),
                                    tools: vec!["lead_finder".to_string(), "enrich_person".to_string(), "verify_social".to_string()],
                                    instructions: "Evaluate candidate against ICP criteria and verify deliverability before recording.".to_string(),
                                });
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        if skills.is_empty() {
                            div().flex().flex_col().gap_3()
                                .child(self.render_skill_card("Web Research & Synthesis", "Deep research across Google, Exa, Tavily and Firecrawl", &["exa_search", "firecrawl_scrape", "tavily_search"]))
                                .child(self.render_skill_card("Browser Automation & Extraction", "Drive Playwright stealth browser, click buttons, fill forms and download assets", &["computer_navigate", "computer_click", "computer_type", "computer_snapshot"]))
                                .child(self.render_skill_card("Security Audit & Vulnerability Triage", "Scan dependencies, inspect CVEs, verify AST patterns and check licenses", &["ast_edit", "audit_log", "compiler_check"]))
                                .child(self.render_skill_card("Google Drive & Notion Connector", "Read documents, spreadsheets and sync workspaces over user OAuth", &["gdrive_read", "notion_sync", "doc_export"]))
                        } else {
                            let mut list = div().flex().flex_col().gap_3();
                            for s in skills {
                                let tools_str: Vec<&str> = s.tools.iter().map(|t| t.as_str()).collect();
                                list = list.child(self.render_skill_card(&s.name, &s.description, &tools_str));
                            }
                            list
                        },
                    ),
            )
    }
}
