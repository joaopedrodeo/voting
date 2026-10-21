use anyhow::Result;
use std::net::SocketAddr;
use voting::{app, store::Store};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let db_path = std::env::var("VOTING_DB_PATH").unwrap_or_else(|_| "voting.sqlite3".into());
    let address: SocketAddr = std::env::var("VOTING_LISTEN_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;
    let store = Store(db_path.into());
    store.init()?;

    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(address = %listener.local_addr()?, "servidor iniciado");
    axum::serve(listener, app(store)).await?;
    Ok(())
}
