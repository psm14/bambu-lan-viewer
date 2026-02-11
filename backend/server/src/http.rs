use crate::auth::{AuthContext, AuthManager};
use crate::commands::{CommandPayload, CommandRequest};
use crate::config::AppConfig;
use crate::db::{self, PrinterCreateRequest, PrinterUpdateRequest};
use crate::printers::PrinterRuntime;
use crate::state::PrinterState;
use async_stream::stream;
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    Path, State,
};
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: SqlitePool,
    pub printers: Arc<RwLock<HashMap<i64, Arc<PrinterRuntime>>>>,
    pub auth: AuthManager,
}

pub fn router(state: Arc<AppState>) -> Router {
    let protected = Router::new()
        .route("/api/session", get(get_session))
        .route("/api/printers", get(list_printers).post(create_printer))
        .route(
            "/api/printers/:id",
            get(get_printer).put(update_printer).delete(delete_printer),
        )
        .route("/api/printers/:id/status", get(get_status))
        .route("/api/printers/:id/status/stream", get(get_status_stream))
        .route("/api/printers/:id/command", post(post_command))
        .route("/api/printers/:id/video/cmaf", get(get_cmaf_stream_ws))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(protected)
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

async fn auth_middleware<B>(
    State(state): State<Arc<AppState>>,
    mut req: axum::http::Request<B>,
    next: middleware::Next<B>,
) -> Response {
    match state.auth.authenticate(req.headers()).await {
        Ok(context) => {
            req.extensions_mut().insert(context);
            next.run(req).await
        }
        Err(error) => error.into_response(),
    }
}

async fn get_session(Extension(auth): Extension<AuthContext>) -> impl IntoResponse {
    Json(SessionResponse {
        user_email: auth.email,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    user_email: String,
}

async fn list_printers(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match db::list_printers(&state.db).await {
        Ok(printers) => (StatusCode::OK, Json(printers)).into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to list printers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("database error")),
            )
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

async fn get_printer(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match db::get_printer(&state.db, id).await {
        Ok(Some(printer)) => (StatusCode::OK, Json(printer)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("printer not found")),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to load printer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("database error")),
            )
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
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("printer not found")),
        )
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
                let _ = tokio::fs::remove_dir_all(&runtime.cmaf_dir).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new("printer not found")),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(?error, "failed to delete printer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("database error")),
            )
                .into_response()
        }
    }
}

async fn get_status(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
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

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
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

async fn get_cmaf_stream_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let runtime = match runtime_for(&state, id).await {
        Ok(runtime) => runtime,
        Err(response) => return response.into_response(),
    };

    ws.on_upgrade(move |socket| async move {
        handle_cmaf_ws(socket, runtime).await;
    })
}

async fn handle_cmaf_ws(mut socket: WebSocket, runtime: Arc<PrinterRuntime>) {
    let mut subscription = runtime.cmaf_stream.subscribe();
    let init = match tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(init) = subscription.init_rx.borrow().clone() {
                break Some(init);
            }
            if subscription.init_rx.changed().await.is_err() {
                break None;
            }
        }
    })
    .await
    {
        Ok(Some(init)) => init,
        _ => return,
    };

    if socket
        .send(Message::Text(format!("codec:{}", init.codec)))
        .await
        .is_err()
    {
        return;
    }

    if socket
        .send(Message::Binary(init.bytes.to_vec()))
        .await
        .is_err()
    {
        return;
    }

    let backlog = runtime.cmaf_stream.backlog_snapshot();
    let mut last_seq = backlog.last().map(|fragment| fragment.seq).unwrap_or(0);
    for fragment in backlog {
        if socket
            .send(Message::Binary(fragment.bytes.to_vec()))
            .await
            .is_err()
        {
            return;
        }
    }

    loop {
        match subscription.fragment_rx.recv().await {
            Ok(fragment) => {
                if fragment.seq <= last_seq {
                    continue;
                }
                last_seq = fragment.seq;
                if socket
                    .send(Message::Binary(fragment.bytes.to_vec()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                tracing::warn!(
                    skipped,
                    "cmaf websocket lagged; replaying from in-memory backlog"
                );
                let backlog = runtime.cmaf_stream.backlog_snapshot();
                for fragment in backlog {
                    if fragment.seq <= last_seq {
                        continue;
                    }
                    last_seq = fragment.seq;
                    if socket
                        .send(Message::Binary(fragment.bytes.to_vec()))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
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
    printers.get(&id).cloned().ok_or((
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
