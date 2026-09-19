//! Turn orchestration: user message in → engine turn → streamed bot message.
//! One engine session per (thread, bot) pair, spawned lazily and reused while
//! alive. Sends during a running turn queue up and drain in order; engines
//! that can steer a live session (claude stdin) get the message mid-turn.

use crate::engine::*;
use crate::state::AppState;
use fathom_core::types::*;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// Everything a send carries beyond the thread id.
#[derive(Debug, Clone, Default)]
pub struct SendOpts {
    pub text: String,
    pub model: Option<String>,
    pub reply_to: Option<String>,
    pub attachments: Vec<String>,
    pub sender: Option<String>,
    pub via_api: bool,
    /// Stable client id — a retry with the same send_id returns the stored
    /// message instead of sending twice.
    pub send_id: Option<String>,
    /// Per-send channel behavior ("chat" default | "goal" reserved).
    pub channel_mode: Option<String>,
}

/// A queued send while a thread is mid-turn.
#[derive(Debug, Clone)]
pub struct QueuedSend {
    pub queue_id: String,
    pub msg_id: String,
    pub bot_id: String,
    pub opts: SendOpts,
    /// Already injected mid-turn via the session's steer channel — the drain
    /// only clears the queued flag instead of running another turn.
    pub steered: bool,
}

/// What a send did: started a turn now, queued behind a running one, or hit
/// a send_id that already landed.
#[allow(clippy::large_enum_variant)]
pub enum SendOutcome {
    Started { user: Message, pending: Message },
    Queued { user: Message },
    Deduped { user: Message },
}

/// Push a `queued_messages` snapshot for a thread — mirrors the bot.queued
/// frame on the OpenMausBot wire.
async fn emit_queue(state: &AppState, key: &str, thread_id: &str) {
    let items: Vec<QueuedItem> = state
        .queues
        .lock()
        .await
        .get(key)
        .map(|v| {
            v.iter()
                .map(|q| QueuedItem {
                    queue_id: q.queue_id.clone(),
                    text: q.opts.text.clone(),
                    reason: if q.steered {
                        Some("steer".into())
                    } else {
                        Some("busy".into())
                    },
                })
                .collect()
        })
        .unwrap_or_default();
    let _ = state.bus.send(ServerEvent::QueuedMessages {
        thread_id: thread_id.to_string(),
        items,
    });
}

/// Title an untitled thread from the first user message (WireTask
/// titleFromFirstMessage). Emits ThreadUpsert when it renames.
fn title_from_first_message(state: &AppState, thread_id: &str, text: &str) {
    let title: String = text.trim().chars().take(48).collect();
    if title.is_empty() {
        return;
    }
    if let Ok(true) = state.store.title_thread_once(thread_id, &title) {
        if let Ok(Some(t)) = state.store.get_thread(thread_id) {
            let _ = state.bus.send(ServerEvent::ThreadUpsert { thread: t });
        }
    }
}

fn session_key(thread_id: &str, bot_id: &str, room: bool) -> String {
    if room {
        format!("{thread_id}#{bot_id}")
    } else {
        thread_id.to_string()
    }
}

