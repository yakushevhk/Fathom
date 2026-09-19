//! Root view: sidebar roster + chat pane + dialogs, all in GPUI.

use crate::input::{Submitted, TextInput};
use crate::state::{AppData, Dialog};
use crate::theme as t;
use fathom_core::types::*;
use gpui::{
    div, prelude::*, px, rgb, rgba, App, Context, Entity, FocusHandle, Focusable, MouseButton,
    ScrollHandle, SharedString, Window,
};
use std::collections::HashSet;

/// Pending model-picker dropdown state lives in RootView.
pub struct RootView {
    data: Entity<AppData>,
    composer: Entity<TextInput>,
    scroll: ScrollHandle,
    focus: FocusHandle,
    /// message id → expanded thinking segments
    expanded_thinking: HashSet<String>,
    /// Model dropdown open?
    model_menu: bool,
    /// Dialog field entities — created per dialog open.
    dlg_name: Entity<TextInput>,
    dlg_model: Entity<TextInput>,
    dlg_soul: Entity<TextInput>,
    dlg_cwd: Entity<TextInput>,
    dlg_engine: EngineKind,
    /// Settings dialog: engine path inputs.
    dlg_engine_inputs: Vec<(EngineKind, Entity<TextInput>)>,
}

impl RootView {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>, data: Entity<AppData>) -> Self {
        let composer = cx.new(|cx| {
            let mut i = TextInput::new(cx, "Message");
            i.submit_on_enter = true;
            i
        });
        cx.subscribe(&composer, |this: &mut Self, input, ev: &Submitted, cx| {
            let text = ev.text.clone();
            input.update(cx, |i, _| i.reset());
            this.send_message(text, cx);
        })
        .detach();

        // Re-render on any data change (bus pump notifies the AppData entity).
        cx.observe(&data, |_, _, cx| cx.notify()).detach();

        let mk = |cx: &mut Context<Self>, ph: &str, submit: bool| -> Entity<TextInput> {
            cx.new(|cx| {
                let mut i = TextInput::new(cx, ph.to_string());
                i.submit_on_enter = submit;
                i
            })
        };
        let dlg_name = mk(cx, "Bot name", false);
        let dlg_model = mk(cx, "Model (optional)", false);
        let dlg_soul = mk(cx, "Persona / SOUL.md instructions", false);
        let dlg_cwd = mk(cx, "Working directory (optional)", false);

