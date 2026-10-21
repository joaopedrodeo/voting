use anyhow::Result;
use std::net::SocketAddr;
use voting::{app, camara::Camara, store::Store};

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
    let sync_store = store.clone();
    tokio::spawn(async move {
        let camara = Camara::new(
            "https://dadosabertos.camara.leg.br/api/v2".into(),
            "https://dadosabertos.camara.leg.br/arquivos/votacoes/csv".into(),
        );
        let today = chrono::Utc::now().date_naive();
        if let Err(error) = camara.sync_recent(&sync_store, today).await {
            tracing::warn!(%error, "falha ao atualizar votações recentes da Câmara");
        }
    });
    axum::serve(listener, app(store)).await?;
    Ok(())
}
