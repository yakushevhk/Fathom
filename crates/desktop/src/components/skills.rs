//! Skills & Tools component — reusable agent instructions, tool capabilities,
//! interactive skill authoring modal, tool subset picker, and coworker skill binding.

use crate::state::AppState;
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, Div, IntoElement, Render, SharedString, Window,
};
use std::sync::Arc;

pub struct SkillsView {
    state: Arc<AppState>,
    show_add_modal: bool,
    new_name: String,
    new_desc: String,
    new_tools: Vec<String>,
    new_instructions: String,
}

impl SkillsView {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            show_add_modal: false,
            new_name: "Web Data Extraction".to_string(),
            new_desc: "Scrape pages, extract pricing tables, and save structured records".to_string(),
            new_tools: vec!["computer_navigate".to_string(), "computer_read".to_string(), "file_write".to_string()],
            new_instructions: "Navigate to designated URL, extract tabular data, convert to CSV, and save to workspace disk.".to_string(),
        }
    }

    fn render_add_skill_modal(&self, cx: &mut Context<Self>) -> Div {
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
                    .child("Author New Coworker Skill & Tool Subset"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::text_muted())
                    .child("Skills package targeted instructions and restrict the tool set available during execution."),
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
                                    .child("Skill Name:"),
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
                                    .child("Granted Tool Subset:"),
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
                                    .text_color(Theme::accent_blue())
                                    .child(self.new_tools.join(", ")),
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
                            .child("Detailed Instructions & Prompt Contract:"),
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
                            .font_family("JetBrains Mono")
                            .text_color(Theme::text_secondary())
                            .child(self.new_instructions.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("cancel-skill-btn")
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
                            .id("confirm-skill-btn")
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(Theme::accent_purple())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .child("🧩 Publish Skill")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.state.skills.write().push(crate::api::Skill {
                                    id: format!("skill_{}", &uuid::Uuid::new_v4().to_string()[..6]),
                                    name: this.new_name.clone(),
                                    description: this.new_desc.clone(),
                                    tools: this.new_tools.clone(),
                                    instructions: this.new_instructions.clone(),
                                });
                                this.show_add_modal = false;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_skill_card(&self, id: &str, name: &str, desc: &str, tools: &[String], cx: &mut Context<Self>) -> Div {
        let s_id = id.to_string();
        let mut tool_tags = div().flex().flex_wrap().gap_1p5();
        for tool in tools {
            tool_tags = tool_tags.child(
                div()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(Theme::bg_elevated())
                    .text_xs()
                    .font_family("JetBrains Mono")
                    .text_color(Theme::accent_blue())
                    .child(tool.clone()),
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
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_primary())
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("delete-skill-{}", id)))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::danger_red())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::danger_red()).text_color(Theme::text_primary()))
                            .child("Delete ✕")
                            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                this.state.skills.write().retain(|s| s.id != s_id);
                                cx.notify();
                            })),
                    ),
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
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(Theme::text_muted())
                            .child("Scoped Tools:"),
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
                                    .child("Skills & Tool Scopes Catalogue"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child("Reusable domain expertise packaging prompt guidelines with precise tool boundaries."),
                            ),
                    )
                    .child(
                        div()
                            .id("author-skill-btn")
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
                                this.show_add_modal = !this.show_add_modal;
                                cx.notify();
                            })),
                    ),
            )
            .children(if self.show_add_modal {
                Some(self.render_add_skill_modal(cx))
            } else {
                None
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        if skills.is_empty() {
                            div().flex().flex_col().gap_3()
                                .child(self.render_skill_card("skill_1", "Web Research & Lead Sourcing", "Search public directories, verify email contacts and extract company profiles", &["web_search".into(), "web_scrape".into(), "lead_finder".into()], cx))
                                .child(self.render_skill_card("skill_2", "Security Boundary Review", "Evaluate endpoints, inspect certificates, and verify compliance with CEL action policies", &["policy_check".into(), "audit_query".into(), "cert_inspect".into()], cx))
                                .child(self.render_skill_card("skill_3", "Codebase Ast Refactoring", "Perform structural AST rewrites and diagnostics using language server intelligence", &["ast_edit".into(), "lsp_query".into(), "compiler_check".into()], cx))
                        } else {
                            let mut list = div().flex().flex_col().gap_3();
                            for s in skills {
                                list = list.child(self.render_skill_card(&s.id, &s.name, &s.description, &s.tools, cx));
                            }
                            list
                        },
                    ),
            )
    }
}
