//! Composer component with mid-turn message queueing (`queue.ts`),
//! trigger suggestions popover for `@bot` mentions and `/command` skills,
//! quick prompt presets, and server-directed turn dispatch.

use crate::state::{AppState, ChatMessage};
use crate::theme::Theme;
use gpui::{
    div, prelude::*, ClickEvent, Context, Div, IntoElement, KeyDownEvent, Render,
    SharedString, Window,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub id: String,
    pub text: String,
    pub timestamp: String,
}

pub struct Composer {
    state: Arc<AppState>,
    input_text: String,
    queued_messages: Vec<QueuedMessage>,
    show_agent_suggestions: bool,
    show_command_suggestions: bool,
    focus_handle: gpui::FocusHandle,
}

impl Composer {
    pub fn new(state: Arc<AppState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            input_text: String::new(),
            queued_messages: Vec::new(),
            show_agent_suggestions: false,
            show_command_suggestions: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn submit_turn(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }

        let is_running = self.state.messages.read().iter().rev().any(|m| {
            m.role == "tool" && m.tool_status.as_deref() == Some("running")
        });

        if is_running {
            // Mid-turn queueing (OpenBot queue.ts parity): park message while bot has the turn
            self.queued_messages.push(QueuedMessage {
                id: uuid::Uuid::new_v4().to_string(),
                text: text.clone(),
                timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
            });
            self.input_text.clear();
            cx.notify();
            return;
        }

        // Add user message to transcript immediately
        self.state.add_message(ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: "user".to_string(),
            content: text.clone(),
            thinking: None,
            tool_name: None,
            tool_status: None,
            tool_input: None,
            tool_output: None,
            question: None,
            request_id: None,
            timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
            expanded: false,
        });

        self.input_text.clear();
        self.show_agent_suggestions = false;
        self.show_command_suggestions = false;

        let api = self.state.api.clone();
        let state_clone = self.state.clone();
        cx.spawn(async move |_this, _cx| {
            if let Ok(res) = api.create_session(&text).await {
                *state_clone.active_session_id.write() = Some(res.id);
            }
        }).detach();
        cx.notify();
    }

    fn render_queued_strip(&mut self, cx: &mut Context<Self>) -> Div {
        if self.queued_messages.is_empty() {
            return div();
        }

        let mut strip = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_2()
            .rounded_md()
            .bg(Theme::bg_elevated())
            .border_1()
            .border_color(Theme::warning_yellow())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_xs()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::warning_yellow())
                            .child("⏳ Queued for Next Turn (running after current action):"),
                    )
                    .child(
                        div()
                            .text_color(Theme::text_muted())
                            .child(format!("{} queued", self.queued_messages.len())),
                    ),
            );

        for (ix, queued) in self.queued_messages.clone().into_iter().enumerate() {
            strip = strip.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::text_primary())
                            .child(queued.text.clone()),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("dequeue-btn-{}", queued.id)))
                            .px_2()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::danger_red())
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .cursor_pointer()
                            .child("Undo ✕")
                            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                                if ix < this.queued_messages.len() {
                                    this.queued_messages.remove(ix);
                                    cx.notify();
                                }
                            })),
                    ),
            );
        }

        strip
    }

    fn render_suggestion_popover(&mut self, cx: &mut Context<Self>) -> Div {
        if !self.show_agent_suggestions && !self.show_command_suggestions {
            return div();
        }

        let is_agent = self.show_agent_suggestions;
        let title = if is_agent { "Select Coworker to Hand Work to (@):" } else { "Select Skill or Command (/):" };

        let coworkers = self.state.coworkers.read().clone();
        let skills = self.state.skills.read().clone();

        let items: Vec<(String, String, String)> = if is_agent {
            if !coworkers.is_empty() {
                coworkers.into_iter().map(|c| {
                    (format!("@{}", c.name.replace(' ', "")), c.name, c.role)
                }).collect()
            } else {
                vec![
                    ("@GeneralAssistant".to_string(), "🤖 General Autonomous Worker".to_string(), "Full computer use, file management, search".to_string()),
                    ("@RiskAnalyst".to_string(), "🛡️ Compliance & Risk Officer".to_string(), "CEL rule checks, credential audits, DLP review".to_string()),
                    ("@DevOpsEngineer".to_string(), "🐳 DevOps & Cloud Infrastructure".to_string(), "Container management, health checks, shell runner".to_string()),
                ]
            }
        } else {
            let mut cmds = vec![
                ("/browser".to_string(), "🌐 Launch Computer Session".to_string(), "Open Chromium container and navigate".to_string()),
                ("/routine".to_string(), "⚡ Standing Routine".to_string(), "Register scheduled cron automation".to_string()),
                ("/vault".to_string(), "🔑 Access Credential".to_string(), "Inject encrypted AES-256 secret token".to_string()),
                ("/audit".to_string(), "📜 View Audit Ledger".to_string(), "Inspect refusal reasons and action logs".to_string()),
                ("/skill-creator".to_string(), "🛠️ Create New Skill".to_string(), "Author and register coworker skill".to_string()),
            ];
            for s in skills {
                cmds.push((format!("/{}", s.id), s.name, s.description));
            }
            cmds
        };

        div()
            .flex()
            .flex_col()
            .p_2()
            .rounded_lg()
            .bg(Theme::bg_card())
            .border_1()
            .border_color(Theme::border_focus())
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(Theme::accent_purple())
                    .child(title),
            )
            .children(items.into_iter().map(|(trigger, name, desc)| {
                let trigger_str = trigger.clone();
                div()
                    .id(SharedString::from(format!("suggestion-item-{}", trigger)))
                    .flex()
                    .items_center()
                    .justify_between()
                    .p_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_elevated_hover()))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_primary())
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::text_muted())
                                    .child(desc),
                            ),
                    )
                    .child(
                        div()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .font_family("JetBrains Mono")
                            .text_color(Theme::accent_blue())
                            .child(trigger),
                    )
                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                        this.input_text = format!("{} ", trigger_str);
                        this.show_agent_suggestions = false;
                        this.show_command_suggestions = false;
                        cx.notify();
                    }))
            }))
    }
}

