mod analytics;
mod app;
mod auth;
mod authz;
mod config;
mod crm;
mod crm_mutations;
mod dashboard;
mod db;
mod followups;
mod followups_http;
mod pipeline;

use std::error::Error;

use app::{AppState, build_pool, router};
use auth::{BootstrapStatus, bootstrap_admin};
use config::Config;
use db::run_migrations;
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_tracing();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(config_error) => {
            error!(error = %config_error, "server configuration is invalid");
            return Err(Box::<dyn Error>::from(config_error));
        }
    };

    let pool = build_pool(&config.database_url, config.db_max_connections)?;

    info!("applying PostgreSQL migrations");
    run_migrations(&pool).await?;
    info!("PostgreSQL migrations are current");

    match bootstrap_admin(&pool, config.bootstrap_admin.as_ref()).await? {
        BootstrapStatus::Created(user_id) => {
            info!(user_id = %user_id, "created initial ADMIN account from bootstrap configuration");
        }
        BootstrapStatus::ExistingUsers => {
            info!("application users already exist; bootstrap configuration was not applied");
        }
        BootstrapStatus::NotConfigured => {
            warn!("no users exist and bootstrap ADMIN is not configured; login will be unavailable until an account is provisioned");
        }
    }

    let listener = TcpListener::bind(config.bind_addr).await?;
    let state = AppState {
        pool,
        session_ttl_hours: config.session_ttl_hours,
    };
    let app = router(state.clone())
        .merge(followups_http::router(state.clone()))
        .merge(pipeline::router(state.clone()))
        .merge(analytics::router(state.clone()))
        .merge(dashboard::router(state));

    info!(
        bind_addr = %config.bind_addr,
        db_max_connections = config.db_max_connections,
        session_ttl_hours = config.session_ttl_hours,
        "starting Ertip Lead Manager API"
    );

    axum::serve(listener, app)
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
