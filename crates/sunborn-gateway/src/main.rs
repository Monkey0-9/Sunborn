use std::net::SocketAddr;
use sunborn_core::{init_tracing, AppConfig};
use sunborn_gateway::{create_router, AppState};
use tokio::net::TcpListener;

async fn run_healthcheck() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("SUNBORN_SERVER__PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let host = std::env::var("HOST")
        .or_else(|_| std::env::var("SUNBORN_SERVER__HOST"))
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let check_host = if host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        &host
    };
    let addr: SocketAddr = format!("{}:{}", check_host, port).parse()?;

    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut buf = [0u8; 512];
        let n = stream.read(&mut buf).await?;
        let response = String::from_utf8_lossy(&buf[..n]);
        if response.contains("200 OK") {
            Ok(())
        } else {
            Err(format!("Healthcheck failed: received non-200 response: {response}").into())
        }
    })
    .await?
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown...");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown...");
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "healthcheck" || args[1] == "--healthcheck") {
        match run_healthcheck().await {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Gateway healthcheck probe failed: {e}");
                std::process::exit(1);
            }
        }
    }

    init_tracing();

    let config = AppConfig::load_from_env();
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));

    tracing::info!(
        address = %addr,
        "Starting Sunborn Gateway (Phase 01 Foundation)"
    );

    let state = AppState::new(config);
    state.verify_production_readiness().await?;
    let app = create_router(state.clone());

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("Gateway listening on http://{}", addr);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(());
    });

    let shutdown_waiter = async {
        let _ = shutdown_rx.await;
    };

    let drain_timeout_secs = std::env::var("SUNBORN_SHUTDOWN_DRAIN_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(15);

    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let server = axum::serve(listener, app).with_graceful_shutdown(async move {
        shutdown_waiter.await;
        tracing::info!(
            drain_timeout_secs = drain_timeout_secs,
            "Entering graceful drain period (deadline: {}s). Stopping new connections...",
            drain_timeout_secs
        );
        tokio::spawn(async move {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(drain_timeout_secs)) => {
                    tracing::error!(
                        timeout_secs = drain_timeout_secs,
                        "Shutdown drain deadline reached with active work remaining; terminating server process with exit code 75"
                    );
                    std::process::exit(75);
                }
                _ = done_rx => {}
            }
        });
    });

    server.await?;
    state.graceful_drain().await?;
    let _ = done_tx.send(());
    tracing::info!("Sunborn Gateway shutdown complete: all in-flight connections and background work drained cleanly.");

    Ok(())
}