        let mut view = Self {
            data,
            composer,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            expanded_thinking: HashSet::new(),
            model_menu: false,
            dlg_name,
            dlg_model,
            dlg_soul,
            dlg_cwd,
            dlg_engine: EngineKind::Claude,
            dlg_engine_inputs: vec![],
        };
        // Open the most recent conversation on launch.
        if let Some(id) = view.data.read(cx).bots.first().map(|b| b.id.clone()) {
            view.select_bot(id, cx);
        }
        view
    }

    fn send_message(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() {
            return;
        }
        let (bot_id, thread_id) = {
            let d = self.data.read(cx);
            match (d.active_bot.clone(), d.active_thread()) {
                (Some(b), Some(t)) => (b, t),
                _ => return,
            }
        };
        let harness = self.data.read(cx).harness.clone();
        let rt = self.data.read(cx).tokio.clone();
        rt.spawn(async move {
            let _ =
                fathom_harness::sessions::start_turn(&harness, &bot_id, &thread_id, &text, None)
                    .await;
        });
        cx.notify();
    }

    fn select_bot(&mut self, bot_id: String, cx: &mut Context<Self>) {
        {
            let harness = self.data.read(cx).harness.clone();
            self.data.update(cx, |d, _| {
                d.active_bot = Some(bot_id.clone());
                if let Ok(t) = harness.store.direct_thread(&bot_id) {
                    d.threads.insert(bot_id.clone(), t.id.clone());
                    let msgs = harness
                        .store
                        .list_messages(&t.id, 200, None)
                        .unwrap_or_default();
                    d.messages.insert(t.id, msgs);
                }
            });
        }
        let rt = self.data.read(cx).tokio.clone();
        let harness = self.data.read(cx).harness.clone();
        let bid = bot_id.clone();
        rt.spawn(async move {
            let _ = harness.store.mark_read(&bid);
            if let Ok(Some(b)) = harness.store.get_bot(&bid) {
                let _ = harness.bus.send(ServerEvent::BotUpsert { bot: b });
            }
        });
        self.composer.update(cx, |i, _cx| i.reset());
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn stop_turn(&mut self, cx: &mut Context<Self>) {
        let Some(thread_id) = self.data.read(cx).active_thread() else {
            return;
        };
        let harness = self.data.read(cx).harness.clone();
        let rt = self.data.read(cx).tokio.clone();
        rt.spawn(async move {
            fathom_harness::sessions::abort_thread(&harness, &thread_id).await;
        });
        cx.notify();
    }

    fn resolve_approval(&mut self, id: String, decision: &'static str, cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        let rt = self.data.read(cx).tokio.clone();
        rt.spawn(async move {
            fathom_harness::sessions::resolve_approval(&harness, &id, decision, None).await;
        });
        cx.notify();
    }

    fn create_bot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.dlg_name.read(cx).content.to_string();
        if name.trim().is_empty() {
            return;
        }
        let model = self.dlg_model.read(cx).content.to_string();
        let soul = self.dlg_soul.read(cx).content.to_string();
        let cwd = self.dlg_cwd.read(cx).content.to_string();
        let harness = self.data.read(cx).harness.clone();
        let bot = Bot {
            id: fathom_core::new_id("bot"),
            name: name.trim().to_string(),
            engine: self.dlg_engine,
            model: if model.trim().is_empty() {
                None
            } else {
                Some(model.trim().to_string())
            },
            soul: soul.clone(),
            avatar_seed: name.bytes().fold(0u8, |a, b| a.wrapping_add(b)),
            cwd: if cwd.trim().is_empty() {
                None
            } else {
                Some(cwd.trim().to_string())
            },
            auto_approve: false,
            archived: false,
            computer_id: None,
            created_at: now_ms(),
            updated_at: now_ms(),
            last_message: None,
            last_activity_at: Some(now_ms()),
            unread: 0,
            working: false,
        };
        let _ = harness.store.upsert_bot(&bot);
        let _ = harness.store.write_soul(&bot.id, &soul);
        let _ = harness.store.direct_thread(&bot.id);
        let _ = harness
            .bus
            .send(ServerEvent::BotUpsert { bot: bot.clone() });
        self.data.update(cx, |d, _| d.dialog = None);
        self.select_bot(bot.id, cx);
        window.focus(&self.composer.read(cx).focus_handle);
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        // Build engine path inputs lazily.
        if self.dlg_engine_inputs.is_empty() {
            let harness = self.data.read(cx).harness.clone();
            self.dlg_engine_inputs = EngineKind::ALL
                .iter()
                .map(|k| {
                    let cfg = harness.engine_config(*k);
                    let input = cx.new(|cx| {
                        let mut i = TextInput::new(
                            cx,
                            match k {
                                EngineKind::Grok | EngineKind::OpenAiCompat => "API base URL",
                                _ => "CLI binary path (blank = PATH)",
                            },
                        );
                        i.submit_on_enter = false;
                        i
                    });
                    let initial = cfg.cli.clone().or(cfg.url.clone()).unwrap_or_default();
                    input.update(cx, |i, cx| i.set_text(&initial, cx));
                    (*k, input)
                })
                .collect();
        }
        self.data
            .update(cx, |d, _| d.dialog = Some(Dialog::Settings));
        cx.notify();
    }

    fn save_engine(&mut self, kind: EngineKind, cx: &mut Context<Self>) {
        let Some((_, input)) = self.dlg_engine_inputs.iter().find(|(k, _)| *k == kind) else {
            return;
        };
        let val = input.read(cx).content.to_string();
        let harness = self.data.read(cx).harness.clone();
        let mut cfg = harness.engine_config(kind);
        if kind.is_cli() {
            cfg.cli = if val.trim().is_empty() {
                None
            } else {
                Some(val.trim().to_string())
            };
        } else {
            cfg.url = if val.trim().is_empty() {
                None
            } else {
                Some(val.trim().to_string())
            };
        }
        let _ = harness.set_engine_config(kind, &cfg);
        let rt = self.data.read(cx).tokio.clone();
        rt.spawn(async move {
            let engines = harness.engine_statuses().await;
            let _ = harness.bus.send(ServerEvent::Engines { engines });
        });
        cx.notify();
    }

    fn set_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        let Some(bot_id) = self.data.read(cx).active_bot.clone() else {
            return;
        };
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(mut bot)) = harness.store.get_bot(&bot_id) {
            bot.model = model;
            bot.updated_at = now_ms();
            let _ = harness.store.upsert_bot(&bot);
            let _ = harness.bus.send(ServerEvent::BotUpsert { bot });
        }
        self.model_menu = false;
        cx.notify();
    }

    fn avatar_color(seed: u8) -> u32 {
        const PALETTE: [u32; 8] = [
            0x3b82f6, 0x8b5cf6, 0x22d3ee, 0x34d399, 0xfbbf24, 0xf472b6, 0xf87171, 0xa3e635,
        ];
        PALETTE[(seed as usize) % PALETTE.len()]
    }

    // ---- rendering ----------------------------------------------------------

    fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx);
        let active = data.active_bot.clone();
        let mut rows = div().flex().flex_col().gap_1();
        for bot in &data.bots {
            let bid = bot.id.clone();
            let selected = active.as_deref() == Some(&bot.id);
            let unread = bot.unread;
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("bot-{}", bot.id)))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .mx_2()
                    .rounded_md()
                    .cursor_pointer()
                    .when(selected, |d| d.bg(rgb(t::BG_HOVER)))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.select_bot(bid.clone(), cx);
                        }),
                    )
                    .child(self.avatar(bot))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(t::TEXT))
                                            .child(bot.name.clone()),
                                    )
                                    .when(bot.working, |d| {
                                        d.child(div().text_xs().text_color(rgb(t::CYAN)).child("●"))
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(t::TEXT_FAINT))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(
                                        bot.last_message
                                            .clone()
                                            .unwrap_or_else(|| bot.engine.label().to_string()),
                                    ),
                            ),
                    )
                    .when(unread > 0, |d| {
                        d.child(
                            div()
                                .bg(rgb(t::ACCENT))
                                .rounded_full()
                                .px_2()
                                .py_0p5()
                                .text_xs()
                                .text_color(gpui::white())
                                .child(format!("{unread}")),
                        )
                    }),
            );
        }

        div()
            .flex()
            .flex_col()
            .w(px(250.))
            .h_full()
            .bg(rgb(t::BG_PANEL))
            .border_r_1()
            .border_color(rgb(t::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(t::BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .size_5()
                                    .rounded_sm()
                                    .bg(rgb(t::ACCENT))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(gpui::white())
                                    .child("F"),
                            )
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(t::TEXT))
                                    .child("fathom"),
                            ),
                    )
                    .child(self.icon_btn("new-bot", "+", cx, |this, cx| {
                        this.data.update(cx, |d, _| d.dialog = Some(Dialog::NewBot));
                        cx.notify();
                    })),
            )
            .child(div().flex_1().overflow_hidden().py_2().child(rows))
            .child(
                div()
                    .border_t_1()
                    .border_color(rgb(t::BORDER))
                    .px_4()
                    .py_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("local-first · 127.0.0.1:8799"),
                    )
                    .child(self.icon_btn("settings", "⚙", cx, |this, cx| this.open_settings(cx))),
            )
    }

    fn avatar(&self, bot: &Bot) -> impl IntoElement {
        let color = Self::avatar_color(bot.avatar_seed);
        let initial: String = bot
            .name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .collect();
        div()
            .size_8()
            .rounded_full()
            .bg(rgb(color))
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(gpui::white())
            .child(initial)
    }

    fn icon_btn(
        &self,
        id: &str,
        label: &str,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md()
            .text_color(rgb(t::TEXT_DIM))
            .hover(|d| d.bg(rgb(t::BG_HOVER)).text_color(rgb(t::TEXT)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| f(this, cx)),
            )
            .child(label.to_string())
    }

    fn chat_header(&mut self, bot: &Bot, cx: &mut Context<Self>) -> impl IntoElement {
        let model_label = bot.model.clone().unwrap_or_else(|| "default".into());
        let engine = bot.engine;
        let bid = bot.id.clone();
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(rgb(t::BORDER))
            .child(self.avatar(bot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_base()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(t::TEXT))
                            .child(bot.name.clone()),
                    )
                    .child(
                        div().flex().gap_2().items_center().child(
                            div()
                                .id("engine-badge")
                                .text_xs()
                                .text_color(rgb(t::CYAN))
                                .child(format!("{} · {}", engine.label(), model_label)),
                        ),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("model-picker")
                    .cursor_pointer()
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .text_xs()
                    .text_color(rgb(t::TEXT_DIM))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.model_menu = !this.model_menu;
                            cx.notify();
                        }),
                    )
                    .child("model ▾"),
            )
            .child(self.icon_btn("computer-btn", "🖥", cx, move |this, cx| {
                let bid = bid.clone();
                this.data
                    .update(cx, |d, _| d.dialog = Some(Dialog::Computer(bid.clone())));
                cx.notify();
            }))
            .when(bot.working, |d| {
                d.child(self.icon_btn("stop-btn", "■ stop", cx, |this, cx| this.stop_turn(cx)))
            })
            .child(self.icon_btn("profile-btn", "✎", cx, {
                let bid = bot.id.clone();
                move |this, cx| {
                    // Prefill the persona field from the bot before opening.
                    if let Some(b) = this.data.read(cx).bot(&bid) {
                        let soul = b.soul.clone();
                        this.dlg_soul.update(cx, |i, cx| i.set_text(&soul, cx));
                    }
                    this.data
                        .update(cx, |d, _| d.dialog = Some(Dialog::BotProfile(bid.clone())));
                    cx.notify();
                }
            }))
    }

    fn model_menu(&self, bot: &Bot, cx: &mut Context<Self>) -> impl IntoElement {
        let catalog = self
            .data
            .read(cx)
            .engines
            .iter()
            .find(|e| e.kind == bot.engine)
            .map(|e| e.models.clone())
            .unwrap_or_default();
        let mut menu = div().flex().flex_col().py_1();
        for m in catalog {
            let mid = m.id.clone();
            let selected =
                bot.model.as_deref() == Some(&m.id) || (m.default && bot.model.is_none());
            menu = menu.child(
                div()
                    .id(SharedString::from(format!("model-{}", m.id)))
                    .cursor_pointer()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .when(selected, |d| d.text_color(rgb(t::CYAN)))
                    .when(!selected, |d| d.text_color(rgb(t::TEXT)))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.set_model(Some(mid.clone()), cx);
                        }),
                    )
                    .child(format!(
                        "{}{}",
                        m.label,
                        if m.default { " (default)" } else { "" }
                    )),
            );
        }
        div()
            .absolute()
            .top(px(56.))
            .right(px(120.))
            .bg(rgb(t::BG_RAISED))
            .border_1()
            .border_color(rgb(t::BORDER))
            .rounded_md()
            .shadow_lg()
            .w(px(220.))
            .child(menu)
    }

    fn messages(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let data = self.data.read(cx);
        let Some(thread_id) = data.active_thread() else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size_16()
                                .rounded_lg()
                                .bg(rgb(t::ACCENT_DIM))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_3xl()
                                .text_color(rgb(t::ACCENT))
                                .child("F"),
                        )
                        .child(
                            div()
                                .text_lg()
                                .text_color(rgb(t::TEXT_DIM))
                                .child("Pick a bot or create one"),
                        )
                        .child(div().text_sm().text_color(rgb(t::TEXT_FAINT)).child(
                            "Every contact is a real agent — its own model, persona and computer",
                        )),
                )
                .into_any_element();
        };
        let msgs = data.messages.get(&thread_id).cloned().unwrap_or_default();
        let mut list = div().flex().flex_col().gap_3().px_6().py_4();
        if msgs.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child("Say hi — your bot is listening."),
            );
        }
        for m in &msgs {
            list = list.child(self.message_row(m, cx));
        }

        div()
            .id("messages-scroll")
            .flex_1()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn message_row(&self, m: &Message, cx: &mut Context<Self>) -> impl IntoElement {
        let is_user = m.role == Role::User;
        let mut bubble = div().flex().flex_col().gap_1().max_w(px(620.));

        let segs: &[Segment] = if m.segments.is_empty() {
            if m.text.is_empty() {
                &[]
            } else {
                // synthesized text segment on the fly via fallback below
                &[]
            }
        } else {
            &m.segments
        };

        let mut content = div().flex().flex_col().gap_2();
        if segs.is_empty() {
            if m.pending {
                content = content.child(
                    div().flex().gap_1p5().items_center().child(
                        div()
                            .text_sm()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("working…"),
                    ),
                );
            } else if !m.text.is_empty() {
                content = content.child(self.render_text(&m.text, is_user));
            }
        } else {
            for (i, seg) in segs.iter().enumerate() {
                match seg {
                    Segment::Text { text } => {
                        content = content.child(self.render_text(text, is_user));
                    }
                    Segment::Thinking { text } => {
                        let key = format!("{}:{i}", m.id);
                        let open = self.expanded_thinking.contains(&key);
                        let key2 = key.clone();
                        let preview: String = text.chars().take(60).collect();
                        content =
                            content.child(
                                div()
                                    .id(SharedString::from(format!("think-{key}")))
                                    .cursor_pointer()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(t::BORDER))
                                    .bg(rgb(t::CODE_BG))
                                    .px_3()
                                    .py_2()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if !this.expanded_thinking.insert(key2.clone()) {
                                                this.expanded_thinking.remove(&key2);
                                            }
                                            cx.notify();
                                        }),
                                    )
                                    .child(div().flex().gap_2().items_center().child(
                                        div().text_xs().text_color(rgb(t::TEXT_FAINT)).child(
                                            format!(
                                                "💭 thinking — {} {}",
                                                if open { "▾" } else { "▸" },
                                                preview
                                            ),
                                        ),
                                    ))
                                    .when(open, |d| {
                                        d.child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(t::TEXT_DIM))
                                                .font_family("monospace")
                                                .child(text.clone()),
                                        )
                                    }),
                            );
                    }
                    Segment::ToolCall {
                        name,
                        input,
                        output,
                        status,
                    } => {
                        let (icon, color) = match status {
                            ToolStatus::Running => ("▶", t::AMBER),
                            ToolStatus::Done => ("✓", t::GREEN),
                            ToolStatus::Failed => ("✗", t::RED),
                        };
                        content = content.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .rounded_md()
                                .border_1()
                                .border_color(rgb(t::BORDER))
                                .bg(rgb(t::CODE_BG))
                                .px_3()
                                .py_2()
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .items_center()
                                        .child(div().text_xs().text_color(rgb(color)).child(icon))
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(rgb(t::CYAN))
                                                .child(name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_family("monospace")
                                                .text_color(rgb(t::TEXT_FAINT))
                                                .overflow_hidden()
                                                .whitespace_nowrap()
                                                .child(input.chars().take(120).collect::<String>()),
                                        ),
                                )
                                .when_some(output.clone(), |d, out| {
                                    d.child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(rgb(t::TEXT_DIM))
                                            .child(out.chars().take(500).collect::<String>()),
                                    )
                                }),
                        );
                    }
                    Segment::Approval { approval } => {
                        content = content.child(self.approval_card(approval, cx));
                    }
                    Segment::Error { text } => {
                        content = content.child(
                            div()
                                .text_sm()
                                .text_color(rgb(t::RED))
                                .child(format!("error: {text}")),
                        );
                    }
                }
            }
        }
        if let Some(err) = &m.error {
            content = content.child(
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t::RED))
                    .bg(rgba(0xf8717114))
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(t::RED))
                    .child(format!("{err}")),
            );
        }

        bubble = bubble.child(
            div()
                .rounded_lg()
                .when(is_user, |d| d.bg(rgb(t::USER_BUBBLE)))
                .when(!is_user, |d| {
                    d.bg(rgb(t::BOT_BUBBLE))
                        .border_1()
                        .border_color(rgb(t::BORDER))
                })
                .px_4()
                .py_3()
                .child(content),
        );

        div()
            .flex()
            .w_full()
            .when(is_user, |d| d.justify_end())
            .when(!is_user, |d| d.justify_start())
            .child(bubble)
    }

    fn render_text(&self, text: &str, is_user: bool) -> impl IntoElement {
        // Markdown-lite: fenced code blocks get a mono panel; rest is plain text.
        let mut out = div().flex().flex_col().gap_2();
        let mut in_code = false;
        let mut buf = String::new();
        let mut blocks: Vec<(bool, String)> = Vec::new();
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                blocks.push((in_code, std::mem::take(&mut buf)));
                in_code = !in_code;
            } else {
                buf.push_str(line);
                buf.push('\n');
            }
        }
        blocks.push((in_code, buf));
        for (code, block) in blocks {
            if block.trim().is_empty() {
                continue;
            }
            if code {
                out = out.child(
                    div()
                        .bg(rgb(t::CODE_BG))
                        .border_1()
                        .border_color(rgb(t::BORDER))
                        .rounded_md()
                        .px_3()
                        .py_2()
                        .text_sm()
                        .font_family("monospace")
                        .text_color(rgb(t::GREEN))
                        .child(block.trim_end().to_string()),
                );
            } else {
                out = out.child(
                    div()
                        .text_sm()
                        .text_color(rgb(if is_user { 0xffffff } else { t::TEXT }))
                        .child(block.trim_end().to_string()),
                );
            }
        }
        out
    }

    fn approval_card(&self, a: &ApprovalRequest, cx: &mut Context<Self>) -> impl IntoElement {
        let aid = a.id.clone();
        let aid2 = a.id.clone();
        let pending = a.status == ApprovalStatus::Pending;
        let status_label = match a.status {
            ApprovalStatus::Pending => "needs approval",
            ApprovalStatus::Allowed => "allowed",
            ApprovalStatus::Denied => "denied",
            ApprovalStatus::Answered => "answered",
        };
        let status_color = match a.status {
            ApprovalStatus::Pending => t::AMBER,
            ApprovalStatus::Allowed => t::GREEN,
            ApprovalStatus::Denied => t::RED,
            ApprovalStatus::Answered => t::CYAN,
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(t::AMBER))
            .bg(rgba(0xfbbf2412))
            .px_4()
            .py_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(status_color))
                            .child(format!("⚠ {status_label}")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(t::TEXT))
                            .child(a.title.clone()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .font_family("monospace")
                    .text_color(rgb(t::TEXT_DIM))
                    .bg(rgb(t::CODE_BG))
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .child(a.detail.chars().take(400).collect::<String>()),
            )
            .when(pending, |d| {
                d.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id(SharedString::from(format!("appr-allow-{aid}")))
                                .cursor_pointer()
                                .px_4()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(t::GREEN))
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(gpui::black())
                                .hover(|d| d.opacity(0.85))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.resolve_approval(aid.clone(), "allow", cx);
                                    }),
                                )
                                .child("Allow"),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("appr-deny-{aid2}")))
                                .cursor_pointer()
                                .px_4()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(t::RED))
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(gpui::black())
                                .hover(|d| d.opacity(0.85))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.resolve_approval(aid2.clone(), "deny", cx);
                                    }),
                                )
                                .child("Deny"),
                        ),
                )
            })
            .when(!pending, |d| {
                d.when_some(a.response.clone(), |d, r| {
                    d.child(div().text_xs().text_color(rgb(t::TEXT_FAINT)).child(r))
                })
            })
    }

    fn composer(&self, _bot: Option<&Bot>, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgb(t::BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .bg(rgb(t::BG_RAISED))
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .rounded_lg()
                    .px_4()
                    .py_2p5()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(rgb(t::TEXT))
                            .child(self.composer.clone()),
                    )
                    .child(
                        div()
                            .id("send-btn")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(t::ACCENT))
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(gpui::white())
                            .hover(|d| d.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    let text = this.composer.read(cx).content.to_string();
                                    this.composer.update(cx, |i, _| i.reset());
                                    this.send_message(text, cx);
                                }),
                            )
                            .child("Send"),
                    )
                    .child(div().text_xs().text_color(rgb(t::TEXT_FAINT)).child("⏎")),
            )
    }

    // ---- dialogs -------------------------------------------------------------

    fn dialog_overlay(&self, inner: impl IntoElement, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.data.update(cx, |d, _| d.dialog = None);
                    cx.notify();
                }),
            )
            .child(inner)
    }

    fn dialog_card(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .w(px(520.))
            .bg(rgb(t::BG_PANEL))
            .border_1()
            .border_color(rgb(t::BORDER))
            .rounded_lg()
            .shadow_xl()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
    }

    fn field(&self, label: &str, input: &Entity<TextInput>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .bg(rgb(t::CODE_BG))
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(rgb(t::TEXT))
                    .child(input.clone()),
            )
    }

    fn new_bot_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut engines = div().flex().gap_2();
        for k in EngineKind::ALL {
            let selected = self.dlg_engine == k;
            engines = engines.child(
                div()
                    .id(SharedString::from(format!("eng-{k}")))
                    .cursor_pointer()
                    .px_3()
                    .py_1p5()
                    .rounded_md()
                    .text_sm()
                    .border_1()
                    .when(selected, |d| {
                        d.bg(rgb(t::ACCENT_DIM))
                            .border_color(rgb(t::ACCENT))
                            .text_color(rgb(t::TEXT))
                    })
                    .when(!selected, |d| {
                        d.bg(rgb(t::BG_RAISED))
                            .border_color(rgb(t::BORDER))
                            .text_color(rgb(t::TEXT_DIM))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.dlg_engine = k;
                            cx.notify();
                        }),
                    )
                    .child(k.label()),
            );
        }
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("New bot"),
            )
            .child(self.field("Name", &self.dlg_name))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("Engine"),
                    )
                    .child(engines),
            )
            .child(self.field("Model (blank = engine default)", &self.dlg_model))
            .child(self.field("Persona — SOUL.md instructions", &self.dlg_soul))
            .child(self.field("Working directory", &self.dlg_cwd))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        div()
                            .id("nb-cancel")
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(t::TEXT_DIM))
                            .hover(|d| d.bg(rgb(t::BG_HOVER)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.data.update(cx, |d, _| d.dialog = None);
                                    cx.notify();
                                }),
                            )
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("nb-create")
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t::ACCENT))
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(gpui::white())
                            .hover(|d| d.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.create_bot(window, cx);
                                }),
                            )
                            .child("Create bot"),
                    ),
            )
    }

    fn settings_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let engines = self.data.read(cx).engines.clone();
        let mut card = self
            .dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Settings"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child("Engines — point a bot's brain at a custom CLI binary or endpoint."),
            );
        for (kind, input) in self.dlg_engine_inputs.clone() {
            let status = engines.iter().find(|e| e.kind == kind);
            let (badge, color) = match status {
                Some(s) if s.available => ("available", t::GREEN),
                Some(_) => ("not found", t::RED),
                None => ("…", t::TEXT_FAINT),
            };
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_sm().text_color(rgb(t::TEXT)).child(kind.label()))
                            .child(
                                div().text_xs().text_color(rgb(color)).child(
                                    status
                                        .and_then(|s| s.version.clone())
                                        .map(|v| format!("{badge} · {v}"))
                                        .unwrap_or_else(|| badge.to_string()),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .bg(rgb(t::CODE_BG))
                                    .border_1()
                                    .border_color(rgb(t::BORDER))
                                    .rounded_md()
                                    .px_3()
                                    .py_2()
                                    .text_sm()
                                    .text_color(rgb(t::TEXT))
                                    .child(input),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("eng-save-{kind}")))
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1p5()
                                    .rounded_md()
                                    .bg(rgb(t::BG_HOVER))
                                    .text_xs()
                                    .text_color(rgb(t::TEXT))
                                    .hover(|d| d.opacity(0.85))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.save_engine(kind, cx);
                                        }),
                                    )
                                    .child("Save"),
                            ),
                    ),
            );
        }
        card.child(
            div().flex().justify_end().child(
                div()
                    .id("set-close")
                    .cursor_pointer()
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .text_sm()
                    .text_color(rgb(t::TEXT_DIM))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.data.update(cx, |d, _| d.dialog = None);
                            cx.notify();
                        }),
                    )
                    .child("Close"),
            ),
        )
    }

    fn profile_dialog(&mut self, bot_id: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let bot = self.data.read(cx).bot(bot_id).cloned();
        let Some(bot) = bot else {
            return self.dialog_card(cx).child("bot not found");
        };
        // Populate soul/model fields on open — callers set them before opening.
        let bid = bot.id.clone();
        let auto = bot.auto_approve;
        self.dialog_card(cx)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.avatar(&bot))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(t::TEXT))
                            .child(bot.name.clone()),
                    ),
            )
            .child(self.field("Persona — SOUL.md instructions", &self.dlg_soul))
            .child(
                div()
                    .id("auto-toggle")
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_2()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            let harness = this.data.read(cx).harness.clone();
                            if let Ok(Some(mut b)) = harness.store.get_bot(&bid) {
                                b.auto_approve = !b.auto_approve;
                                b.updated_at = now_ms();
                                let _ = harness.store.upsert_bot(&b);
                                let _ = harness.bus.send(ServerEvent::BotUpsert { bot: b });
                            }
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .size_4()
                            .rounded_sm()
                            .border_1()
                            .border_color(rgb(t::BORDER))
                            .when(auto, |d| d.bg(rgb(t::ACCENT)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(gpui::white())
                            .when(auto, |d| d.child("✓")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(t::TEXT))
                            .child("Auto-approve actions (skip permission prompts)"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        div()
                            .id("del-bot")
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(t::RED))
                            .hover(|d| d.bg(rgb(t::BG_HOVER)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener({
                                    let bid = bot.id.clone();
                                    move |this, _, _, cx| {
                                        let harness = this.data.read(cx).harness.clone();
                                        let rt = this.data.read(cx).tokio.clone();
                                        let bid2 = bid.clone();
                                        rt.spawn(async move {
                                            if let Ok(t) = harness.store.direct_thread(&bid2) {
                                                fathom_harness::sessions::abort_thread(
                                                    &harness, &t.id,
                                                )
                                                .await;
                                            }
                                            let _ = harness.store.delete_bot(&bid2);
                                        });
                                        this.data.update(cx, |d, _| {
                                            d.dialog = None;
                                            d.active_bot = None;
                                            d.reload();
                                        });
                                        cx.notify();
                                    }
                                }),
                            )
                            .child("Delete bot"),
                    )
                    .child(
                        div()
                            .id("profile-save")
                            .cursor_pointer()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t::ACCENT))
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(gpui::white())
                            .hover(|d| d.opacity(0.85))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener({
                                    let bid = bot.id.clone();
                                    move |this, _, _, cx| {
                                        let soul = this.dlg_soul.read(cx).content.to_string();
                                        let harness = this.data.read(cx).harness.clone();
                                        if let Ok(Some(mut b)) = harness.store.get_bot(&bid) {
                                            b.soul = soul.clone();
                                            b.updated_at = now_ms();
                                            let _ = harness.store.upsert_bot(&b);
                                            let _ = harness.store.write_soul(&bid, &soul);
                                            let _ =
                                                harness.bus.send(ServerEvent::BotUpsert { bot: b });
                                        }
                                        this.data.update(cx, |d, _| d.dialog = None);
                                        cx.notify();
                                    }
                                }),
                            )
                            .child("Save"),
                    ),
            )
    }

    fn computer_dialog(&self, bot: &Bot, cx: &mut Context<Self>) -> impl IntoElement {
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child(format!("{} — computer", bot.name)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .py_6()
                    .child(div().text_3xl().child("🖥"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(t::TEXT_DIM))
                            .child("No computer attached yet"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .justify_center()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t::BG_RAISED))
                            .border_1()
                            .border_color(rgb(t::BORDER))
                            .text_sm()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("Local VM — soon"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t::BG_RAISED))
                            .border_1()
                            .border_color(rgb(t::BORDER))
                            .text_sm()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("This computer — soon"),
                    )
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_md()
                            .bg(rgb(t::BG_RAISED))
                            .border_1()
                            .border_color(rgb(t::BORDER))
                            .text_sm()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("Cloud desktop — soon"),
                    ),
            )
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("comp-close")
                        .cursor_pointer()
                        .px_4()
                        .py_2()
                        .rounded_md()
                        .text_sm()
                        .text_color(rgb(t::TEXT_DIM))
                        .hover(|d| d.bg(rgb(t::BG_HOVER)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.data.update(cx, |d, _| d.dialog = None);
                                cx.notify();
                            }),
                        )
                        .child("Close"),
                ),
            )
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx);
        let dialog = data.dialog.clone();
        let active_bot = data
            .active_bot
            .as_ref()
            .and_then(|id| data.bot(id).cloned());
        let engines_ready = !data.engines.is_empty();
        let _ = (window, engines_ready);

        let mut root = div()
            .flex()
            .size_full()
            .bg(rgb(t::BG))
            .text_color(rgb(t::TEXT))
            .track_focus(&self.focus)
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .when_some(active_bot.clone(), |d, bot| {
                        d.child(self.chat_header(&bot, cx))
                            .child(self.messages(cx))
                            .child(self.composer(Some(&bot), cx))
                    })
                    .when(active_bot.is_none(), |d| d.child(self.messages(cx))),
            );

        if let Some(bot) = &active_bot {
            if self.model_menu {
                root = root.child(self.model_menu(bot, cx));
            }
        }

        match dialog {
            Some(Dialog::NewBot) => {
                let card = self.new_bot_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::Settings) => {
                let card = self.settings_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::BotProfile(bid)) => {
                let dialog = self.profile_dialog(&bid, cx);
                root = root.child(self.dialog_overlay(dialog, cx));
            }
            Some(Dialog::Computer(bid)) => {
                if let Some(b) = self.data.read(cx).bot(&bid).cloned() {
                    root = root.child(self.dialog_overlay(self.computer_dialog(&b, cx), cx));
                }
            }
            None => {}
        }
        root
    }
}
