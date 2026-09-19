//! Root view: sidebar roster (bots + rooms + archive) + chat pane + dialogs,
//! all in GPUI.

use crate::input::{Submitted, TextInput};
use crate::state::{AppData, ChatTarget, Dialog};
use crate::theme as t;
use fathom_core::types::*;
use gpui::{
    div, img, prelude::*, px, rgb, rgba, App, Context, Entity, FocusHandle, Focusable, MouseButton,
    ScrollHandle, SharedString, Window,
};
use std::collections::HashSet;
use std::path::PathBuf;

const EMOJI_SET: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🔥"];

/// A hit in the global search dialog.
enum SearchHit {
    Bot {
        id: String,
        label: String,
    },
    Room {
        id: String,
        label: String,
    },
    Message {
        thread_id: String,
        bot_id: Option<String>,
        label: String,
    },
}

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
    /// Tasks (threads) dropdown open?
    tasks_menu: bool,
    /// Message id whose emoji picker is open.
    react_for: Option<String>,
    /// Composer state: flat reply target + staged attachments.
    reply_to: Option<Message>,
    staged: Vec<Attachment>,
    /// One-line toast shown above the composer.
    notice: Option<String>,
    /// Global search dialog results.
    search_results: Vec<SearchHit>,
    /// Dialog field entities — created per dialog open.
    dlg_name: Entity<TextInput>,
    dlg_model: Entity<TextInput>,
    dlg_soul: Entity<TextInput>,
    dlg_cwd: Entity<TextInput>,
    dlg_title: Entity<TextInput>,
    dlg_desc: Entity<TextInput>,
    dlg_attach: Entity<TextInput>,
    dlg_search: Entity<TextInput>,
    dlg_rename: Entity<TextInput>,
    dlg_profile_name: Entity<TextInput>,
    dlg_engine: EngineKind,
    /// Settings dialog: engine path inputs.
    dlg_engine_inputs: Vec<(EngineKind, Entity<TextInput>)>,
    /// New-room dialog: selected member bot ids + responder mode.
    room_members: HashSet<String>,
    room_responder_everyone: bool,
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
        let dlg_title = mk(cx, "Role / title (optional)", false);
        let dlg_desc = mk(cx, "Description (optional)", false);
        let dlg_attach = mk(cx, "/absolute/path/to/file", true);
        let dlg_search = mk(cx, "Search bots, rooms, messages…", true);
        let dlg_rename = mk(cx, "Task title", true);
        let dlg_profile_name = mk(cx, "Your display name", false);

        // Search submit → run the query.
        cx.subscribe(&dlg_search, |this: &mut Self, _i, _ev: &Submitted, cx| {
            this.run_search(cx);
        })
        .detach();
        cx.subscribe(&dlg_rename, |this: &mut Self, _i, _ev: &Submitted, cx| {
            this.commit_rename(cx);
        })
        .detach();
        cx.subscribe(&dlg_attach, |this: &mut Self, _i, _ev: &Submitted, cx| {
            this.commit_attach(cx);
        })
        .detach();

        let mut view = Self {
            data,
            composer,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            expanded_thinking: HashSet::new(),
            model_menu: false,
            tasks_menu: false,
            react_for: None,
            reply_to: None,
            staged: vec![],
            notice: None,
            search_results: vec![],
            dlg_name,
            dlg_model,
            dlg_soul,
            dlg_cwd,
            dlg_title,
            dlg_desc,
            dlg_attach,
            dlg_search,
            dlg_rename,
            dlg_profile_name,
            dlg_engine: EngineKind::Claude,
            dlg_engine_inputs: vec![],
            room_members: HashSet::new(),
            room_responder_everyone: false,
        };
        // Open the most recent conversation on launch.
        if let Some(id) = view.data.read(cx).bots.first().map(|b| b.id.clone()) {
            view.select_bot(id, cx);
        }
        view
    }

    // ---- actions -------------------------------------------------------------

    fn notice(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.notice = Some(msg.into());
        cx.notify();
    }

    fn save_draft(&mut self, cx: &mut Context<Self>) {
        // Persist the composer text under the current thread.
        let d = self.data.read(cx);
        let Some(tid) = d.active_thread() else { return };
        let text = self.composer.read(cx).content.to_string();
        let _ = d.harness.store.set_draft(&tid, &text);
    }

    fn send_message(&mut self, text: String, cx: &mut Context<Self>) {
        if text.trim().is_empty() && self.staged.is_empty() {
            return;
        }
        let active = self.data.read(cx).active.clone();
        let thread_id = self.data.read(cx).active_thread();
        let (Some(target), Some(thread_id)) = (active, thread_id) else {
            return;
        };
        let harness = self.data.read(cx).harness.clone();
        let rt = self.data.read(cx).tokio.clone();
        // Sent — clear the draft for this thread before spawning.
        let _ = harness.store.set_draft(&thread_id, "");
        let opts = fathom_harness::sessions::SendOpts {
            text,
            model: None,
            reply_to: self.reply_to.take().map(|m| m.id),
            attachments: self.staged.drain(..).map(|a| a.id).collect(),
            sender: None,
            via_api: false,
        };
        rt.spawn(async move {
            match target {
                ChatTarget::Bot(bot_id) => {
                    let _ =
                        fathom_harness::sessions::start_turn(&harness, &bot_id, &thread_id, opts)
                            .await;
                }
                ChatTarget::Room(room_id) => {
                    let _ =
                        fathom_harness::sessions::start_room_turn(&harness, &room_id, opts).await;
                }
            }
        });
        cx.notify();
    }

    fn select_bot(&mut self, bot_id: String, cx: &mut Context<Self>) {
        self.save_draft(cx);
        {
            let harness = self.data.read(cx).harness.clone();
            self.data.update(cx, |d, _| {
                d.active = Some(ChatTarget::Bot(bot_id.clone()));
                if !d.threads.contains_key(&bot_id) {
                    if let Ok(t) = harness.store.direct_thread(&bot_id) {
                        d.thread_meta.insert(t.id.clone(), t.clone());
                        d.threads.insert(bot_id.clone(), t.id);
                    }
                }
                d.reload_threads(&bot_id);
                if let Some(tid) = d.threads.get(&bot_id).cloned() {
                    d.load_thread(&tid);
                }
            });
        }
        self.restore_draft(cx);
        let rt = self.data.read(cx).tokio.clone();
        let harness = self.data.read(cx).harness.clone();
        let bid = bot_id.clone();
        rt.spawn(async move {
            let _ = harness.store.mark_read(&bid);
            if let Ok(Some(b)) = harness.store.get_bot(&bid) {
                let _ = harness.bus.send(ServerEvent::BotUpsert { bot: b });
            }
        });
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn select_thread(&mut self, bot_id: String, thread_id: String, cx: &mut Context<Self>) {
        self.save_draft(cx);
        self.data.update(cx, |d, _| {
            d.threads.insert(bot_id.clone(), thread_id.clone());
            d.load_thread(&thread_id);
        });
        self.restore_draft(cx);
        self.tasks_menu = false;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn select_room(&mut self, room_id: String, cx: &mut Context<Self>) {
        self.save_draft(cx);
        let harness = self.data.read(cx).harness.clone();
        self.data.update(cx, |d, _| {
            d.active = Some(ChatTarget::Room(room_id.clone()));
            let tid = d.room(&room_id).map(|r| r.thread_id.clone());
            if let Some(tid) = tid {
                d.load_thread(&tid);
            }
        });
        self.restore_draft(cx);
        let rt = self.data.read(cx).tokio.clone();
        rt.spawn(async move {
            let _ = harness.store.mark_room_read(&room_id);
            if let Ok(Some(r)) = harness.store.get_room(&room_id) {
                let _ = harness.bus.send(ServerEvent::RoomUpsert { room: r });
            }
        });
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn restore_draft(&mut self, cx: &mut Context<Self>) {
        let d = self.data.read(cx);
        let Some(tid) = d.active_thread() else { return };
        let draft = d
            .harness
            .store
            .get_draft(&tid)
            .ok()
            .flatten()
            .unwrap_or_default();
        self.composer.update(cx, |i, cx| i.set_text(&draft, cx));
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

    fn patch_bot(&mut self, bot_id: &str, f: impl FnOnce(&mut Bot), cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(mut b)) = harness.store.get_bot(bot_id) {
            f(&mut b);
            b.updated_at = now_ms();
            let _ = harness.store.upsert_bot(&b);
            let _ = harness.bus.send(ServerEvent::BotUpsert { bot: b });
        }
        cx.notify();
    }

    fn patch_room(&mut self, room_id: &str, f: impl FnOnce(&mut Room), cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(mut r)) = harness.store.get_room(room_id) {
            f(&mut r);
            r.updated_at = now_ms();
            let _ = harness.store.upsert_room(&r);
            let _ = harness.bus.send(ServerEvent::RoomUpsert { room: r });
        }
        cx.notify();
    }

    fn new_task(&mut self, cx: &mut Context<Self>) {
        let Some(ChatTarget::Bot(bot_id)) = self.data.read(cx).active.clone() else {
            return;
        };
        let harness = self.data.read(cx).harness.clone();
        if let Ok(t) = harness.store.new_task_thread(&bot_id, None) {
            let _ = harness
                .bus
                .send(ServerEvent::ThreadUpsert { thread: t.clone() });
            self.select_thread(bot_id, t.id, cx);
        }
    }

    fn react(&mut self, msg_id: String, emoji: &'static str, cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(m)) = harness.store.toggle_reaction(&msg_id, emoji, "user") {
            let _ = harness.bus.send(ServerEvent::MessageUpsert { message: m });
        }
        self.react_for = None;
        cx.notify();
    }

    fn toggle_pin(&mut self, msg: &Message, cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(mut t)) = harness.store.get_thread(&msg.thread_id) {
            t.pinned_message_id = if t.pinned_message_id.as_deref() == Some(&msg.id) {
                None
            } else {
                Some(msg.id.clone())
            };
            t.updated_at = now_ms();
            let _ = harness.store.upsert_thread(&t);
            let _ = harness.bus.send(ServerEvent::ThreadUpsert { thread: t });
        }
        cx.notify();
    }

    fn delete_message(&mut self, msg: &Message, cx: &mut Context<Self>) {
        let harness = self.data.read(cx).harness.clone();
        let _ = harness.store.delete_message(&msg.id);
        let _ = harness.bus.send(ServerEvent::MessageDeleted {
            id: msg.id.clone(),
            thread_id: msg.thread_id.clone(),
        });
        cx.notify();
    }

    fn export_thread(&mut self, cx: &mut Context<Self>) {
        let d = self.data.read(cx);
        let Some(tid) = d.active_thread() else { return };
        let harness = d.harness.clone();
        let msgs = harness
            .store
            .list_messages(&tid, 10000, None)
            .unwrap_or_default();
        let dir = harness.store.data_dir.join("exports");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(format!("{tid}.md"));
        let mut md = String::from("# transcript\n\n");
        for m in &msgs {
            let who = match m.role {
                Role::User => m.sender.clone().unwrap_or_else(|| "You".into()),
                Role::Bot => m
                    .from_bot
                    .as_ref()
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "Bot".into()),
                _ => "system".into(),
            };
            if !m.text.trim().is_empty() {
                md.push_str(&format!("**{who}:** {}\n\n", m.text));
            }
        }
        match std::fs::write(&path, md) {
            Ok(()) => self.notice(format!("exported → {}", path.display()), cx),
            Err(e) => self.notice(format!("export failed: {e}"), cx),
        }
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let q = self.dlg_search.read(cx).content.to_string();
        let needle = q.trim().to_lowercase();
        self.search_results.clear();
        if needle.is_empty() {
            cx.notify();
            return;
        }
        let d = self.data.read(cx);
        for b in &d.bots {
            if b.name.to_lowercase().contains(&needle) || b.title.to_lowercase().contains(&needle) {
                self.search_results.push(SearchHit::Bot {
                    id: b.id.clone(),
                    label: format!("bot · {}", b.name),
                });
            }
        }
        for r in &d.rooms {
            if r.name.to_lowercase().contains(&needle) {
                self.search_results.push(SearchHit::Room {
                    id: r.id.clone(),
                    label: format!("room · {}", r.name),
                });
            }
        }
        for m in d
            .harness
            .store
            .search_messages(&needle, 50)
            .unwrap_or_default()
        {
            let thread = d.harness.store.get_thread(&m.thread_id).ok().flatten();
            self.search_results.push(SearchHit::Message {
                thread_id: m.thread_id.clone(),
                bot_id: thread.and_then(|t| t.bot_id),
                label: format!("…{}", m.text.chars().take(80).collect::<String>()),
            });
        }
        cx.notify();
    }

    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(Dialog::RenameThread(tid)) = self.data.read(cx).dialog.clone() else {
            return;
        };
        let title = self.dlg_rename.read(cx).content.to_string();
        let harness = self.data.read(cx).harness.clone();
        if let Ok(Some(mut t)) = harness.store.get_thread(&tid) {
            t.title = if title.trim().is_empty() {
                None
            } else {
                Some(title.trim().to_string())
            };
            t.updated_at = now_ms();
            let _ = harness.store.upsert_thread(&t);
            let _ = harness.bus.send(ServerEvent::ThreadUpsert { thread: t });
        }
        self.data.update(cx, |d, _| d.dialog = None);
        cx.notify();
    }

    fn commit_attach(&mut self, cx: &mut Context<Self>) {
        let path = self.dlg_attach.read(cx).content.to_string();
        let path = path.trim();
        if path.is_empty() {
            return;
        }
        let p = PathBuf::from(path);
        let name = p
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("file")
            .to_string();
        let mime = mime_guess(&name);
        let harness = self.data.read(cx).harness.clone();
        match std::fs::read(&p) {
            Ok(bytes) => match harness.store.save_attachment(&name, &mime, &bytes) {
                Ok(a) => {
                    self.staged.push(a);
                    self.dlg_attach.update(cx, |i, cx| i.set_text("", cx));
                    self.data.update(cx, |d, _| d.dialog = None);
                }
                Err(e) => self.notice(format!("attach failed: {e}"), cx),
            },
            Err(e) => self.notice(format!("can't read {path}: {e}"), cx),
        }
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
        let title = self.dlg_title.read(cx).content.to_string();
        let harness = self.data.read(cx).harness.clone();
        let bot = Bot {
            id: fathom_core::new_id("bot"),
            name: name.trim().to_string(),
            title: title.trim().to_string(),
            description: String::new(),
            engine: self.dlg_engine,
            model: if model.trim().is_empty() {
                None
            } else {
                Some(model.trim().to_string())
            },
            effort: None,
            soul: soul.clone(),
            avatar_seed: name.bytes().fold(0u8, |a, b| a.wrapping_add(b)),
            avatar: None,
            cwd: if cwd.trim().is_empty() {
                None
            } else {
                Some(cwd.trim().to_string())
            },
            approval_mode: ApprovalMode::Ask,
            always_allow: vec![],
            notifications: true,
            pinned: false,
            hidden: false,
            section: None,
            park_dms: true,
            archived: false,
            computer_id: None,
            created_at: now_ms(),
            updated_at: now_ms(),
            last_message: None,
            last_activity_at: Some(now_ms()),
            unread: 0,
            working: false,
            waiting_on_you: false,
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

    fn create_room(&mut self, cx: &mut Context<Self>) {
        let name = self.dlg_name.read(cx).content.to_string();
        if name.trim().is_empty() || self.room_members.is_empty() {
            return;
        }
        let harness = self.data.read(cx).harness.clone();
        let now = now_ms();
        let thread = Thread {
            id: fathom_core::new_id("th"),
            kind: "room".into(),
            bot_id: None,
            title: Some(name.trim().to_string()),
            pinned_message_id: None,
            created_at: now,
            updated_at: now,
        };
        let _ = harness.store.upsert_thread(&thread);
        let room = Room {
            id: fathom_core::new_id("room"),
            name: name.trim().to_string(),
            member_ids: self.room_members.iter().cloned().collect(),
            thread_id: thread.id.clone(),
            responder: if self.room_responder_everyone {
                Responder::Everyone
            } else {
                Responder::Mentions
            },
            working: false,
            unread: 0,
            last_message: None,
            last_activity_at: Some(now),
            created_at: now,
            updated_at: now,
        };
        let _ = harness.store.upsert_room(&room);
        let _ = harness
            .bus
            .send(ServerEvent::RoomUpsert { room: room.clone() });
        self.room_members.clear();
        self.data.update(cx, |d, _| d.dialog = None);
        self.select_room(room.id, cx);
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
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
            let name = harness
                .store
                .get_kv("profile_name")
                .ok()
                .flatten()
                .unwrap_or_default();
            self.dlg_profile_name
                .update(cx, |i, cx| i.set_text(&name, cx));
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
        let Some(ChatTarget::Bot(bot_id)) = self.data.read(cx).active.clone() else {
            self.model_menu = false;
            return;
        };
        self.patch_bot(&bot_id, |b| b.model = model, cx);
        self.model_menu = false;
    }

    fn avatar_color(seed: u8) -> u32 {
        const PALETTE: [u32; 8] = [
            0x3b82f6, 0x8b5cf6, 0x22d3ee, 0x34d399, 0xfbbf24, 0xf472b6, 0xf87171, 0xa3e635,
        ];
        PALETTE[(seed as usize) % PALETTE.len()]
    }

    // ---- rendering ----------------------------------------------------------

    fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let (bots, rooms, active, archived_n) = {
            let d = self.data.read(cx);
            (
                d.bots.clone(),
                d.rooms.clone(),
                d.active.clone(),
                d.archived.len(),
            )
        };
        let mut rows = div().flex().flex_col().gap_1();

        // Group bots by optional section label; unsectioned first.
        let mut sectioned: Vec<(String, Vec<Bot>)> = Vec::new();
        for bot in &bots {
            let key = bot.section.clone().unwrap_or_default();
            if let Some((_, v)) = sectioned.iter_mut().find(|(k, _)| *k == key) {
                v.push(bot.clone());
            } else {
                sectioned.push((key, vec![bot.clone()]));
            }
        }
        for (section, bots) in &sectioned {
            if !section.is_empty() {
                rows = rows.child(
                    div().px_4().pt_2().pb_1().child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(t::TEXT_FAINT))
                            .child(section.clone()),
                    ),
                );
            }
            for bot in bots {
                rows = rows.child(self.bot_row(bot, &active, cx));
            }
        }

        // Rooms section.
        if !rooms.is_empty() {
            rows = rows.child(
                div().px_4().pt_2().pb_1().child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(t::TEXT_FAINT))
                        .child("Rooms"),
                ),
            );
            for room in &rooms {
                rows = rows.child(self.room_row(room, &active, cx));
            }
        }

        let bots_empty = bots.is_empty();

        div()
            .flex()
            .flex_col()
            .w(px(260.))
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(self.icon_btn("search-btn", "⌕", cx, |this, cx| {
                                this.search_results.clear();
                                this.data.update(cx, |d, _| d.dialog = Some(Dialog::Search));
                                cx.notify();
                            }))
                            .child(self.icon_btn("new-room", "👥", cx, |this, cx| {
                                this.room_members.clear();
                                this.dlg_name.update(cx, |i, cx| i.set_text("", cx));
                                this.data
                                    .update(cx, |d, _| d.dialog = Some(Dialog::NewRoom));
                                cx.notify();
                            }))
                            .child(self.icon_btn("new-bot", "+", cx, |this, cx| {
                                this.data.update(cx, |d, _| d.dialog = Some(Dialog::NewBot));
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .py_2()
                    .child(rows)
                    .when(bots_empty && rooms.is_empty(), |d| {
                        d.child(
                            div().px_4().py_3().child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(t::TEXT_FAINT))
                                    .child("No bots yet — hit + to create one."),
                            ),
                        )
                    }),
            )
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
                            .id("archived-row")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .hover(|d| d.text_color(rgb(t::TEXT)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.data.update(cx, |d, _| d.reload());
                                    this.data
                                        .update(cx, |d, _| d.dialog = Some(Dialog::Archived));
                                    cx.notify();
                                }),
                            )
                            .child(format!("🗄 {archived_n} archived")),
                    )
                    .child(self.icon_btn("settings", "⚙", cx, |this, cx| this.open_settings(cx))),
            )
    }

    fn bot_row(
        &self,
        bot: &Bot,
        active: &Option<ChatTarget>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bid = bot.id.clone();
        let selected = matches!(active, Some(ChatTarget::Bot(b)) if *b == bot.id);
        let unread = bot.unread;
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
            .child(self.avatar(bot, cx))
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
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(t::TEXT))
                                    .child(format!(
                                        "{}{}",
                                        if bot.pinned { "📌 " } else { "" },
                                        bot.name
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(if bot.waiting_on_you {
                                        t::AMBER
                                    } else {
                                        t::CYAN
                                    }))
                                    .when(bot.waiting_on_you, |d| d.child("!"))
                                    .when(bot.working, |d| d.child("●")),
                            ),
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
                                    .filter(|s| !s.is_empty())
                                    .or_else(|| {
                                        if bot.title.is_empty() {
                                            Some(bot.engine.label().to_string())
                                        } else {
                                            Some(bot.title.clone())
                                        }
                                    })
                                    .unwrap_or_default(),
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
            })
    }

    fn room_row(
        &self,
        room: &Room,
        active: &Option<ChatTarget>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let rid = room.id.clone();
        let selected = matches!(active, Some(ChatTarget::Room(r)) if *r == room.id);
        div()
            .id(SharedString::from(format!("room-{}", room.id)))
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
                    this.select_room(rid.clone(), cx);
                }),
            )
            .child(
                div()
                    .size_8()
                    .rounded_md()
                    .bg(rgb(t::ACCENT_DIM))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(t::ACCENT))
                    .child("👥"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(t::TEXT))
                            .child(room.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(
                                room.last_message
                                    .clone()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| {
                                        format!("{} members", room.member_ids.len())
                                    }),
                            ),
                    ),
            )
            .when(room.unread > 0, |d| {
                d.child(
                    div()
                        .bg(rgb(t::ACCENT))
                        .rounded_full()
                        .px_2()
                        .py_0p5()
                        .text_xs()
                        .text_color(gpui::white())
                        .child(format!("{}", room.unread)),
                )
            })
    }

    fn avatar(&self, bot: &Bot, cx: &mut Context<Self>) -> gpui::AnyElement {
        // Custom avatar image wins over the colored initial.
        if let Some(att_id) = &bot.avatar {
            if let Ok(Some(a)) = self.data.read(cx).harness.store.get_attachment(att_id) {
                return img(PathBuf::from(&a.path))
                    .size_8()
                    .rounded_full()
                    .into_any_element();
            }
        }
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
            .into_any_element()
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

    fn chip_btn(
        &self,
        id: &str,
        label: &str,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
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
                cx.listener(move |this, _, _, cx| f(this, cx)),
            )
            .child(label.to_string())
    }

    fn chat_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let (active, bots, rooms, bot_threads, threads, thread_meta) = {
            let d = self.data.read(cx);
            (
                d.active.clone(),
                d.bots.clone(),
                d.rooms.clone(),
                d.bot_threads.clone(),
                d.threads.clone(),
                d.thread_meta.clone(),
            )
        };
        let find_bot = |id: &str| bots.iter().find(|b| b.id == id).cloned();
        let mut header = div()
            .flex()
            .items_center()
            .gap_3()
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(rgb(t::BORDER));

        match &active {
            Some(ChatTarget::Bot(bot_id)) => {
                let Some(bot) = find_bot(bot_id) else {
                    return header.child("bot not found");
                };
                let bid2 = bot.id.clone();
                let bid3 = bot.id.clone();
                let bid4 = bot.id.clone();
                let bid5 = bot.id.clone();
                let tasks_n = bot_threads
                    .get(&bot.id)
                    .map(|v| v.len())
                    .unwrap_or(1)
                    .max(1);
                let thread_title = threads
                    .get(&bot.id)
                    .and_then(|t| thread_meta.get(t))
                    .and_then(|t| t.title.clone());
                header = header
                    .child(self.avatar(&bot, cx))
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
                                div()
                                    .id("engine-badge")
                                    .text_xs()
                                    .text_color(rgb(t::CYAN))
                                    .child(format!(
                                        "{} · {}{}",
                                        bot.engine.label(),
                                        bot.model.clone().unwrap_or_else(|| "default".into()),
                                        thread_title.map(|t| format!(" · {t}")).unwrap_or_default()
                                    )),
                            ),
                    )
                    .child(div().flex_1())
                    .child(self.chip_btn(
                        "tasks-btn",
                        &format!("tasks ({tasks_n}) ▾"),
                        cx,
                        |this, cx| {
                            this.tasks_menu = !this.tasks_menu;
                            this.model_menu = false;
                            cx.notify();
                        },
                    ))
                    .child(self.chip_btn("model-picker", "model ▾", cx, |this, cx| {
                        this.model_menu = !this.model_menu;
                        this.tasks_menu = false;
                        cx.notify();
                    }))
                    .child(self.chip_btn(
                        "mode-chip",
                        bot.approval_mode.label(),
                        cx,
                        move |this, cx| {
                            this.patch_bot(
                                &bid2,
                                |b| {
                                    b.approval_mode = b.approval_mode.next_for(b.engine);
                                },
                                cx,
                            );
                        },
                    ))
                    .child(self.icon_btn("export-btn", "⤓", cx, |this, cx| this.export_thread(cx)))
                    .child(self.icon_btn(
                        "pin-bot",
                        if bot.pinned { "📌" } else { "📎" },
                        cx,
                        move |this, cx| {
                            this.patch_bot(&bid3, |b| b.pinned = !b.pinned, cx);
                        },
                    ))
                    .child(self.icon_btn("archive-bot", "🗄", cx, move |this, cx| {
                        this.patch_bot(&bid4, |b| b.archived = true, cx);
                        this.data.update(cx, |d, _| {
                            d.active = None;
                            d.reload();
                        });
                    }))
                    .child(self.icon_btn("computer-btn", "🖥", cx, move |this, cx| {
                        let bid = bid5.clone();
                        this.data
                            .update(cx, |d, _| d.dialog = Some(Dialog::Computer(bid)));
                        cx.notify();
                    }))
                    .when(bot.working, |d| {
                        d.child(
                            self.icon_btn("stop-btn", "■ stop", cx, |this, cx| {
                                this.stop_turn(cx)
                            }),
                        )
                    })
                    .child(self.icon_btn("profile-btn", "✎", cx, {
                        let bid = bot.id.clone();
                        move |this, cx| {
                            if let Some(b) = this.data.read(cx).bot(&bid) {
                                let soul = b.soul.clone();
                                let title = b.title.clone();
                                let desc = b.description.clone();
                                this.dlg_soul.update(cx, |i, cx| i.set_text(&soul, cx));
                                this.dlg_title.update(cx, |i, cx| i.set_text(&title, cx));
                                this.dlg_desc.update(cx, |i, cx| i.set_text(&desc, cx));
                            }
                            this.data.update(cx, |d, _| {
                                d.dialog = Some(Dialog::BotProfile(bid.clone()))
                            });
                            cx.notify();
                        }
                    }));
            }
            Some(ChatTarget::Room(room_id)) => {
                let Some(room) = rooms.iter().find(|r| &r.id == room_id).cloned() else {
                    return header.child("room not found");
                };
                let rid = room.id.clone();
                let responder_label = match &room.responder {
                    Responder::Mentions => "mentions",
                    Responder::Everyone => "everyone",
                    Responder::Member { bot_id } => {
                        match find_bot(bot_id).map(|b| b.name.clone()) {
                            Some(n) => Box::leak(n.into_boxed_str()) as &str,
                            None => "member",
                        }
                    }
                };
                header = header
                    .child(
                        div()
                            .size_8()
                            .rounded_md()
                            .bg(rgb(t::ACCENT_DIM))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .child("👥"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(t::TEXT))
                                    .child(room.name.clone()),
                            )
                            .child(div().text_xs().text_color(rgb(t::CYAN)).child(format!(
                                "{} members · answer: {}",
                                room.member_ids.len(),
                                responder_label
                            ))),
                    )
                    .child(div().flex_1())
                    .child(
                        self.chip_btn("resp-chip", "answer mode ↻", cx, move |this, cx| {
                            this.patch_room(
                                &rid,
                                |r| {
                                    r.responder = match &r.responder {
                                        Responder::Mentions => Responder::Everyone,
                                        _ => Responder::Mentions,
                                    };
                                },
                                cx,
                            );
                        }),
                    )
                    .child(self.icon_btn("export-btn", "⤓", cx, |this, cx| this.export_thread(cx)));
            }
            None => {}
        }
        header
    }

    fn tasks_menu(&self, bot_id: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let data = self.data.read(cx);
        let threads = data.bot_threads.get(bot_id).cloned().unwrap_or_default();
        let current = data.threads.get(bot_id).cloned();
        let bid = bot_id.to_string();
        let mut menu = div().flex().flex_col().py_1();
        for th in threads {
            let tid = th.id.clone();
            let tid2 = th.id.clone();
            let selected = current.as_deref() == Some(&th.id);
            let label = th
                .title
                .clone()
                .unwrap_or_else(|| format!("task {}", &th.id[th.id.len().saturating_sub(4)..]));
            menu = menu.child(
                div()
                    .id(SharedString::from(format!("task-{tid}")))
                    .cursor_pointer()
                    .px_4()
                    .py_2()
                    .text_sm()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .when(selected, |d| d.text_color(rgb(t::CYAN)))
                    .when(!selected, |d| d.text_color(rgb(t::TEXT)))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let bid = bid.clone();
                            move |this, _, _, cx| {
                                this.select_thread(bid.clone(), tid.clone(), cx);
                            }
                        }),
                    )
                    .child(label)
                    .child(
                        div()
                            .id(SharedString::from(format!("task-ren-{tid2}")))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.data.update(cx, |d, _| {
                                        d.dialog = Some(Dialog::RenameThread(tid2.clone()))
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("✎"),
                    ),
            );
        }
        menu = menu.child(
            div()
                .id("task-new")
                .cursor_pointer()
                .px_4()
                .py_2()
                .text_sm()
                .text_color(rgb(t::ACCENT))
                .hover(|d| d.bg(rgb(t::BG_HOVER)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.new_task(cx)),
                )
                .child("+ New task"),
        );
        div()
            .absolute()
            .top(px(56.))
            .right(px(200.))
            .bg(rgb(t::BG_RAISED))
            .border_1()
            .border_color(rgb(t::BORDER))
            .rounded_md()
            .shadow_lg()
            .w(px(240.))
            .child(menu)
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
        // Effort row at the bottom of the menu.
        let bid = bot.id.clone();
        menu = menu.child(
            div()
                .id("effort-row")
                .cursor_pointer()
                .px_4()
                .py_2()
                .text_xs()
                .text_color(rgb(t::TEXT_DIM))
                .border_t_1()
                .border_color(rgb(t::BORDER))
                .hover(|d| d.bg(rgb(t::BG_HOVER)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.patch_bot(
                            &bid,
                            |b| {
                                let cur = b.effort.clone().unwrap_or_else(|| "none".into());
                                let idx = EFFORT_LEVELS.iter().position(|e| *e == cur).unwrap_or(0);
                                let next = EFFORT_LEVELS[(idx + 1) % EFFORT_LEVELS.len()];
                                b.effort = (next != "none").then(|| next.to_string());
                            },
                            cx,
                        );
                    }),
                )
                .child(format!(
                    "effort: {} (click to cycle)",
                    bot.effort.clone().unwrap_or_else(|| "default".into())
                )),
        );
        div()
            .absolute()
            .top(px(56.))
            .right(px(140.))
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
        let pinned = data
            .thread_meta
            .get(&thread_id)
            .and_then(|t| t.pinned_message_id.clone())
            .and_then(|pid| msgs.iter().find(|m| m.id == pid).cloned())
            .or_else(|| {
                data.thread_meta
                    .get(&thread_id)
                    .and_then(|t| t.pinned_message_id.clone())
                    .and_then(|pid| data.harness.store.get_message(&pid).ok().flatten())
            });
        let mut list = div().flex().flex_col().gap_3().px_6().py_4();
        if let Some(p) = pinned {
            list = list.child(
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t::AMBER))
                    .bg(rgba(0xfbbf2410))
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().child("📌"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_DIM))
                            .overflow_hidden()
                            .child(p.text.chars().take(200).collect::<String>()),
                    ),
            );
        }
        if msgs.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child("Say hi — your bot is listening."),
            );
        }
        for m in &msgs {
            list = list.child(self.message_row(m, &msgs, cx));
        }

        div()
            .id("messages-scroll")
            .flex_1()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn message_row(
        &self,
        m: &Message,
        all: &[Message],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_user = m.role == Role::User;
        let mid = m.id.clone();
        let mid_react = m.id.clone();
        let m_reply = m.clone();
        let m_pin = m.clone();
        let m_del = m.clone();
        let mut bubble = div().flex().flex_col().gap_1().max_w(px(620.));

        // sender attribution (rooms / shared)
        if let Some(f) = &m.from_bot {
            bubble = bubble.child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(Self::avatar_color(f.avatar_seed)))
                    .child(f.name.clone()),
            );
        } else if let Some(s) = &m.sender {
            if is_user {
                bubble = bubble.child(
                    div()
                        .text_xs()
                        .text_color(rgb(t::TEXT_FAINT))
                        .child(s.clone()),
                );
            }
        }

        let mut content = div().flex().flex_col().gap_2();

        // flat reply quote
        if let Some(rid) = &m.reply_to {
            if let Some(orig) = all.iter().find(|o| &o.id == rid) {
                content = content.child(
                    div()
                        .border_l_2()
                        .border_color(rgb(t::ACCENT))
                        .pl_2()
                        .text_xs()
                        .text_color(rgb(t::TEXT_FAINT))
                        .child(orig.text.chars().take(160).collect::<String>()),
                );
            }
        }

        let segs = &m.segments;
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

        // attachments
        if !m.attachments.is_empty() {
            let mut files = div().flex().flex_col().gap_1();
            for a in &m.attachments {
                if a.kind == "image" {
                    files = files.child(
                        img(PathBuf::from(&a.path))
                            .max_w(px(360.))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(t::BORDER)),
                    );
                } else {
                    files = files.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(rgb(t::CYAN))
                            .child(format!("📎 {} ({} B)", a.name, a.size)),
                    );
                }
            }
            content = content.child(files);
        }

        // reactions row
        if !m.reactions.is_empty() {
            let mut row = div().flex().gap_1();
            let mut groups: Vec<(String, usize)> = vec![];
            for r in &m.reactions {
                if let Some((_, n)) = groups.iter_mut().find(|(e, _)| *e == r.emoji) {
                    *n += 1;
                } else {
                    groups.push((r.emoji.clone(), 1));
                }
            }
            for (emoji, n) in groups {
                let mid2 = m.id.clone();
                let e2 = emoji.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("react-{}-{}", m.id, emoji)))
                        .cursor_pointer()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(t::BORDER))
                        .bg(rgb(t::BG_RAISED))
                        .text_xs()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let harness = this.data.read(cx).harness.clone();
                                if let Ok(Some(mm)) =
                                    harness.store.toggle_reaction(&mid2, &e2, "user")
                                {
                                    let _ = harness
                                        .bus
                                        .send(ServerEvent::MessageUpsert { message: mm });
                                }
                                cx.notify();
                            }),
                        )
                        .child(format!("{emoji} {n}")),
                );
            }
            content = content.child(row);
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

        // meta chips + action row
        let mut meta = div().flex().gap_2().items_center();
        if m.queued {
            meta = meta.child(div().text_xs().text_color(rgb(t::AMBER)).child("queued"));
        }
        if m.steered {
            meta = meta.child(div().text_xs().text_color(rgb(t::CYAN)).child("steered"));
        }
        if m.via_api {
            meta = meta.child(div().text_xs().text_color(rgb(t::TEXT_FAINT)).child("api"));
        }
        meta = meta.child(
            div()
                .id(SharedString::from(format!("m-reply-{mid}")))
                .cursor_pointer()
                .text_xs()
                .text_color(rgb(t::TEXT_FAINT))
                .hover(|d| d.text_color(rgb(t::TEXT)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.reply_to = Some(m_reply.clone());
                        cx.notify();
                    }),
                )
                .child("↩"),
        );
        meta = meta.child(
            div()
                .id(SharedString::from(format!("m-react-{mid_react}")))
                .cursor_pointer()
                .text_xs()
                .text_color(rgb(t::TEXT_FAINT))
                .hover(|d| d.text_color(rgb(t::TEXT)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.react_for = if this.react_for.as_deref() == Some(&mid_react) {
                            None
                        } else {
                            Some(mid_react.clone())
                        };
                        cx.notify();
                    }),
                )
                .child("☺"),
        );
        meta = meta.child(
            div()
                .id(SharedString::from(format!("m-pin-{}", m.id)))
                .cursor_pointer()
                .text_xs()
                .text_color(rgb(t::TEXT_FAINT))
                .hover(|d| d.text_color(rgb(t::TEXT)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| this.toggle_pin(&m_pin, cx)),
                )
                .child("📌"),
        );
        meta = meta.child(
            div()
                .id(SharedString::from(format!("m-del-{}", m.id)))
                .cursor_pointer()
                .text_xs()
                .text_color(rgb(t::TEXT_FAINT))
                .hover(|d| d.text_color(rgb(t::RED)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| this.delete_message(&m_del, cx)),
                )
                .child("🗑"),
        );
        bubble = bubble.child(meta);

        // inline emoji picker
        if self.react_for.as_deref() == Some(&m.id) {
            let mut picker = div().flex().gap_1();
            for e in EMOJI_SET {
                let mid3 = m.id.clone();
                picker = picker.child(
                    div()
                        .id(SharedString::from(format!("pick-{}-{}", m.id, e)))
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(t::BG_RAISED))
                        .border_1()
                        .border_color(rgb(t::BORDER))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| this.react(mid3.clone(), e, cx)),
                        )
                        .child(e),
                );
            }
            bubble = bubble.child(picker);
        }

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
        let aid3 = a.id.clone();
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
        let mut card = div()
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
            .when_some(a.held.clone(), |d, held| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(rgb(t::AMBER))
                        .child(format!("held: {held}")),
                )
            })
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
            );

        if pending && !a.options.is_empty() {
            // Structured question card — one button per option.
            let mut opts = div().flex().gap_2().flex_wrap();
            for (i, opt) in a.options.iter().enumerate() {
                let aid_o = a.id.clone();
                let label = opt.clone();
                opts = opts.child(
                    div()
                        .id(SharedString::from(format!("appr-opt-{aid_o}-{i}")))
                        .cursor_pointer()
                        .px_4()
                        .py_1p5()
                        .rounded_md()
                        .bg(rgb(t::BG_HOVER))
                        .text_sm()
                        .text_color(rgb(t::TEXT))
                        .hover(|d| d.opacity(0.85))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let harness = this.data.read(cx).harness.clone();
                                let rt = this.data.read(cx).tokio.clone();
                                let id = aid_o.clone();
                                let answer = label.clone();
                                rt.spawn(async move {
                                    fathom_harness::sessions::resolve_approval(
                                        &harness, &id, &answer, None,
                                    )
                                    .await;
                                });
                                cx.notify();
                            }),
                        )
                        .child(opt.clone()),
                );
            }
            card = card.child(opts);
        } else if pending {
            card = card.child(
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
                    .when(a.tool.is_some(), |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("appr-always-{aid3}")))
                                .cursor_pointer()
                                .px_4()
                                .py_1p5()
                                .rounded_md()
                                .bg(rgb(t::CYAN))
                                .text_sm()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(gpui::black())
                                .hover(|d| d.opacity(0.85))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.resolve_approval(aid3.clone(), "always", cx);
                                    }),
                                )
                                .child("Always allow"),
                        )
                    })
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
            );
        } else {
            card = card.when_some(a.response.clone(), |d, r| {
                d.child(div().text_xs().text_color(rgb(t::TEXT_FAINT)).child(r))
            });
        }
        card
    }

    fn composer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(rgb(t::BORDER))
            .flex()
            .flex_col()
            .gap_2();

        // reply chip
        if let Some(m) = &self.reply_to {
            bar = bar.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child(format!("↩ {}", m.text.chars().take(80).collect::<String>())),
                    )
                    .child(
                        div()
                            .id("reply-clear")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(t::TEXT_DIM))
                            .hover(|d| d.text_color(rgb(t::TEXT)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.reply_to = None;
                                    cx.notify();
                                }),
                            )
                            .child("×"),
                    ),
            );
        }

        // staged attachments
        if !self.staged.is_empty() {
            let mut row = div().flex().gap_2();
            for (i, a) in self.staged.clone().iter().enumerate() {
                row = row.child(
                    div()
                        .id(SharedString::from(format!("staged-{i}")))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .bg(rgb(t::BG_RAISED))
                        .border_1()
                        .border_color(rgb(t::BORDER))
                        .text_xs()
                        .text_color(rgb(t::TEXT_DIM))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if i < this.staged.len() {
                                    this.staged.remove(i);
                                }
                                cx.notify();
                            }),
                        )
                        .child(format!("📎 {} ×", a.name)),
                );
            }
            bar = bar.child(row);
        }

        if let Some(n) = &self.notice {
            bar = bar.child(
                div()
                    .id("notice")
                    .cursor_pointer()
                    .text_xs()
                    .text_color(rgb(t::CYAN))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.notice = None;
                            cx.notify();
                        }),
                    )
                    .child(n.clone()),
            );
        }

        bar.child(
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
                        .id("attach-btn")
                        .cursor_pointer()
                        .text_sm()
                        .text_color(rgb(t::TEXT_DIM))
                        .hover(|d| d.text_color(rgb(t::TEXT)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.data.update(cx, |d, _| d.dialog = Some(Dialog::Attach));
                                cx.notify();
                            }),
                        )
                        .child("📎"),
                )
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
            .max_h(px(640.))
            .bg(rgb(t::BG_PANEL))
            .border_1()
            .border_color(rgb(t::BORDER))
            .rounded_lg()
            .shadow_xl()
            .p_5()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_hidden()
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

    fn toggle_row(
        &self,
        id: &str,
        label: &str,
        on: bool,
        cx: &mut Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(id.to_string()))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap_2()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| f(this, cx)),
            )
            .child(
                div()
                    .size_4()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .when(on, |d| d.bg(rgb(t::ACCENT)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(gpui::white())
                    .when(on, |d| d.child("✓")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(t::TEXT))
                    .child(label.to_string()),
            )
    }

    fn new_bot_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut engines = div().flex().gap_2().flex_wrap();
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
            .child(self.field("Role / title", &self.dlg_title))
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
            .child(self.dialog_buttons(
                cx,
                "nb-cancel",
                "nb-create",
                "Create bot",
                |this, _w, cx| {
                    this.create_bot(_w, cx);
                },
            ))
    }

    fn dialog_buttons(
        &self,
        cx: &mut Context<Self>,
        cancel_id: &str,
        ok_id: &str,
        ok_label: &str,
        on_ok: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                div()
                    .id(SharedString::from(cancel_id.to_string()))
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
                    .id(SharedString::from(ok_id.to_string()))
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
                        cx.listener(move |this, _, window, cx| {
                            on_ok(this, window, cx);
                        }),
                    )
                    .child(ok_label.to_string()),
            )
    }

    fn new_room_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let bots = self.data.read(cx).bots.clone();
        let mut members = div().flex().flex_col().gap_1();
        for b in &bots {
            let bid = b.id.clone();
            let on = self.room_members.contains(&b.id);
            members = members.child(self.toggle_row(
                &format!("room-mem-{}", b.id),
                &b.name,
                on,
                cx,
                move |this, cx| {
                    if !this.room_members.insert(bid.clone()) {
                        this.room_members.remove(&bid);
                    }
                    cx.notify();
                },
            ));
        }
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("New room"),
            )
            .child(self.field("Room name", &self.dlg_name))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("Members"),
                    )
                    .child(members),
            )
            .child(self.toggle_row(
                "room-everyone",
                "Everyone answers each message (default: mentions only)",
                self.room_responder_everyone,
                cx,
                |this, cx| {
                    this.room_responder_everyone = !this.room_responder_everyone;
                    cx.notify();
                },
            ))
            .child(self.dialog_buttons(
                cx,
                "nr-cancel",
                "nr-create",
                "Create room",
                |this, _w, cx| {
                    this.create_room(cx);
                },
            ))
    }

    fn settings_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let engines = self.data.read(cx).engines.clone();
        let analytics = self
            .data
            .read(cx)
            .harness
            .store
            .get_kv("analytics_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let mut card = self
            .dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Settings"),
            )
            .child(self.field("Your display name", &self.dlg_profile_name))
            .child(
                div()
                    .id("profile-name-save")
                    .cursor_pointer()
                    .text_xs()
                    .text_color(rgb(t::ACCENT))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            let name = this.dlg_profile_name.read(cx).content.to_string();
                            let harness = this.data.read(cx).harness.clone();
                            let _ = harness.store.set_kv("profile_name", name.trim());
                            this.notice("name saved", cx);
                        }),
                    )
                    .child("Save name"),
            )
            .child(self.toggle_row(
                "analytics-toggle",
                "Anonymous usage analytics (opt-in, off by default)",
                analytics,
                cx,
                |this, cx| {
                    let harness = this.data.read(cx).harness.clone();
                    let cur = harness
                        .store
                        .get_kv("analytics_enabled")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(false);
                    let _ = harness
                        .store
                        .set_kv("analytics_enabled", if cur { "false" } else { "true" });
                    cx.notify();
                },
            ))
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
        let bid_mode = bot.id.clone();
        let bid_notif = bot.id.clone();
        let bid_hide = bot.id.clone();
        let bid_park = bot.id.clone();
        let bid_arch = bot.id.clone();
        let mode_label = format!(
            "Approval mode: {} (click to cycle)",
            bot.approval_mode.label()
        );
        let mut card = self
            .dialog_card(cx)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.avatar(&bot, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(t::TEXT))
                                    .child(bot.name.clone()),
                            )
                            .child(div().text_xs().text_color(rgb(t::CYAN)).child(format!(
                                "{} · {}",
                                bot.engine.label(),
                                bot.model.clone().unwrap_or_else(|| "default".into())
                            ))),
                    ),
            )
            .child(self.field("Role / title", &self.dlg_title))
            .child(self.field("Description", &self.dlg_desc))
            .child(self.field("Persona — SOUL.md instructions", &self.dlg_soul))
            .child(
                div()
                    .id("mode-cycle")
                    .cursor_pointer()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(t::BORDER))
                    .bg(rgb(t::BG_RAISED))
                    .text_sm()
                    .text_color(rgb(t::TEXT))
                    .hover(|d| d.bg(rgb(t::BG_HOVER)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.patch_bot(
                                &bid_mode,
                                |b| {
                                    b.approval_mode = b.approval_mode.next_for(b.engine);
                                },
                                cx,
                            );
                        }),
                    )
                    .child(mode_label),
            )
            .child(self.toggle_row(
                "notif-toggle",
                "Notifications for this bot's replies",
                bot.notifications,
                cx,
                move |this, cx| {
                    this.patch_bot(&bid_notif, |b| b.notifications = !b.notifications, cx);
                },
            ))
            .child(self.toggle_row(
                "park-toggle",
                "Park messages while working (no mid-turn steer)",
                bot.park_dms,
                cx,
                move |this, cx| {
                    this.patch_bot(&bid_park, |b| b.park_dms = !b.park_dms, cx);
                },
            ))
            .child(self.toggle_row(
                "hide-toggle",
                "Hide from sidebar",
                bot.hidden,
                cx,
                move |this, cx| {
                    this.patch_bot(&bid_hide, |b| b.hidden = !b.hidden, cx);
                },
            ));

        // always-allow list
        if !bot.always_allow.is_empty() {
            let mut chips = div().flex().gap_1().flex_wrap();
            for tool in &bot.always_allow {
                let bid_t = bot.id.clone();
                let tool = tool.clone();
                let tool_lbl = tool.clone();
                chips = chips.child(
                    div()
                        .id(SharedString::from(format!("aa-{tool}")))
                        .cursor_pointer()
                        .px_2()
                        .py_0p5()
                        .rounded_full()
                        .bg(rgb(t::BG_RAISED))
                        .border_1()
                        .border_color(rgb(t::BORDER))
                        .text_xs()
                        .text_color(rgb(t::TEXT_DIM))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let tool = tool.clone();
                                this.patch_bot(
                                    &bid_t,
                                    |b| {
                                        b.always_allow.retain(|t| t != &tool);
                                    },
                                    cx,
                                );
                            }),
                        )
                        .child(format!("{tool_lbl} ×")),
                );
            }
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(t::TEXT_FAINT))
                            .child("Always-allowed tools"),
                    )
                    .child(chips),
            );
        }

        card = card.child(
            div()
                .flex()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            div()
                                .id("arch-bot")
                                .cursor_pointer()
                                .px_4()
                                .py_2()
                                .rounded_md()
                                .text_sm()
                                .text_color(rgb(t::TEXT_DIM))
                                .hover(|d| d.bg(rgb(t::BG_HOVER)))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener({
                                        let bid = bid_arch.clone();
                                        move |this, _, _, cx| {
                                            this.patch_bot(&bid, |b| b.archived = true, cx);
                                            this.data.update(cx, |d, _| {
                                                d.dialog = None;
                                                d.active = None;
                                                d.reload();
                                            });
                                        }
                                    }),
                                )
                                .child("Archive"),
                        )
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
                                                d.active = None;
                                                d.reload();
                                            });
                                            cx.notify();
                                        }
                                    }),
                                )
                                .child("Delete"),
                        ),
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
                                    let title = this.dlg_title.read(cx).content.to_string();
                                    let desc = this.dlg_desc.read(cx).content.to_string();
                                    this.patch_bot(
                                        &bid,
                                        |b| {
                                            b.soul = soul.clone();
                                            b.title = title.trim().to_string();
                                            b.description = desc.trim().to_string();
                                        },
                                        cx,
                                    );
                                    let harness = this.data.read(cx).harness.clone();
                                    let _ = harness.store.write_soul(&bid, &soul);
                                    this.data.update(cx, |d, _| d.dialog = None);
                                    cx.notify();
                                }
                            }),
                        )
                        .child("Save"),
                ),
        );
        card
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

    fn archived_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let archived = self.data.read(cx).archived.clone();
        let mut list = div().flex().flex_col().gap_2();
        if archived.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child("Nothing archived."),
            );
        }
        for b in archived {
            let bid_r = b.id.clone();
            let bid_d = b.id.clone();
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.avatar(&b, cx))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(t::TEXT))
                                    .child(b.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id(SharedString::from(format!("restore-{}", b.id)))
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(t::BG_HOVER))
                                    .text_xs()
                                    .text_color(rgb(t::TEXT))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.patch_bot(&bid_r, |b| b.archived = false, cx);
                                            this.data.update(cx, |d, _| d.reload());
                                        }),
                                    )
                                    .child("Restore"),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!("purge-{}", b.id)))
                                    .cursor_pointer()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .text_xs()
                                    .text_color(rgb(t::RED))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            let harness = this.data.read(cx).harness.clone();
                                            let _ = harness.store.delete_bot(&bid_d);
                                            this.data.update(cx, |d, _| d.reload());
                                            cx.notify();
                                        }),
                                    )
                                    .child("Delete"),
                            ),
                    ),
            );
        }
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Archived bots"),
            )
            .child(list)
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("arch-close")
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

    fn search_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut results = div().flex().flex_col().gap_1();
        for (i, hit) in self.search_results.iter().enumerate() {
            match hit {
                SearchHit::Bot { id, label } => {
                    let id = id.clone();
                    results = results.child(
                        div()
                            .id(SharedString::from(format!("sr-b-{i}")))
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(t::TEXT))
                            .hover(|d| d.bg(rgb(t::BG_HOVER)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.data.update(cx, |d, _| d.dialog = None);
                                    this.select_bot(id.clone(), cx);
                                }),
                            )
                            .child(label.clone()),
                    );
                }
                SearchHit::Room { id, label } => {
                    let id = id.clone();
                    results = results.child(
                        div()
                            .id(SharedString::from(format!("sr-r-{i}")))
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(t::TEXT))
                            .hover(|d| d.bg(rgb(t::BG_HOVER)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.data.update(cx, |d, _| d.dialog = None);
                                    this.select_room(id.clone(), cx);
                                }),
                            )
                            .child(label.clone()),
                    );
                }
                SearchHit::Message {
                    thread_id,
                    bot_id,
                    label,
                } => {
                    let tid = thread_id.clone();
                    let bid = bot_id.clone();
                    results = results.child(
                        div()
                            .id(SharedString::from(format!("sr-m-{i}")))
                            .cursor_pointer()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_sm()
                            .text_color(rgb(t::TEXT_DIM))
                            .hover(|d| d.bg(rgb(t::BG_HOVER)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.data.update(cx, |d, _| d.dialog = None);
                                    if let Some(bid) = bid.clone() {
                                        this.select_thread(bid, tid.clone(), cx);
                                    }
                                    cx.notify();
                                }),
                            )
                            .child(label.clone()),
                    );
                }
            }
        }
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Search"),
            )
            .child(self.field("Query (Enter to search)", &self.dlg_search))
            .child(results)
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("search-close")
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

    fn attach_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Attach a file"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(t::TEXT_FAINT))
                    .child("Absolute path — images render inline, other files attach as links."),
            )
            .child(self.field("File path", &self.dlg_attach))
            .child(
                self.dialog_buttons(cx, "att-cancel", "att-ok", "Attach", |this, _w, cx| {
                    this.commit_attach(cx);
                }),
            )
    }

    fn rename_dialog(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        self.dialog_card(cx)
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(t::TEXT))
                    .child("Rename task"),
            )
            .child(self.field("Title", &self.dlg_rename))
            .child(
                self.dialog_buttons(cx, "ren-cancel", "ren-ok", "Rename", |this, _w, cx| {
                    this.commit_rename(cx);
                }),
            )
    }
}