/// A user send → kicks a turn in the background (or queues behind a running
/// one). Returns the persisted user message; events stream over the bus.
pub async fn start_turn(
    state: &Arc<AppState>,
    bot_id: &str,
    thread_id: &str,
    opts: SendOpts,
) -> anyhow::Result<SendOutcome> {
    let store = &state.store;
    let bot = store
        .get_bot(bot_id)?
        .ok_or_else(|| anyhow::anyhow!("bot {bot_id} not found"))?;
    let now = now_ms();

    // Idempotent retries: a repeated send_id returns the stored line.
    if let Some(sid) = &opts.send_id {
        if let Ok(Some(existing)) = store.find_by_send_id(thread_id, sid) {
            return Ok(SendOutcome::Deduped { user: existing });
        }
    }

    let attachments = store.attachments_by_ids(&opts.attachments);

    let mut user_msg = Message::blank(thread_id, Role::User, opts.text.clone());
    user_msg.reply_to = opts.reply_to.clone();
    user_msg.attachments = attachments.clone();
    user_msg.sender = opts
        .sender
        .clone()
        .or_else(|| profile_name(state).filter(|s| !s.is_empty()));
    user_msg.via_api = opts.via_api;
    user_msg.send_id = opts.send_id.clone();
    user_msg.channel_mode = opts.channel_mode.clone();
    user_msg.created_at = now;

    let key = session_key(thread_id, bot_id, false);
    // Atomic busy-check: inserting marks the thread running before we spawn.
    let busy = !state.running.lock().await.insert(key.clone());

    if busy {
        // If the engine can steer a live session, feed it mid-turn.
        let steered = if !bot.park_dms {
            state
                .steers
                .lock()
                .await
                .get(&key)
                .map(|tx| tx.send(user_msg.text.clone()).is_ok())
                .unwrap_or(false)
        } else {
            false
        };
        user_msg.queued = true;
        user_msg.steered = steered;
        user_msg.queue_id = Some(new_id("q"));
        store.upsert_message(&user_msg)?;
        let _ = state.bus.send(ServerEvent::MessageUpsert {
            message: user_msg.clone(),
        });
        state
            .queues
            .lock()
            .await
            .entry(key.clone())
            .or_insert_with(VecDeque::new)
            .push_back(QueuedSend {
                queue_id: user_msg.queue_id.clone().unwrap_or_else(|| new_id("q")),
                msg_id: user_msg.id.clone(),
                bot_id: bot_id.to_string(),
                opts,
                steered,
            });
        emit_queue(state, &key, thread_id).await;
        return Ok(SendOutcome::Queued { user: user_msg });
    }

    title_from_first_message(state, thread_id, &user_msg.text);
    store.upsert_message(&user_msg)?;
    let _ = state.bus.send(ServerEvent::MessageUpsert {
        message: user_msg.clone(),
    });

    let pending = kick(state, &bot, thread_id, opts, false);
    Ok(SendOutcome::Started {
        user: user_msg,
        pending,
    })
}

/// Create the pending bot placeholder and spawn the turn task — synchronous
/// so callers (including the queue drain) never await a self-recursive future.
fn kick(state: &Arc<AppState>, bot: &Bot, thread_id: &str, opts: SendOpts, room: bool) -> Message {
    let store = &state.store;
    let bot_msg = Message {
        engine: Some(bot.engine),
        model: opts.model.clone().or(bot.model.clone()),
        pending: true,
        created_at: now_ms() + 1,
        from_bot: if room {
            Some(FromBot {
                bot_id: bot.id.clone(),
                name: bot.name.clone(),
                avatar_seed: bot.avatar_seed,
            })
        } else {
            None
        },
        ..Message::blank(thread_id, Role::Bot, String::new())
    };
    let _ = store.upsert_message(&bot_msg);
    let _ = state.bus.send(ServerEvent::MessageUpsert {
        message: bot_msg.clone(),
    });
    if !room {
        let _ = store.touch_bot_activity(&bot.id, &opts.text, false);
        let _ = store.set_bot_working(&bot.id, true);
    } else if let Ok(Some(r)) = store.get_room_by_thread(thread_id) {
        let _ = store.set_room_busy(&r.id, Some(&bot.id));
    }
    let state2 = state.clone();
    let bot_id = bot.id.clone();
    let bot_msg_id = bot_msg.id.clone();
    let thread_id = thread_id.to_string();
    let key = session_key(&thread_id, &bot_id, room);
    // Mark running synchronously so concurrent sends queue instead of racing
    // the spawned task.
    if let Ok(mut r) = state.running.try_lock() {
        r.insert(key.clone());
    }
    tokio::spawn(async move {
        state2.running.lock().await.insert(key.clone());
        if let Err(e) = run_turn(
            state2.clone(),
            bot_id.clone(),
            thread_id.clone(),
            bot_msg_id.clone(),
            opts,
            room,
        )
        .await
        {
            tracing::warn!("turn failed: {e:#}");
            fail_turn(
                &state2,
                &bot_id,
                &thread_id,
                &bot_msg_id,
                format!("{e:#}"),
                room,
            )
            .await;
        }
        drain_queue(&state2, &key, &thread_id).await;
    });
    bot_msg
}

/// Run queued sends in order after the active turn finishes.
async fn drain_queue(state: &Arc<AppState>, key: &str, thread_id: &str) {
    loop {
        let next = {
            let mut q = state.queues.lock().await;
            q.get_mut(key).and_then(|v| v.pop_front())
        };
        let Some(q) = next else { break };
        emit_queue(state, key, thread_id).await;
        // Clear the queued flag on the stored user message.
        if let Ok(Some(mut m)) = state.store.get_message(&q.msg_id) {
            m.queued = false;
            m.steered = m.steered || q.steered;
            let _ = state.store.upsert_message(&m);
            let _ = state.bus.send(ServerEvent::MessageUpsert { message: m });
        }
        if q.steered {
            // Already answered inside the previous turn — no new turn needed.
            continue;
        }
        let Ok(Some(bot)) = state.store.get_bot(&q.bot_id) else {
            continue;
        };
        // kick() spawns the turn task; wait until the thread goes idle before
        // draining the next item.
        kick(state, &bot, thread_id, q.opts, false);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
            if !state.running.lock().await.contains(key) {
                break;
            }
        }
    }
}