impl Render for Composer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_running = self.state.messages.read().iter().rev().any(|m| {
            m.role == "tool" && m.tool_status.as_deref() == Some("running")
        });

        div()
            .track_focus(&self.focus_handle)
            .flex()
            .flex_col()
            .w_full()
            .border_t_1()
            .border_color(Theme::border_subtle())
            .bg(Theme::bg_surface())
            .p_3()
            .gap_2()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let keystroke = &event.keystroke.key;
                if keystroke == "enter" {
                    let text = this.input_text.clone();
                    this.submit_turn(text, cx);
                } else if keystroke == "backspace" {
                    this.input_text.pop();
                    cx.notify();
                } else if keystroke == "@" {
                    this.input_text.push('@');
                    this.show_agent_suggestions = true;
                    this.show_command_suggestions = false;
                    cx.notify();
                } else if keystroke == "/" && this.input_text.is_empty() {
                    this.input_text.push('/');
                    this.show_command_suggestions = true;
                    this.show_agent_suggestions = false;
                    cx.notify();
                } else if keystroke == "escape" {
                    this.show_agent_suggestions = false;
                    this.show_command_suggestions = false;
                    cx.notify();
                } else if keystroke == "space" {
                    this.input_text.push(' ');
                    cx.notify();
                } else if keystroke.chars().count() == 1 {
                    if let Some(ch) = keystroke.chars().next() {
                        this.input_text.push(ch);
                        cx.notify();
                    }
                }
            }))
            // Mid-turn Queued Strip (queue.ts parity)
            .child(self.render_queued_strip(cx))
            // Suggestion Popover (@ and / trigger menu)
            .child(self.render_suggestion_popover(cx))
            // Quick Command Chips (@bot, /skill, /routine)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(
                        div()
                            .text_color(Theme::text_muted())
                            .child("Quick Triggers:"),
                    )
                    .child(
                        div()
                            .id("chip-agent-ga")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::accent_blue())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("@GeneralAssistant")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.input_text = "@GeneralAssistant ".to_string();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("chip-agent-ra")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::accent_purple())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("@RiskAnalyst")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.input_text = "@RiskAnalyst ".to_string();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("chip-cmd-browser")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::success_green())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("/browser")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.input_text = "/browser https://github.com".to_string();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("chip-cmd-routine")
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(Theme::bg_elevated())
                            .text_xs()
                            .text_color(Theme::warning_yellow())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::bg_elevated_hover()).text_color(Theme::text_primary()))
                            .child("/routine")
                            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                this.input_text = "/routine every weekday at 9am check competitor updates".to_string();
                                cx.notify();
                            })),
                    ),
            )
            // Composer main box & action buttons
            .child(
                div()
                    .flex()
                    .items_center()
                    .p_2()
                    .rounded_lg()
                    .bg(Theme::bg_elevated())
                    .border_1()
                    .border_color(Theme::border_focus())
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(Theme::text_primary())
                            .child(
                                if self.input_text.is_empty() {
                                    div()
                                        .text_color(Theme::text_muted())
                                        .child("Hand work to coworker or type @ for bots, / for skills (e.g. 'Investigate pricing on G2')... [Type and press Enter]")
                                } else {
                                    div()
                                        .font_family("JetBrains Mono")
                                        .child(format!("{}▌", self.input_text))
                                },
                            ),
                    )
                    // Action Buttons (Send / Steer / Stop)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("composer-send-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(if is_running { Theme::warning_yellow() } else { Theme::accent_purple() })
                                    .child(if is_running { "Queue Turn ↵" } else { "Send Turn ↵" })
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let text = if this.input_text.is_empty() {
                                            "Investigate competitors and update knowledge base".to_string()
                                        } else {
                                            this.input_text.clone()
                                        };
                                        this.submit_turn(text, cx);
                                    })),
                            )
                            .children(if is_running {
                                Some(
                                    div()
                                        .id("composer-stop-btn")
                                        .px_3()
                                        .py_1p5()
                                        .rounded_md()
                                        .bg(Theme::danger_red())
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(Theme::text_primary())
                                        .cursor_pointer()
                                        .child("🛑 Stop Turn")
                                        .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                            let session_id = this.state.active_session_id.read().clone();
                                            let api = this.state.api.clone();
                                            if let Some(id) = session_id {
                                                cx.spawn(async move |_this, _cx| {
                                                    let _ = api.cancel_session(&id).await;
                                                }).detach();
                                            }
                                            this.state.add_message(ChatMessage {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                role: "system".to_string(),
                                                content: "🛑 Turn stopped by human operator.".to_string(),
                                                thinking: None,
                                                tool_name: None,
                                                tool_status: None,
                                                tool_input: None,
                                                tool_output: None,
                                                question: None,
                                                request_id: None,
                                                timestamp: chrono::Utc::now().format("%H:%M:%S").to_string(),
                                                expanded: false,
                                            });
                                            cx.notify();
                                        })),
                                )
                            } else {
                                None
                            })
                            .child(
                                div()
                                    .id("composer-steer-btn")
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(Theme::bg_card())
                                    .border_1()
                                    .border_color(Theme::border_subtle())
                                    .text_xs()
                                    .text_color(Theme::text_secondary())
                                    .cursor_pointer()
                                    .hover(|s| s.bg(Theme::bg_elevated()).text_color(Theme::text_primary()))
                                    .child("Steer [s]")
                                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                                        let text = if this.input_text.is_empty() {
                                            "Prioritize open-source repositories over proprietary vendors".to_string()
                                        } else {
                                            this.input_text.clone()
                                        };
                                        let session_id = this.state.active_session_id.read().clone();
                                        let api = this.state.api.clone();
                                        if let Some(id) = session_id {
                                            cx.spawn(async move |_this, _cx| {
                                                let _ = api.steer_session(&id, &text).await;
                                            }).detach();
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }
}
