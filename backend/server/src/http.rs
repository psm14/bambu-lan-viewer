use crate::commands::{CommandPayload, CommandRequest};
use crate::config::AppConfig;
use crate::db::{self, PrinterCreateRequest, PrinterUpdateRequest};
use crate::printers::PrinterRuntime;
use crate::state::PrinterState;
use async_stream::stream;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use tokio::sync::RwLock;
use tokio::time::sleep;
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
        .route("/hls/:id/:segment", get(get_segment))
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
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

async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if let Err(error) = sqlx::query("SELECT 1").execute(&state.db).await {
        tracing::error!(?error, "readyz database check failed");
        return (StatusCode::SERVICE_UNAVAILABLE, "db unavailable").into_response();
    }

    (StatusCode::OK, "ready").into_response()
}

async fn get_playlist(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<LlReloadQuery>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };
    let path = runtime.hls_dir.join("stream.m3u8");
    let should_block = query.msn.is_some();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(_) => return StatusCode::NOT_FOUND.into_response(),
        };

        if should_block {
            if let (Some(msn), Some(playlist)) = (query.msn, std::str::from_utf8(&bytes).ok()) {
                if ll_request_ready(playlist, msn, query.part) || Instant::now() >= deadline {
                    return playlist_response(bytes);
                }
                sleep(Duration::from_millis(200)).await;
                continue;
            }
        }

        return playlist_response(bytes);
    }
}

#[derive(Deserialize, Default)]
struct LlReloadQuery {
    #[serde(rename = "_HLS_msn")]
    msn: Option<u64>,
    #[serde(rename = "_HLS_part")]
    part: Option<u32>,
    #[serde(rename = "_HLS_skip")]
    _skip: Option<String>,
}

fn playlist_response(bytes: Vec<u8>) -> Response {
    (
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
        .into_response()
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
    let path = runtime.hls_dir.join(&segment);
    match tokio::fs::read(&path).await {
        Ok(mut bytes) => {
            let mut total_len = bytes.len();
            let range_header = headers
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_string());
            let mut range = range_header
                .as_deref()
                .and_then(|value| parse_range(value, total_len));

            if range.is_none() && range_header.is_some() && segment.ends_with(".m4s") {
                if let Some(start) = parse_range_start(range_header.as_deref().unwrap()) {
                    if start >= total_len {
                        let deadline = Instant::now() + Duration::from_secs(5);
                        loop {
                            if Instant::now() >= deadline {
                                break;
                            }
                            sleep(Duration::from_millis(150)).await;
                            if let Ok(updated) = tokio::fs::read(&path).await {
                                bytes = updated;
                                total_len = bytes.len();
                                if start < total_len {
                                    range = range_header
                                        .as_deref()
                                        .and_then(|value| parse_range(value, total_len));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            let (status, body, content_range) = match range {
                Some((start, end)) => (
                    StatusCode::PARTIAL_CONTENT,
                    bytes[start..=end].to_vec(),
                    Some(format!("bytes {}-{}/{}", start, end, total_len)),
                ),
                None => {
                    if range_header.is_some() {
                        let mut response = Response::new(Body::empty());
                        *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
                        let headers = response.headers_mut();
                        headers.insert(
                            header::CACHE_CONTROL,
                            header::HeaderValue::from_static("no-store"),
                        );
                        headers.insert(
                            header::ACCEPT_RANGES,
                            header::HeaderValue::from_static("bytes"),
                        );
                        if let Ok(value) =
                            header::HeaderValue::from_str(&format!("bytes */{}", total_len))
                        {
                            headers.insert(header::CONTENT_RANGE, value);
                        }
                        return response.into_response();
                    }
                    (StatusCode::OK, bytes, None)
                }
            };

            let body_len = body.len();
            let mut response = Response::new(Body::from(body));
            *response.status_mut() = status;
            let headers = response.headers_mut();
            let content_type = if segment.ends_with(".m4s") || segment.ends_with(".mp4") {
                "video/mp4"
            } else {
                "application/octet-stream"
            };
            if let Ok(value) = header::HeaderValue::from_str(content_type) {
                headers.insert(header::CONTENT_TYPE, value);
            }
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

fn parse_range_start(range: &str) -> Option<usize> {
    let range = range.strip_prefix("bytes=")?;
    let mut parts = range.splitn(2, '-');
    let start_part = parts.next()?;
    if start_part.is_empty() {
        return None;
    }
    start_part.parse().ok()
}

fn ll_request_ready(playlist: &str, msn: u64, part: Option<u32>) -> bool {
    let index = match parse_ll_playlist(playlist) {
        Some(index) => index,
        None => return true,
    };

    if msn < index.media_sequence {
        return true;
    }

    match part {
        None => {
            if let Some(last_complete) = index.last_complete_seq {
                return msn <= last_complete;
            }
            false
        }
        Some(part_index) => {
            if msn == index.in_progress_seq {
                return part_index < index.in_progress_parts;
            }
            if let Some(count) = index.parts_by_seq.get(&msn) {
                return part_index < *count;
            }
            false
        }
    }
}

struct LlPlaylistIndex {
    media_sequence: u64,
    parts_by_seq: HashMap<u64, u32>,
    in_progress_seq: u64,
    in_progress_parts: u32,
    last_complete_seq: Option<u64>,
}

fn parse_ll_playlist(playlist: &str) -> Option<LlPlaylistIndex> {
    let mut media_sequence: Option<u64> = None;
    let mut current_seq: u64 = 0;
    let mut part_count: u32 = 0;
    let mut parts_by_seq: HashMap<u64, u32> = HashMap::new();
    let mut last_complete_seq: Option<u64> = None;

    for line in playlist.lines().map(|line| line.trim()) {
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            if let Ok(parsed) = value.trim().parse::<u64>() {
                media_sequence = Some(parsed);
                current_seq = parsed;
                part_count = 0;
            }
            continue;
        }
        if line.starts_with("#EXT-X-PART:") {
            part_count = part_count.saturating_add(1);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if media_sequence.is_some() {
            parts_by_seq.insert(current_seq, part_count);
            last_complete_seq = Some(current_seq);
            current_seq = current_seq.saturating_add(1);
            part_count = 0;
        }
    }

    let media_sequence = media_sequence?;
    Some(LlPlaylistIndex {
        media_sequence,
        parts_by_seq,
        in_progress_seq: current_seq,
        in_progress_parts: part_count,
        last_complete_seq,
    })
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
