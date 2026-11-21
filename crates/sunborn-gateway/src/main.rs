use std::net::SocketAddr;
use sunborn_core::{init_tracing, AppConfig};
use sunborn_gateway::{create_router, AppState};
use tokio::net::TcpListener;

/// Exit code used for an operator configuration error (BSD `EX_CONFIG`).
/// A supervisor can distinguish "bad configuration, do not restart blindly"
/// from a transient runtime failure.
const EXIT_CONFIG: i32 = 78;

fn resolved_addr(config: &AppConfig) -> SocketAddr {
    format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], config.server.port)))
}

fn print_version() {
    println!(
        "sunborn-gateway {} ({})",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    println!(
        "git:      {}",
        option_env!("SUNBORN_GIT_SHA").unwrap_or("unknown")
    );
    println!(
        "rustc:    {}",
        option_env!("SUNBORN_RUSTC_VERSION").unwrap_or("unknown")
    );
}

fn print_usage() {
    println!(
        "sunborn-gateway {}\n\
         \n\
         USAGE:\n\
         \x20 sunborn-gateway                  Start the gateway (default)\n\
         \x20 sunborn-gateway serve            Start the gateway explicitly\n\
         \x20 sunborn-gateway config-check     Validate configuration, print the effective (redacted) view\n\
         \x20 sunborn-gateway doctor           Run local pre-flight checks before starting\n\
         \x20 sunborn-gateway healthcheck      Probe a running gateway's /health endpoint\n\
         \x20 sunborn-gateway version          Print build identity\n\
         \x20 sunborn-gateway help             Print this message\n\
         \n\
         Environment: SUNBORN_ENV, PORT, HOST, SUNBORN_* (see .env.example).\n\
         Exit codes: 0 success, 78 invalid configuration, 1 other failure.",
        env!("CARGO_PKG_VERSION")
    );
}

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

/// `config-check`: load, validate, and print the effective configuration with
/// secrets redacted. Exits 78 on any problem so scripts can gate on it.
fn run_config_check() -> i32 {
    let config = match AppConfig::try_load_from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Configuration could not be loaded: {err}");
            return EXIT_CONFIG;
        }
    };

    if let Err(err) = config.validate() {
        eprintln!("{err}");
        return EXIT_CONFIG;
    }

    println!("Configuration is valid.");
    match serde_json::to_string_pretty(&config.redacted_summary()) {
        Ok(pretty) => println!("{pretty}"),
        Err(err) => {
            eprintln!("Configuration loaded but could not be rendered: {err}");
            return EXIT_CONFIG;
        }
    }
    0
}

/// Verifies a directory (or the file's parent) accepts a write, without
/// leaving anything behind. Returns the reason on failure.
fn check_writable(label: &str, path: &std::path::Path) -> Result<(), String> {
    let parent = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    if !parent.exists() {
        return Err(format!(
            "{label}: directory '{}' does not exist",
            parent.display()
        ));
    }
    let probe = parent.join(format!(".sunborn-doctor-{}", std::process::id()));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(err) => Err(format!(
            "{label}: '{}' is not writable ({err})",
            parent.display()
        )),
    }
}

