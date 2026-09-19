pub mod api;
pub mod drivers;
pub mod engine;
pub mod sessions;
pub mod state;

pub use state::AppState;

/// Run the harness server on `127.0.0.1:8799`. Used by the desktop app
/// in-process and by `fathombot-harness` for headless use.
pub async fn serve(store: fathom_core::Store) -> std::io::Result<()> {
    let state = AppState::new(store);
    serve_state(state).await
}

/// Serve an already-constructed shared state — the desktop app calls this so
/// the UI and HTTP API share the same store/bus/sessions.
pub async fn serve_state(state: std::sync::Arc<AppState>) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", api::PORT)).await?;
    tracing::info!("fathom harness listening on http://127.0.0.1:{}", api::PORT);
    axum::serve(listener, api::router(state)).await
}
