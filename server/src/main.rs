mod app;
mod config;

use std::error::Error;

use app::{AppState, build_pool, router};
use config::Config;
use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(config_error) => {
            error!(error = %config_error, "server configuration is invalid");
            return Err(Box::new(config_error));
        }
    };

    let pool = build_pool(&config.database_url, config.db_max_connections)?;
    let listener = TcpListener::bind(config.bind_addr).await?;

    info!(
        bind_addr = %config.bind_addr,
        db_max_connections = config.db_max_connections,
        "starting Ertip Lead Manager API"
    );

    axum::serve(listener, router(AppState { pool }))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,ertip_lead_manager_server=debug,tower_http=info")
    });

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(signal_error) = tokio::signal::ctrl_c().await {
            error!(error = %signal_error, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(signal_error) => {
                error!(error = %signal_error, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received");
}