/// `doctor`: everything an operator needs to know before first start, in one
/// pass, with a non-zero exit if any blocking check fails.
async fn run_doctor() -> i32 {
    let mut failures = 0usize;
    let mut warnings = 0usize;

    println!("Sunborn doctor\n──────────────");

    // 1. Configuration.
    let config = match AppConfig::try_load_from_env() {
        Ok(config) => {
            println!(
                "[PASS] configuration loaded ({} mode)",
                if sunborn_core::is_production_mode() {
                    "production"
                } else {
                    "development"
                }
            );
            config
        }
        Err(err) => {
            println!("[FAIL] configuration could not be loaded: {err}");
            return EXIT_CONFIG;
        }
    };
    match config.validate() {
        Ok(()) => println!("[PASS] configuration validated"),
        Err(err) => {
            println!("[FAIL] {err}");
            failures += 1;
        }
    }

    // 2. Durable storage paths, when configured.
    for (label, key) in [
        ("execution store", "SUNBORN_EXECUTION_STORE_PATH"),
        ("learning store", "SUNBORN_LEARNING_WAL_PATH"),
    ] {
        if let Ok(value) = std::env::var(key) {
            if value.trim().is_empty() {
                continue;
            }
            match check_writable(label, std::path::Path::new(&value)) {
                Ok(()) => println!("[PASS] {label} path is writable: {value}"),
                Err(reason) => {
                    println!("[FAIL] {reason}");
                    failures += 1;
                }
            }
        } else if sunborn_core::is_production_mode() {
            println!(
                "[WARN] {key} is not set; production startup will reject an ephemeral {label}"
            );
            warnings += 1;
        }
    }

    // 3. Network bind availability.
    let addr = resolved_addr(&config);
    match TcpListener::bind(addr).await {
        Ok(listener) => {
            drop(listener);
            println!("[PASS] {addr} is available for binding");
        }
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            // Either a healthy Sunborn instance or an unrelated process.
            if run_healthcheck().await.is_ok() {
                println!("[PASS] a healthy Sunborn gateway is already listening on {addr}");
            } else {
                println!("[FAIL] {addr} is in use by another process");
                failures += 1;
            }
        }
        Err(err) => {
            println!("[FAIL] cannot bind {addr}: {err}");
            failures += 1;
        }
    }

    // 4. Key material presence (never the value).
    if config.cache.master_kek.is_some() {
        println!("[PASS] encryption key material is configured");
    } else if sunborn_core::is_production_mode() {
        println!("[FAIL] SUNBORN_MASTER_KEK is required in production");
        failures += 1;
    } else {
        println!("[WARN] SUNBORN_MASTER_KEK is not set; local development keys will be used");
        warnings += 1;
    }

    println!("──────────────");
    if failures == 0 {
        println!("Doctor result: OK ({warnings} warning(s)). Ready to start.");
        println!("Next: sunborn-gateway   (or your service manager)");
        0
    } else {
        println!("Doctor result: {failures} blocking problem(s), {warnings} warning(s).");
        1
    }
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

async fn serve() -> anyhow::Result<()> {
    init_tracing();
    sunborn_gateway::handlers::init_metrics();

    // Fail with a diagnosable message and a distinct exit code rather than a
    // panic (release builds use `panic = "abort"`, which loses the cause).
    let config = match AppConfig::try_load_from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("FATAL: Sunborn Gateway configuration load failed: {err}");
            std::process::exit(EXIT_CONFIG);
        }
    };
    if let Err(err) = config.validate() {
        eprintln!("FATAL: {err}");
        std::process::exit(EXIT_CONFIG);
    }

    let addr = resolved_addr(&config);

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

    // Connect info supplies the peer address used by the pre-authentication rate
    // limiter. Without it that middleware fails open, so this is required for the
    // brute-force brake to be active at all.
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let command = std::env::args().nth(1);

    match command.as_deref() {
        None | Some("serve") | Some("start") => serve().await,
        Some("version") | Some("--version") | Some("-V") => {
            print_version();
            Ok(())
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_usage();
            Ok(())
        }
        Some("config-check") | Some("config") | Some("--check-config") => {
            std::process::exit(run_config_check());
        }
        Some("doctor") | Some("--doctor") => {
            std::process::exit(run_doctor().await);
        }
        Some("healthcheck") | Some("--healthcheck") => match run_healthcheck().await {
            Ok(()) => {
                println!("healthy");
                Ok(())
            }
            Err(e) => {
                eprintln!("Gateway healthcheck probe failed: {e}");
                std::process::exit(1);
            }
        },
        Some(other) => {
            eprintln!("Unknown command: {other}\n");
            print_usage();
            std::process::exit(2);
        }
    }
}
