//! Turn orchestration: user message in → engine turn → streamed bot message.
//! One engine session per thread, spawned lazily and reused while alive.

use crate::engine::*;
use crate::state::AppState;
use fathom_core::types::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// A user send → kicks a turn in the background. Returns the created user
/// message + pending bot placeholder immediately; events stream over the bus.
pub async fn start_turn(
    state: &Arc<AppState>,
    bot_id: &str,
    thread_id: &str,
    text: &str,
    model: Option<String>,
) -> anyhow::Result<(Message, Message)> {
    let store = &state.store;
    let bot = store
        .get_bot(bot_id)?
        .ok_or_else(|| anyhow::anyhow!("bot {bot_id} not found"))?;
    let now = now_ms();

    let user_msg = Message {
        id: new_id("msg"),
        thread_id: thread_id.into(),
        role: Role::User,
        text: text.to_string(),
        engine: None,
        model: None,
        segments: vec![Segment::Text {
            text: text.to_string(),
        }],
        pending: false,
        error: None,
        created_at: now,
    };
    store.upsert_message(&user_msg)?;
    let _ = state.bus.send(ServerEvent::MessageUpsert {
        message: user_msg.clone(),
    });

    let bot_msg = Message {
        id: new_id("msg"),
        thread_id: thread_id.into(),
        role: Role::Bot,
        text: String::new(),
        engine: Some(bot.engine),
        model: model.clone().or(bot.model.clone()),
        segments: vec![],
        pending: true,
        error: None,
        created_at: now + 1,
    };
    store.upsert_message(&bot_msg)?;
    let _ = state.bus.send(ServerEvent::MessageUpsert {
        message: bot_msg.clone(),
    });

    store.touch_bot_activity(bot_id, text, false)?;
    store.set_bot_working(bot_id, true)?;
    if let Some(b) = store.get_bot(bot_id)? {
        let _ = state.bus.send(ServerEvent::BotUpsert { bot: b });
    }

    let state2 = state.clone();
    let bot2 = bot.clone();
    let bot_msg_id = bot_msg.id.clone();
    let thread_id = thread_id.to_string();
    let text_owned = text.to_string();
    tokio::spawn(async move {
        if let Err(e) = run_turn(
            state2.clone(),
            bot2.clone(),
            thread_id.clone(),
            bot_msg_id.clone(),
            text_owned,
            model,
        )
        .await
        {
            tracing::warn!("turn failed: {e:#}");
            fail_turn(&state2, &bot2, &thread_id, &bot_msg_id, format!("{e:#}"));
        }
    });

    Ok((user_msg, bot_msg))
}

