use std::net::SocketAddr;
use sunborn_core::{init_tracing, AppConfig};
use sunborn_gateway::{create_router, AppState};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = AppConfig::load_from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));

    tracing::info!(
        address = %addr,
        "Starting Sunborn Gateway (Phase 01 Foundation)"
    );

    let state = AppState::new(config);
    let app = create_router(state);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Gateway listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
