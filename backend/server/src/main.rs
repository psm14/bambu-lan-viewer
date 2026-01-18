mod commands;
mod config;
mod http;
mod mqtt;
mod state;

use crate::config::Config;
use crate::http::AppState;
use crate::state::PrinterState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env()?;
    let printer_state = Arc::new(RwLock::new(PrinterState::default()));
    let (command_tx, command_rx) = mpsc::channel(32);

    let mqtt_state = Arc::clone(&printer_state);
    let mqtt_config = config.clone();
    tokio::spawn(async move {
        mqtt::run(mqtt_config, mqtt_state, command_rx).await;
    });

    let app_state = Arc::new(AppState {
        printer_state,
        command_tx,
    });
    let app = http::router(app_state);

    let addr: SocketAddr = config.http_bind.parse()?;
    info!(%addr, "http server listening");

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