async fn run_turn(
    state: Arc<AppState>,
    bot: Bot,
    thread_id: String,
    bot_msg_id: String,
    text: String,
    model: Option<String>,
) -> anyhow::Result<()> {
    // Get or spawn the engine session for this thread.
    let session_arc = {
        let mut map = state.sessions.lock().await;
        match map.get(&thread_id) {
            Some(s) => s.clone(),
            None => {
                let cfg = state.engine_config(bot.engine);
                let driver = state.driver_for(bot.engine);
                let session = driver.start_session(&cfg).await?;
                let arc = Arc::new(Mutex::new(session));
                map.insert(thread_id.clone(), arc.clone());
                arc
            }
        }
    };

    let (tx, mut rx) = broadcast::channel::<TurnEvent>(256);
    let input = TurnInput {
        prompt: text,
        system_prompt: if bot.soul.trim().is_empty() {
            None
        } else {
            Some(bot.soul.clone())
        },
        model: model.clone().or(bot.model.clone()),
        cwd: bot.cwd.clone().map(PathBuf::from),
        auto_approve: bot.auto_approve,
    };

    let decisions = state.decisions.clone();
    let mut session = session_arc.lock().await;

    // Subscribe BEFORE sending so nothing is lost.
    let mut session_fut = Box::pin(session.send_turn(input, tx, decisions));

    // Fold turn events into the pending message.
    let mut segments: Vec<Segment> = Vec::new();
    let mut text_acc = String::new();
    let store = &state.store;
    macro_rules! publish {
        ($pending:expr, $error:expr) => {{
            let msg = Message {
                id: bot_msg_id.clone(),
                thread_id: thread_id.clone(),
                role: Role::Bot,
                text: text_acc.clone(),
                engine: Some(bot.engine),
                model: model.clone().or(bot.model.clone()),
                segments: segments.clone(),
                pending: $pending,
                error: $error,
                created_at: now_ms(),
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
                    Ok(TurnEvent::PermissionRequest { request_id, title, detail }) => {
                        let approval = ApprovalRequest {
                            id: new_id("appr"),
                            bot_id: bot.id.clone(),
                            thread_id: thread_id.clone(),
                            request_id: request_id.clone(),
                            kind: "tool_use".into(),
                            title,
                            detail,
                            options: vec![],
                            status: ApprovalStatus::Pending,
                            response: None,
                            created_at: now_ms(),
                        };
                        let _ = store.upsert_approval(&approval);
                        segments.push(Segment::Approval { approval: approval.clone() });
                        let _ = state.bus.send(ServerEvent::ApprovalUpsert { approval });
                        publish!(true, None);
                    }
                    Ok(TurnEvent::Done { .. }) => {
                        break;
                    }
                    Ok(TurnEvent::Error(e)) => {
                        publish!(false, Some(e.clone()));
                        finish_turn(&state, &bot);
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
    finish_turn(&state, &bot);
    Ok(())
}

fn append_text(segments: &mut Vec<Segment>, t: String) {
    match segments.last_mut() {
        Some(Segment::Text { text }) => text.push_str(&t),
        _ => segments.push(Segment::Text { text: t }),
    }
}

fn finish_turn(state: &AppState, bot: &Bot) {
    let _ = state.store.set_bot_working(&bot.id, false);
    let _ = state.store.touch_bot_activity(&bot.id, "", false);
    if let Ok(Some(b)) = state.store.get_bot(&bot.id) {
        let _ = state.bus.send(ServerEvent::BotUpsert { bot: b });
    }
}

fn fail_turn(state: &AppState, bot: &Bot, thread_id: &str, bot_msg_id: &str, error: String) {
    if let Ok(msgs) = state.store.list_messages(thread_id, 1, None) {
        if let Some(m) = msgs.into_iter().find(|m| m.id == bot_msg_id) {
            let mut m = m;
            m.pending = false;
            m.error = Some(error.clone());
            let _ = state.store.upsert_message(&m);
            let _ = state.bus.send(ServerEvent::MessageUpsert { message: m });
        }
    }
    finish_turn(state, bot);
    let _ = state.bus.send(ServerEvent::TurnError {
        thread_id: thread_id.into(),
        bot_id: bot.id.clone(),
        error,
    });
}

/// Kill the live engine session for a thread (stop button / bot removal).
pub async fn abort_thread(state: &AppState, thread_id: &str) {
    let sess = state.sessions.lock().await.remove(thread_id);
    if let Some(s) = sess {
        s.lock().await.abort().await;
    }
}

/// Resolve a pending approval — shared by the HTTP route and the in-process UI.
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
        "allow" => {
            a.status = ApprovalStatus::Allowed;
            ApprovalDecision::Allow
        }
        "deny" => {
            a.status = ApprovalStatus::Denied;
            a.response = reason.clone().or(Some("Denied".into()));
            ApprovalDecision::Deny(reason.unwrap_or_else(|| "Denied by user".into()))
        }
        answer => {
            a.status = ApprovalStatus::Answered;
            a.response = Some(answer.to_string());
            ApprovalDecision::Deny(answer.to_string())
        }
    };
    let _ = state.store.upsert_approval(&a);
    let _ = state.bus.send(ServerEvent::ApprovalUpsert {
        approval: a.clone(),
    });
    if let Some(tx) = state.decisions.lock().await.remove(&a.request_id) {
        let _ = tx.send(dec);
    }
    true
}
