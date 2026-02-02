use crate::commands::{CommandPayload, CommandRequest};
use crate::config::AppConfig;
use crate::db::{self, PrinterCreateRequest, PrinterUpdateRequest};
use crate::printers::PrinterRuntime;
use crate::state::PrinterState;
use async_stream::stream;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SqlitePool,
    pub printers: Arc<RwLock<HashMap<i64, Arc<PrinterRuntime>>>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/printers", get(list_printers).post(create_printer))
        .route(
            "/api/printers/:id",
            get(get_printer).put(update_printer).delete(delete_printer),
        )
        .route("/api/printers/:id/status", get(get_status))
        .route("/api/printers/:id/status/stream", get(get_status_stream))
        .route("/api/printers/:id/command", post(post_command))
        .route("/hls/:id/stream.m3u8", get(get_playlist))
        .route("/hls/:id/stream_ll.m3u8", get(get_ll_playlist))
        .route("/hls/:id/:segment", get(get_segment))
        .route("/healthz", get(healthz))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                ])
                .allow_headers(Any),
        )
}

async fn list_printers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::list_printers(&state.db).await {
        Ok(printers) => (StatusCode::OK, Json(printers)).into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to list printers");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse::new("database error")))
                .into_response()
        }
    }
}

async fn create_printer(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PrinterCreateRequest>,
) -> impl IntoResponse {
    match db::create_printer(&state.db, payload).await {
        Ok(printer) => {
            let runtime = PrinterRuntime::spawn(printer.clone(), &state.config);
            let mut printers = state.printers.write().await;
            printers.insert(printer.id, runtime);
            (StatusCode::CREATED, Json(printer)).into_response()
        }
        Err(error) => db_error_response(error),
    }
}

async fn get_printer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::get_printer(&state.db, id).await {
        Ok(Some(printer)) => (StatusCode::OK, Json(printer)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse::new("printer not found")))
            .into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to load printer");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse::new("database error")))
                .into_response()
        }
    }
}

async fn update_printer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<PrinterUpdateRequest>,
) -> impl IntoResponse {
    match db::update_printer(&state.db, id, payload).await {
        Ok(Some(printer)) => {
            let runtime = PrinterRuntime::spawn(printer.clone(), &state.config);
            let mut printers = state.printers.write().await;
            if let Some(existing) = printers.remove(&id) {
                existing.shutdown();
            }
            printers.insert(id, runtime);
            (StatusCode::OK, Json(printer)).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(ErrorResponse::new("printer not found")))
            .into_response(),
        Err(error) => db_error_response(error),
    }
}

async fn delete_printer(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match db::delete_printer(&state.db, id).await {
        Ok(true) => {
            let runtime = {
                let mut printers = state.printers.write().await;
                printers.remove(&id)
            };
            if let Some(runtime) = runtime {
                runtime.shutdown();
                let _ = tokio::fs::remove_dir_all(&runtime.hls_dir).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, Json(ErrorResponse::new("printer not found")))
            .into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to delete printer");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse::new("database error")))
                .into_response()
        }
    }
}

async fn get_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match runtime_for(&state, id).await {
        Ok(runtime) => {
            let snapshot = runtime.state.read().await.clone();
            Json(snapshot).into_response()
        }
        Err(response) => response.into_response(),
    }
}

async fn get_status_stream(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    let mut rx = runtime.status_tx.subscribe();
    let initial = rx.borrow_and_update().clone();

    let stream = stream! {
        yield Ok::<Event, Infallible>(
            Event::default()
                .event("status")
                .data(serialize_status(&initial)),
        );

        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let snapshot = rx.borrow().clone();
            yield Ok::<Event, Infallible>(
                Event::default()
                    .event("status")
                    .data(serialize_status(&snapshot)),
            );
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
    .into_response()
}

async fn post_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(payload): Json<CommandPayload>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    let connected = runtime.state.read().await.connected;
    if !connected {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandResponse {
                ok: false,
                error: Some("printer not connected".to_string()),
            }),
        )
            .into_response();
    }

    let command = CommandRequest::from(payload);
    if runtime.command_tx.send(command).await.is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(CommandResponse {
                ok: false,
                error: Some("command channel unavailable".to_string()),
            }),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(CommandResponse {
            ok: true,
            error: None,
        }),
    )
        .into_response()
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    let path = runtime.hls_dir.join("stream.m3u8");
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

async fn get_ll_playlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    let path = runtime.hls_dir.join("stream_ll.m3u8");
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
    Path((id, segment)): Path<(i64, String)>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    if segment.contains('/') || segment.contains("..") {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let path = runtime.hls_dir.join(segment);
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

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl ErrorResponse {
    fn new(message: &str) -> Self {
        Self {
            error: message.to_string(),
        }
    }
}

fn serialize_status(state: &PrinterState) -> String {
    serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string())
}

async fn runtime_for(
    state: &Arc<AppState>,
    id: i64,
) -> Result<Arc<PrinterRuntime>, (StatusCode, Json<ErrorResponse>)> {
    let printers = state.printers.read().await;
    printers
        .get(&id)
        .cloned()
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("printer not found")),
        ))
}

fn db_error_response(error: anyhow::Error) -> Response {
    let message = error.to_string();
    let status = if message.contains("UNIQUE constraint failed") {
        StatusCode::CONFLICT
    } else if message.contains("required") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(ErrorResponse::new(&message))).into_response()
}
