
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub type AppSuccess = GenericResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericResponse {
    pub status: u16,
    pub message: String,
    pub data: serde_json::Value,
}

impl GenericResponse {
    pub fn new(status: StatusCode, message: &str, data: serde_json::Value) -> Self {
        Self {
            status: status.as_u16(),
            message: message.to_string(),
            data,
        }
    }
}

impl IntoResponse for GenericResponse {
    fn into_response(self) -> Response {
        Json::from(self).into_response()
    }
}

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
pub struct AppError(pub StatusCode, pub anyhow::Error);
impl AppError {
    pub fn new(status: StatusCode, err: anyhow::Error) -> Self {
        Self(status, err)
    }
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("CODE: {}, MESSAGE: {}", self.0.as_u16(), self.1);
        GenericResponse::new(self.0, &self.1.to_string(), json!({})).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(StatusCode::BAD_REQUEST, err.into())
    }
}
