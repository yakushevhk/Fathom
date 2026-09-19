//! `fathombot-harness` — standalone harness server for headless/remote use.
//! The desktop app embeds the same server in-process.

use fathom_core::{default_data_dir, Store};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let data_dir = std::env::var("FATHOM_BOT_DATA")
        .map(Into::into)
        .unwrap_or_else(|_| default_data_dir());
    let store = Store::open(data_dir)?;
    fathom_harness::serve(store).await?;
    Ok(())
}
