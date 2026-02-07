mod auth;
mod commands;
mod config;
mod db;
mod http;
mod mqtt;
mod printers;
mod rtsp;
mod state;
mod tls;

use crate::auth::AuthManager;
use crate::config::AppConfig;
use crate::http::AppState;
use crate::printers::PrinterRuntime;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let _ = dotenvy::dotenv();
    let config = AppConfig::from_env()?;
    let db = db::init(&config.database_url).await?;
    let printers = db::list_printers(&db).await?;
    let mut runtime_map: HashMap<i64, Arc<PrinterRuntime>> = HashMap::new();
    for printer in printers {
        let runtime = PrinterRuntime::spawn(printer.clone(), &config);
        runtime_map.insert(printer.id, runtime);
    }
    let auth = AuthManager::new(&config)?;

    let addr: SocketAddr = config.http_bind.parse()?;
    info!(%addr, "http server listening");

    let app_state = Arc::new(AppState {
        config,
        db,
        printers: Arc::new(RwLock::new(runtime_map)),
        auth,
    });
    let app = http::router(app_state);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