fn mime_guess(name: &str) -> String {
    match name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" | "md" | "rs" | "py" | "js" | "ts" | "json" | "toml" | "yaml" | "yml" => "text/plain",
        _ => "application/octet-stream",
    }
    .to_string()
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
        let active = data.active.clone();
        let active_bot = match &active {
            Some(ChatTarget::Bot(id)) => data.bot(id).cloned(),
            _ => None,
        };
        let has_active = active.is_some();
        let _ = window;

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
                    .when(has_active, |d| {
                        d.child(self.chat_header(cx))
                            .child(self.messages(cx))
                            .child(self.composer(cx))
                    })
                    .when(!has_active, |d| d.child(self.messages(cx))),
            );

        if let Some(bot) = &active_bot {
            if self.model_menu {
                root = root.child(self.model_menu(bot, cx));
            }
            if self.tasks_menu {
                root = root.child(self.tasks_menu(&bot.id, cx));
            }
        }

        match dialog {
            Some(Dialog::NewBot) => {
                let card = self.new_bot_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::NewRoom) => {
                let card = self.new_room_dialog(cx);
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
            Some(Dialog::Archived) => {
                let card = self.archived_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::Search) => {
                let card = self.search_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::Attach) => {
                let card = self.attach_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            Some(Dialog::RenameThread(_)) => {
                let card = self.rename_dialog(cx);
                root = root.child(self.dialog_overlay(card, cx));
            }
            None => {}
        }
        root
    }
}
