use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub email: String,
}

#[derive(Clone)]
pub struct AuthManager;

impl AuthManager {
    pub fn new() -> Self {
        tracing::debug!("authentication disabled (no auth required)");
        Self
    }

    pub async fn authenticate(&self, _headers: &HeaderMap) -> Result<AuthContext, AuthError> {
        Ok(AuthContext {
            email: "anonymous".to_string(),
        })
    }
}

#[derive(Debug)]
pub struct AuthError {
    status: StatusCode,
    message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}