fn profile_name(state: &AppState) -> Option<String> {
    state.store.get_kv("profile_name").ok().flatten()
}

/// Emit a BotUpsert with the transient waiting flag resolved.
pub async fn emit_bot(state: &AppState, bot_id: &str) {
    if let Ok(Some(mut b)) = state.store.get_bot(bot_id) {
        b.waiting_on_you = state
            .store
            .pending_approvals(Some(bot_id))
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let _ = state.bus.send(ServerEvent::BotUpsert { bot: b });
    }
}

pub async fn emit_room(state: &AppState, room_id: &str) {
    if let Ok(Some(r)) = state.store.get_room(room_id) {
        let _ = state.bus.send(ServerEvent::RoomUpsert { room: r });
    }
}

async fn run_turn(
    state: Arc<AppState>,
    bot_id: String,
    thread_id: String,
    bot_msg_id: String,
    opts: SendOpts,
    room: bool,
) -> anyhow::Result<()> {
    let Some(bot) = state.store.get_bot(&bot_id)? else {
        anyhow::bail!("bot {bot_id} not found");
    };
    let key = session_key(&thread_id, &bot_id, room);

    // Get or spawn the engine session for this (thread, bot) pair.
    let session_arc = {
        let mut map = state.sessions.lock().await;
        match map.get(&key) {
            Some(s) => s.clone(),
            None => {
                let cfg = state.engine_config(bot.engine);
                let driver = state.driver_for(bot.engine);
                let session = driver.start_session(&cfg).await?;
                let arc = Arc::new(Mutex::new(session));
                map.insert(key.clone(), arc.clone());
                arc
            }
        }
    };

    // Steer channel: queued sends feed into a live stdin session.
    let (steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();
    state.steers.lock().await.insert(key.clone(), steer_tx);

    let (tx, mut rx) = broadcast::channel::<TurnEvent>(256);
    let mut prompt = opts.text.clone();
    if !opts.attachments.is_empty() {
        let files = state
            .store
            .attachments_by_ids(&opts.attachments)
            .iter()
            .map(|a| a.path.clone())
            .collect::<Vec<_>>();
        prompt.push_str("\n\n[Attachments]\n");
        for f in &files {
            prompt.push_str(&format!("- {f}\n"));
        }
    }
    if let Some(sender) = &opts.sender {
        if room {
            prompt = format!("[room message from {sender}] {prompt}");
        }
    }
    // Room context: shared bulletin rides ahead of each member's persona and
    // the room's desk overrides each member's default cwd.
    let room_ctx = if room {
        state.store.get_room_by_thread(&thread_id).ok().flatten()
    } else {
        None
    };
    let thread_cwd = state
        .store
        .get_thread(&thread_id)
        .ok()
        .flatten()
        .and_then(|t| t.cwd);
    let mut system_prompt = if bot.soul.trim().is_empty() {
        None
    } else {
        Some(bot.soul.clone())
    };
    if let Some(r) = &room_ctx {
        if !r.bulletin.trim().is_empty() {
            let b = format!("[Room bulletin]\n{}", r.bulletin.trim());
            system_prompt = Some(match system_prompt {
                Some(s) => format!("{b}\n\n{s}"),
                None => b,
            });
        }
    }
    let cwd = room_ctx
        .and_then(|r| r.cwd)
        .or(thread_cwd)
        .or(bot.cwd.clone());
    let input = TurnInput {
        prompt,
        system_prompt,
        model: opts.model.clone().or(bot.model.clone()),
        effort: bot
            .effort
            .clone()
            .or(state.engine_config(bot.engine).effort),
        cwd: cwd.map(PathBuf::from),
        approval_mode: bot.approval_mode,
        always_allow: bot.always_allow.clone(),
        attachments: state
            .store
            .attachments_by_ids(&opts.attachments)
            .iter()
            .map(|a| PathBuf::from(&a.path))
            .collect(),
        sender: opts.sender.clone().or_else(|| profile_name(&state)),
        steer_rx: Some(steer_rx),
    };

    let decisions = state.decisions.clone();
    let mut session = session_arc.lock().await;
    let turn_id = new_id("turn");

    // Subscribe BEFORE sending so nothing is lost.
    let mut session_fut = Box::pin(session.send_turn(input, tx, decisions));

    // Fold turn events into the pending message.
    let mut segments: Vec<Segment> = Vec::new();
    let mut text_acc = String::new();
    let store = &state.store;
    macro_rules! publish {
        ($pending:expr, $error:expr) => {{
            let err: Option<String> = $error;
            let msg = Message {
                id: bot_msg_id.clone(),
                thread_id: thread_id.clone(),
                kind: "text".into(),
                text: text_acc.clone(),
                engine: Some(bot.engine),
                model: opts.model.clone().or(bot.model.clone()),
                segments: segments.clone(),
                from_bot: if room {
                    Some(FromBot {
                        bot_id: bot.id.clone(),
                        name: bot.name.clone(),
                        avatar_seed: bot.avatar_seed,
                    })
                } else {
                    None
                },
                pending: $pending,
                error: err.clone(),
                turn_id: Some(turn_id.clone()),
                // The settled snapshot is the terminal text of this turn.
                turn_terminal: !$pending && err.is_none(),
                ..Message::blank(&thread_id, Role::Bot, String::new())
            };
            let _ = store.upsert_message(&msg);
            let _ = state.bus.send(ServerEvent::MessageUpsert { message: msg });
        }};
    }

    loop {
        tokio::select! {
            _ = &mut session_fut => break,
            ev = rx.recv() => {
                match ev {
                    Ok(TurnEvent::Text(t)) => {
                        text_acc.push_str(&t);
                        append_text(&mut segments, t);
                        publish!(true, None);
                    }
                    Ok(TurnEvent::Thinking(t)) => {
                        match segments.last_mut() {
                            Some(Segment::Thinking { text }) => text.push_str(&t),
                            _ => segments.push(Segment::Thinking { text: t }),
                        }
                        publish!(true, None);
                    }
                    Ok(TurnEvent::ToolUse { name, input }) => {
                        segments.push(Segment::ToolCall { name, input, output: None, status: ToolStatus::Running });
                        publish!(true, None);
                    }
                    Ok(TurnEvent::ToolResult { name: _, output, ok }) => {
                        // Attach to the most recent running tool call.
                        for seg in segments.iter_mut().rev() {
                            if let Segment::ToolCall { output: o, status, .. } = seg {
                                if *status == ToolStatus::Running {
                                    *o = Some(output.clone());
                                    *status = if ok { ToolStatus::Done } else { ToolStatus::Failed };
                                    break;
                                }
                            }
                        }
                        publish!(true, None);
                    }
                    Ok(TurnEvent::PermissionRequest { request_id, title, detail, tool }) => {
                        // always_allow grants the tool without opening a card.
                        let granted = tool
                            .as_deref()
                            .map(|t| bot.always_allow.iter().any(|a| a == t))
                            .unwrap_or(false);
                        let mut approval = ApprovalRequest {
                            id: new_id("appr"),
                            bot_id: bot.id.clone(),
                            thread_id: thread_id.clone(),
                            request_id: request_id.clone(),
                            kind: "tool_use".into(),
                            title,
                            detail,
                            tool,
                            options: vec![],
                            held: None,
                            held_code: None,
                            allow_session: true,
                            dismissed: false,
                            status: if granted { ApprovalStatus::Allowed } else { ApprovalStatus::Pending },
                            response: granted.then(|| "always allow".to_string()),
                            created_at: now_ms(),
                        };
                        let _ = store.upsert_approval(&approval);
                        if granted {
                            resolve_approval(&state, &approval.id, "allow", None).await;
                            if let Ok(Some(a)) = store.get_approval(&approval.id) {
                                approval = a;
                            }
                        }
                        segments.push(Segment::Approval { approval: approval.clone() });
                        let _ = state.bus.send(ServerEvent::ApprovalUpsert { approval });
                        emit_bot(&state, &bot.id).await;
                        publish!(true, None);
                    }
                    Ok(TurnEvent::Done { .. }) => {
                        break;
                    }
                    Ok(TurnEvent::Error(e)) => {
                        publish!(false, Some(e.clone()));
                        finish_turn(&state, &bot, &key, room).await;
                        let _ = state.bus.send(ServerEvent::TurnError { thread_id, bot_id: bot.id, error: e });
                        return Ok(());
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        }
    }

    publish!(false, None);
    finish_turn(&state, &bot, &key, room).await;
    Ok(())
}

async fn finish_turn(state: &AppState, bot: &Bot, key: &str, room: bool) {
    state.steers.lock().await.remove(key);
    state.running.lock().await.remove(key);
    if room {
        if let Ok(Some(r)) = state
            .store
            .get_room_by_thread(key.split('#').next().unwrap_or_default())
        {
            let _ = state.store.set_room_busy(&r.id, None);
            let _ = state.store.touch_room_activity(&r.id, "", true);
            emit_room(state, &r.id).await;
        }
        return;
    }
    let _ = state.store.set_bot_working(&bot.id, false);
    // Bot replied: bump unread — the app clears it while the thread is open.
    let _ = state.store.touch_bot_activity(&bot.id, "", true);
    emit_bot(state, &bot.id).await;
}

fn append_text(segments: &mut Vec<Segment>, t: String) {
    match segments.last_mut() {
        Some(Segment::Text { text }) => text.push_str(&t),
        _ => segments.push(Segment::Text { text: t }),
    }
}

async fn fail_turn(
    state: &AppState,
    bot_id: &str,
    thread_id: &str,
    bot_msg_id: &str,
    error: String,
    room: bool,
) {
    if let Ok(Some(mut m)) = state.store.get_message(bot_msg_id) {
        m.pending = false;
        m.error = Some(error.clone());
        let _ = state.store.upsert_message(&m);
        let _ = state.bus.send(ServerEvent::MessageUpsert { message: m });
    }
    let key = session_key(thread_id, bot_id, room);
    state.running.lock().await.remove(&key);
    state.steers.lock().await.remove(&key);
    let _ = state.store.set_bot_working(bot_id, false);
    if room {
        if let Ok(Some(r)) = state.store.get_room_by_thread(thread_id) {
            let _ = state.store.set_room_busy(&r.id, None);
            emit_room(state, &r.id).await;
        }
    }
    emit_bot(state, bot_id).await;
    let _ = state.bus.send(ServerEvent::TurnError {
        thread_id: thread_id.into(),
        bot_id: bot_id.into(),
        error,
    });
}

/// Kill the live engine session for a thread (stop button / bot removal).
pub async fn abort_thread(state: &AppState, thread_id: &str) {
    let keys: Vec<String> = {
        let map = state.sessions.lock().await;
        map.keys()
            .filter(|k| k.as_str() == thread_id || k.starts_with(&format!("{thread_id}#")))
            .cloned()
            .collect()
    };
    let mut map = state.sessions.lock().await;
    for k in keys {
        if let Some(s) = map.remove(&k) {
            s.lock().await.abort().await;
        }
        state.running.lock().await.remove(&k);
        state.steers.lock().await.remove(&k);
        state.queues.lock().await.remove(&k);
    }
    // Clear working flags so rosters don't show stuck spinners.
    if let Ok(Some(t)) = state.store.get_thread(thread_id) {
        if let Some(bid) = t.bot_id {
            let _ = state.store.set_bot_working(&bid, false);
            emit_bot(state, &bid).await;
        } else if let Ok(Some(r)) = state.store.get_room_by_thread(&t.id) {
            emit_room(state, &r.id).await;
        }
    }
}

/// Resolve a pending approval — shared by the HTTP route and the in-process UI.
/// `decision`: "allow" | "deny" | "always" (allow + remember the tool).
/// Returns false if the approval is missing or already resolved.
pub async fn resolve_approval(
    state: &AppState,
    id: &str,
    decision: &str,
    reason: Option<String>,
) -> bool {
    let Ok(Some(mut a)) = state.store.get_approval(id) else {
        return false;
    };
    if a.status != ApprovalStatus::Pending {
        return false;
    }
    let dec = match decision {
        "allow" | "always" => {
            a.status = ApprovalStatus::Allowed;
            a.response = Some(if decision == "always" {
                "always allow".into()
            } else {
                "allowed".into()
            });
            ApprovalDecision::Allow
        }
        "deny" => {
            a.status = ApprovalStatus::Denied;
            a.response = reason.clone().or(Some("Denied".into()));
            ApprovalDecision::Deny(reason.unwrap_or_else(|| "Denied by user".into()))
        }
        answer => {
            // Structured option pick — answers the engine's question.
            a.status = ApprovalStatus::Answered;
            a.response = Some(answer.to_string());
            ApprovalDecision::Deny(answer.to_string())
        }
    };
    let _ = state.store.upsert_approval(&a);
    if decision == "always" {
        if let (Some(tool), Ok(Some(mut bot))) = (a.tool.clone(), state.store.get_bot(&a.bot_id)) {
            if !bot.always_allow.iter().any(|t| t == &tool) {
                bot.always_allow.push(tool);
                let _ = state.store.upsert_bot(&bot);
            }
        }
    }
    let _ = state.bus.send(ServerEvent::ApprovalUpsert {
        approval: a.clone(),
    });
    emit_bot(state, &a.bot_id).await;
    if let Some(tx) = state.decisions.lock().await.remove(&a.request_id) {
        let _ = tx.send(dec);
    }
    true
}

// ---- rooms -------------------------------------------------------------------

/// User message into a room thread: persists the line, then each responding
/// member bot takes a turn in roster order. `@name` mentions narrow the
/// responders when the room is on Mentions mode.
pub async fn start_room_turn(
    state: &Arc<AppState>,
    room_id: &str,
    opts: SendOpts,
) -> anyhow::Result<Message> {
    let store = &state.store;
    let Some(room) = store.get_room(room_id)? else {
        anyhow::bail!("room {room_id} not found");
    };

    // Idempotent retries on the room thread too.
    if let Some(sid) = &opts.send_id {
        if let Ok(Some(existing)) = store.find_by_send_id(&room.thread_id, sid) {
            return Ok(existing);
        }
    }

    let attachments = store.attachments_by_ids(&opts.attachments);
    let mut user_msg = Message::blank(&room.thread_id, Role::User, opts.text.clone());
    user_msg.reply_to = opts.reply_to.clone();
    user_msg.attachments = attachments;
    user_msg.sender = opts
        .sender
        .clone()
        .or_else(|| profile_name(state))
        .or_else(|| Some("You".into()));
    user_msg.via_api = opts.via_api;
    user_msg.send_id = opts.send_id.clone();
    user_msg.channel_mode = opts.channel_mode.clone();
    store.upsert_message(&user_msg)?;
    let _ = state.bus.send(ServerEvent::MessageUpsert {
        message: user_msg.clone(),
    });
    store.touch_room_activity(room_id, &opts.text, false)?;
    emit_room(state, room_id).await;

    // Who answers?
    let members: Vec<Bot> = room
        .member_ids
        .iter()
        .filter_map(|id| store.get_bot(id).ok().flatten())
        .collect();
    let mentioned: Vec<Bot> = members
        .iter()
        .filter(|b| {
            opts.text
                .to_lowercase()
                .contains(&format!("@{}", b.name.to_lowercase()))
        })
        .cloned()
        .collect();
    let responders: Vec<Bot> = match &room.responder {
        Responder::Member { bot_id } => members
            .iter()
            .filter(|b| &b.id == bot_id)
            .cloned()
            .collect(),
        Responder::Everyone => members.clone(),
        Responder::Mentions => {
            if mentioned.is_empty() {
                members.iter().take(1).cloned().collect()
            } else {
                mentioned
            }
        }
    };

    // Member turns run sequentially on the shared room thread.
    let state2 = state.clone();
    let room2 = room.clone();
    let text = opts.text.clone();
    let sender = user_msg.sender.clone();
    tokio::spawn(async move {
        for bot in responders {
            let key = session_key(&room2.thread_id, &bot.id, true);
            let opts2 = SendOpts {
                text: text.clone(),
                sender: sender.clone(),
                attachments: Vec::new(),
                ..Default::default()
            };
            kick(&state2, &bot, &room2.thread_id, opts2, true);
            // Sequential: wait for this member's turn to finish.
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                if !state2.running.lock().await.contains(&key) {
                    break;
                }
            }
        }
        emit_room(&state2, &room2.id).await;
    });

    Ok(user_msg)
}

// ---- peer comms (bot→bot internal tools) -------------------------------------

/// Resolve a bot by id or by (case-insensitive) name.
pub fn find_bot(state: &AppState, id_or_name: &str) -> Option<Bot> {
    if let Ok(Some(b)) = state.store.get_bot(id_or_name) {
        return Some(b);
    }
    let needle = id_or_name.to_lowercase();
    state
        .store
        .list_bots_filtered(false)
        .ok()?
        .into_iter()
        .chain(state.store.list_bots_filtered(true).unwrap_or_default())
        .find(|b| b.name.to_lowercase() == needle)
}

/// Human-in-the-loop gate for bots flagged `approve_peer_comms`: opens a
/// `peer_comm` approval card and waits up to 5 min for a decision.
async fn gate_peer_call(
    state: &Arc<AppState>,
    caller: Option<&Bot>,
    what: &str,
) -> anyhow::Result<()> {
    let Some(bot) = caller else { return Ok(()) };
    if !bot.approve_peer_comms {
        return Ok(());
    }
    let a = ApprovalRequest {
        id: new_id("appr"),
        bot_id: bot.id.clone(),
        thread_id: String::new(),
        request_id: String::new(),
        kind: "peer_comm".into(),
        title: format!("{} wants to {what}", bot.name),
        detail: "Approve this peer call?".into(),
        tool: None,
        options: vec![],
        held: Some("approvePeerComms".into()),
        held_code: Some("peer_comms".into()),
        allow_session: true,
        dismissed: false,
        status: ApprovalStatus::Pending,
        response: None,
        created_at: now_ms(),
    };
    let id = a.id.clone();
    state.store.upsert_approval(&a)?;
    let _ = state.bus.send(ServerEvent::ApprovalUpsert { approval: a });
    emit_bot(state, &bot.id).await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        match state.store.get_approval(&id)? {
            Some(a) => match a.status {
                ApprovalStatus::Pending => {}
                ApprovalStatus::Allowed | ApprovalStatus::Answered => return Ok(()),
                ApprovalStatus::Denied => anyhow::bail!("peer call denied"),
            },
            None => anyhow::bail!("peer call approval vanished"),
        }
        if std::time::Instant::now() > deadline {
            anyhow::bail!("peer call approval timed out");
        }
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }
}

/// Bot → bot DM ("ask a peer"): drops a user-role line carrying a `peer_ask`
/// chip into the target's DM thread, runs the turn, and waits for the reply
/// (bounded at ~10 min — engine turns can be long).
pub async fn ask_bot(
    state: &Arc<AppState>,
    target: &str,
    text: &str,
    caller_id: Option<&str>,
    caller_name: Option<&str>,
    unattended: bool,
) -> anyhow::Result<Message> {
    let caller_bot = caller_id.and_then(|id| state.store.get_bot(id).ok().flatten());
    gate_peer_call(state, caller_bot.as_ref(), &format!("ask @{target}")).await?;
    let Some(bot) = find_bot(state, target) else {
        anyhow::bail!("bot {target} not found");
    };
    if let Some(c) = &caller_bot {
        if !c.peers.is_empty() && !c.peers.contains(&bot.id) {
            anyhow::bail!("{} is not allowed to contact {target}", c.name);
        }
    }
    let thread = state.store.direct_thread(&bot.id)?;
    let mut user_msg = Message::blank(&thread.id, Role::User, text.to_string());
    user_msg.peer_ask = Some(PeerRef {
        bot_id: caller_id.map(|s| s.to_string()),
        name: caller_name.map(|s| s.to_string()),
        unattended,
    });
    user_msg.sender = caller_name.map(|s| format!("@{s}"));
    state.store.upsert_message(&user_msg)?;
    let _ = state
        .bus
        .send(ServerEvent::MessageUpsert { message: user_msg });
    let opts = SendOpts {
        text: text.to_string(),
        sender: caller_name.map(|s| format!("bot {s}")),
        ..Default::default()
    };
    let key = session_key(&thread.id, &bot.id, false);
    match start_turn(state, &bot.id, &thread.id, opts).await? {
        SendOutcome::Deduped { .. } | SendOutcome::Queued { .. } => {}
        SendOutcome::Started { .. } => {}
    }
    // Wait for the turn to settle (bounded).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    while state.running.lock().await.contains(&key) {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("ask-bot timed out waiting for {target}");
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    let msgs = state.store.list_messages(&thread.id, 5, None)?;
    let reply = msgs
        .into_iter()
        .rev()
        .find(|m| m.role == Role::Bot && !m.pending)
        .ok_or_else(|| anyhow::anyhow!("no reply from {target}"))?;
    Ok(reply)
}

/// Bot → bot delegation: open a fresh task thread on the target, fire the
/// turn in the background, return the task handle immediately.
pub async fn delegate_bot(
    state: &Arc<AppState>,
    target: &str,
    text: &str,
    title: Option<String>,
    caller_id: Option<&str>,
    caller_name: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let caller_bot = caller_id.and_then(|id| state.store.get_bot(id).ok().flatten());
    gate_peer_call(
        state,
        caller_bot.as_ref(),
        &format!("delegate to @{target}"),
    )
    .await?;
    let Some(bot) = find_bot(state, target) else {
        anyhow::bail!("bot {target} not found");
    };
    if let Some(c) = &caller_bot {
        if !c.peers.is_empty() && !c.peers.contains(&bot.id) {
            anyhow::bail!("{} is not allowed to contact {target}", c.name);
        }
    }
    let thread = state.store.new_task_thread(
        &bot.id,
        title.or_else(|| Some(text.chars().take(48).collect())),
    )?;
    let _ = state.bus.send(ServerEvent::ThreadUpsert {
        thread: thread.clone(),
    });
    let opts = SendOpts {
        text: text.to_string(),
        sender: caller_name.map(|s| format!("bot {s}")),
        ..Default::default()
    };
    start_turn(state, &bot.id, &thread.id, opts).await?;
    Ok(serde_json::json!({
        "bot_id": bot.id,
        "thread_id": thread.id,
        "delegation_id": thread.id,
        "status": "running",
    }))
}

/// Bot → room push ("post to room"): appends a bot-role line with a
/// `peer_post` chip — the room sees who spoke without a turn running.
pub async fn post_to_room(
    state: &Arc<AppState>,
    bot: &Bot,
    room_id_or_name: &str,
    text: &str,
    unattended: bool,
) -> anyhow::Result<Message> {
    gate_peer_call(state, Some(bot), &format!("post to room {room_id_or_name}")).await?;
    let room = {
        let direct = state.store.get_room(room_id_or_name)?;
        match direct {
            Some(r) => r,
            None => {
                let needle = room_id_or_name.to_lowercase();
                state
                    .store
                    .list_rooms()?
                    .into_iter()
                    .find(|r| r.name.to_lowercase() == needle)
                    .ok_or_else(|| anyhow::anyhow!("room {room_id_or_name} not found"))?
            }
        }
    };
    let mut m = Message::blank(&room.thread_id, Role::Bot, text.to_string());
    m.from_bot = Some(FromBot {
        bot_id: bot.id.clone(),
        name: bot.name.clone(),
        avatar_seed: bot.avatar_seed,
    });
    m.peer_post = Some(PeerRef {
        bot_id: Some(bot.id.clone()),
        name: Some(bot.name.clone()),
        unattended,
    });
    state.store.upsert_message(&m)?;
    let _ = state
        .bus
        .send(ServerEvent::MessageUpsert { message: m.clone() });
    state.store.touch_room_activity(&room.id, text, true)?;
    emit_room(state, &room.id).await;
    Ok(m)
}

/// Bot → coordinate a group: creates (or reuses) a room with the caller plus
/// the named members, sets the directive as its bulletin, and posts the
/// directive so members take a turn.
pub async fn coordinate_bots(
    state: &Arc<AppState>,
    caller: &Bot,
    members: &[String],
    directive: &str,
    room_name: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    gate_peer_call(state, Some(caller), "coordinate bots").await?;
    let mut member_ids: Vec<String> = vec![caller.id.clone()];
    for m in members {
        let Some(b) = find_bot(state, m) else {
            anyhow::bail!("member {m} not found");
        };
        if !member_ids.contains(&b.id) {
            member_ids.push(b.id);
        }
    }
    let now = now_ms();
    let room_id = new_id("room");
    let thread = Thread {
        id: new_id("th"),
        kind: "room".into(),
        bot_id: None,
        room_id: Some(room_id.clone()),
        title: room_name.map(|s| s.to_string()),
        pinned_message_id: None,
        title_from_first_message: false,
        archived_at: None,
        cwd: None,
        rewound: false,
        turn_started_at: None,
        created_at: now,
        updated_at: now,
    };
    state.store.upsert_thread(&thread)?;
    let room = Room {
        id: room_id.clone(),
        name: room_name.unwrap_or("coordination").to_string(),
        member_ids,
        thread_id: thread.id.clone(),
        responder: Responder::Everyone,
        bulletin: directive.to_string(),
        dm: false,
        section: None,
        cwd: None,
        working: false,
        busy_bot_id: None,
        turn_started_at: None,
        setup_completed_at: Some(now),
        setup_skipped_at: None,
        unread: 0,
        last_message: None,
        last_activity_at: Some(now),
        created_at: now,
        updated_at: now,
    };
    state.store.upsert_room(&room)?;
    let _ = state
        .bus
        .send(ServerEvent::RoomUpsert { room: room.clone() });
    // Post the directive and let members answer.
    start_room_turn(
        state,
        &room_id,
        SendOpts {
            text: directive.to_string(),
            sender: Some(format!("bot {}", caller.name)),
            ..Default::default()
        },
    )
    .await?;
    Ok(serde_json::json!({
        "room_id": room.id,
        "thread_id": room.thread_id,
        "members": room.member_ids,
    }))
}
