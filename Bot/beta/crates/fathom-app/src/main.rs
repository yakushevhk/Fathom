//! Fathom — local-first chat app where every contact is a real agent.
//! Pure Rust + GPUI port of OpenMausBot: a roster of bots backed by local CLI
//! harnesses (claude / codex / fathom) or OpenAI-compatible endpoints (grok).

mod input;
mod state;
mod theme;
mod views;

use fathom_core::default_data_dir;
use fathom_core::Store;
use fathom_harness::AppState;
use gpui::{px, size, AppContext, Application, Bounds, WindowBounds, WindowOptions};
use state::AppData;

use views::RootView;

fn main() {
    // 1. Tokio runtime + in-process harness server (127.0.0.1:8799).
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let data_dir = std::env::var("FATHOM_BOT_DATA")
        .map(Into::into)
        .unwrap_or_else(|_| default_data_dir());
    let store = Store::open(data_dir).expect("open store");
    let harness = AppState::new(store);
    {
        let harness = harness.clone();
        rt.spawn(async move {
            if let Err(e) = fathom_harness::serve_state(harness).await {
                eprintln!("harness serve failed (already running?): {e}");
            }
        });
    }
    // Probe engines in the background.
    {
        let harness = harness.clone();
        rt.spawn(async move {
            let engines = harness.engine_statuses().await;
            let _ = harness
                .bus
                .send(fathom_core::ServerEvent::Engines { engines });
        });
    }
    let rt_handle = rt.handle().clone();

    // 2. GPUI app.
    Application::new().run(move |cx| {
        input::register_bindings(cx);

        let data = cx.new(|_| {
            let mut d = AppData::new(harness.clone(), rt_handle.clone());
            d.reload();
            d
        });

        // Pump harness broadcast events into the UI thread.
        {
            let data = data.clone();
            let harness2 = data.read(cx).harness.clone();
            cx.spawn(move |cx: &mut gpui::AsyncApp| {
                let cx = cx.clone();
                async move {
                    let mut rx = harness2.bus.subscribe();
                    loop {
                        match rx.recv().await {
                            Ok(ev) => {
                                let data = data.clone();
                                let _ = cx.update(move |app| {
                                    data.update(app, |d, cx| {
                                        // Load newly referenced threads lazily.
                                        if let fathom_core::ServerEvent::MessageUpsert { message } =
                                            &ev
                                        {
                                            if !d.messages.contains_key(&message.thread_id) {
                                                d.load_thread(&message.thread_id);
                                            }
                                        }
                                        d.apply(&ev);
                                        // Clear unread while the user is
                                        // looking at this thread.
                                        if let fathom_core::ServerEvent::MessageUpsert { message } =
                                            &ev
                                        {
                                            if d.active_thread().as_deref()
                                                == Some(&message.thread_id)
                                            {
                                                match &d.active {
                                                    Some(crate::state::ChatTarget::Bot(b)) => {
                                                        let _ = d.harness.store.mark_read(b);
                                                        if let Ok(Some(bot)) =
                                                            d.harness.store.get_bot(b)
                                                        {
                                                            d.apply(&fathom_core::ServerEvent::BotUpsert { bot });
                                                        }
                                                    }
                                                    Some(crate::state::ChatTarget::Room(r)) => {
                                                        let _ = d.harness.store.mark_room_read(r);
                                                        if let Ok(Some(room)) =
                                                            d.harness.store.get_room(r)
                                                        {
                                                            d.apply(&fathom_core::ServerEvent::RoomUpsert { room });
                                                        }
                                                    }
                                                    None => {}
                                                }
                                            }
                                        }
                                        d.reload();
                                        cx.notify();
                                    });
                                });
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            })
            .detach();
        }

        let bounds = Bounds::centered(None, size(px(1180.0), px(760.0)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: gpui::WindowBackgroundAppearance::Opaque,
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Fathom".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| RootView::new(window, cx, data)),
        );
    });
    // Keep the tokio runtime alive for the app's lifetime (it owns the harness).
    drop(rt);
}
