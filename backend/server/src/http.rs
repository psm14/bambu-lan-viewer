use crate::commands::{CommandPayload, CommandRequest};
use crate::state::PrinterState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub printer_state: Arc<RwLock<PrinterState>>,
    pub command_tx: mpsc::Sender<CommandRequest>,
    pub hls_dir: PathBuf,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/command", post(post_command))
        .route("/healthz", get(healthz))
        .route("/hls/stream.m3u8", get(get_playlist))
        .route("/hls/:segment", get(get_segment))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers(Any),
        )
}

async fn get_status(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = state.printer_state.read().await.clone();
    Json(snapshot)
}

async fn post_command(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CommandPayload>,
) -> impl IntoResponse {
    let connected = state.printer_state.read().await.connected;
    if !connected {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandResponse {
                ok: false,
                error: Some("printer not connected".to_string()),
            }),
        );
    }

    let command = CommandRequest::from(payload);
    if let Err(_) = state.command_tx.send(command).await {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandResponse {
                ok: false,
                error: Some("command channel unavailable".to_string()),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(CommandResponse {
            ok: true,
            error: None,
        }),
    )
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn get_playlist(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let path = state.hls_dir.join("stream.m3u8");
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (
                    axum::http::header::CONTENT_TYPE,
                    "application/vnd.apple.mpegurl",
                ),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_segment(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(segment): axum::extract::Path<String>,
) -> impl IntoResponse {
    if segment.contains('/') || segment.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.hls_dir.join(segment);
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (axum::http::header::CONTENT_TYPE, "video/mp2t"),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Serialize)]
struct CommandResponse {
    ok: bool,
    error: Option<String>,
}
