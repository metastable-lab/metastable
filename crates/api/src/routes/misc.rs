use axum::{routing::{get, post}, Router, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use std::path::Path;
use sqlx::types::Uuid;
use aws_sdk_s3::presigning::PresigningConfig;
use metastable_common::ModuleClient;

use crate::{GlobalState, response::AppError};

pub fn misc_routes() -> Router<GlobalState> {
    Router::new()
        .route("/health",
            get(|| async { "OK" })
        )
        .route("/upload",
            post(upload)
        )
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRequest {
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadResponse {
    pub signed_url: String,
    pub key: String,
    pub image_url: String,
}

async fn upload(
    State(state): State<GlobalState>,
    Json(payload): Json<UploadRequest>,
) -> Result<Json<UploadResponse>, AppError> {
    // Validate input
    if payload.filename.is_empty() || payload.content_type.is_empty() {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!("Missing filename or contentType")
        ));
    }

    // Extract file extension
    let file_ext = Path::new(&payload.filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string();

    if file_ext.is_empty() {
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            anyhow::anyhow!("Invalid filename: no file extension found")
        ));
    }

    // Generate UUID-based key
    let key = format!("{}.{}", Uuid::new_v4(), file_ext);

    // Generate presigned URL (expires in 600 seconds)
    let presigned_req = state.r2_client.get_client()
        .put_object()
        .bucket(state.r2_client.bucket_name())
        .key(&key)
        .content_type(&payload.content_type)
        .presigned(
            PresigningConfig::expires_in(
                std::time::Duration::from_secs(600)
            ).map_err(|e| AppError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                anyhow::anyhow!("Failed to create presigning config: {}", e)
            ))?
        )
        .await
        .map_err(|e| AppError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            anyhow::anyhow!("Failed to generate signed URL: {}", e)
        ))?;

    let signed_url = presigned_req.uri().to_string();
    let image_url = state.r2_client.public_url(&key);

    Ok(Json(UploadResponse {
        signed_url,
        key,
        image_url,
    }))
}