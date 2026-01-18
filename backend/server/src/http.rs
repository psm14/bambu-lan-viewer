use crate::commands::{CommandPayload, CommandRequest};
use crate::state::PrinterState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
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
    headers: HeaderMap,
) -> impl IntoResponse {
    if segment.contains('/') || segment.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = state.hls_dir.join(segment);
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let total_len = bytes.len();
            let range = headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| parse_range(value, total_len));
            let (status, body, content_range) = match range {
                Some((start, end)) => (
                    StatusCode::PARTIAL_CONTENT,
                    bytes[start..=end].to_vec(),
                    Some(format!("bytes {}-{}/{}", start, end, total_len)),
                ),
                None => (StatusCode::OK, bytes, None),
            };

            let body_len = body.len();
            let mut response = Response::new(Body::from(body));
            *response.status_mut() = status;
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("video/mp2t"),
            );
            headers.insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            );
            headers.insert(
                header::ACCEPT_RANGES,
                header::HeaderValue::from_static("bytes"),
            );
            if let Some(content_range) = content_range {
                if let Ok(value) = header::HeaderValue::from_str(&content_range) {
                    headers.insert(header::CONTENT_RANGE, value);
                }
            }
            if let Ok(value) = header::HeaderValue::from_str(&body_len.to_string()) {
                headers.insert(header::CONTENT_LENGTH, value);
            }
            response.into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

fn parse_range(range: &str, len: usize) -> Option<(usize, usize)> {
    let range = range.strip_prefix("bytes=")?;
    let mut parts = range.splitn(2, '-');
    let start_part = parts.next()?;
    let end_part = parts.next()?;

    if start_part.is_empty() {
        let suffix: usize = end_part.parse().ok()?;
        if suffix == 0 || len == 0 {
            return None;
        }
        let start = len.saturating_sub(suffix);
        let end = len.saturating_sub(1);
        return Some((start, end));
    }

    let start: usize = start_part.parse().ok()?;
    if start >= len {
        return None;
    }

    let end = if end_part.is_empty() {
        len.saturating_sub(1)
    } else {
        let parsed_end: usize = end_part.parse().ok()?;
        if parsed_end < start {
            return None;
        }
        parsed_end.min(len.saturating_sub(1))
    };

    Some((start, end))
}

#[derive(Serialize)]
struct CommandResponse {
    ok: bool,
    error: Option<String>,
}
