//! Fathom Desktop — native GPUI desktop application for autonomous AI coworkers
//! with 1:1 OpenBot parity.

pub mod api;
pub mod app;
pub mod components;
pub mod daemon;
pub mod state;
pub mod theme;

use gpui::{
    App, Application, Bounds, WindowBounds, WindowOptions,
    prelude::*, px, size,
};
use std::sync::Arc;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Launching Fathom Desktop (GPUI)...");

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1220.0), px(780.0)), cx);
        let app_state = Arc::new(state::AppState::new());

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| app::DesktopApp::new(app_state, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
