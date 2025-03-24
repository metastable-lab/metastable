use anyhow::anyhow;
use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, decrypt, get_current_timestamp, EnvVars};

use crate::response::AppError;
use crate::utils::extract_bearer_token;
use crate::env::ApiServerEnv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedRequest {
    pub user_id: String,
    pub timestamp: u64,
    pub origin: String,
}

pub async fn authenticate(
    mut req: Request, next: Next
) -> Result<Response<Body>, AppError> {
    let token = extract_bearer_token(&req)?;
    let env = ApiServerEnv::load();
    let decrypted = decrypt(&token, &env.get_env_var("SECRET_SALT"))?;

    let authenticated_request: AuthenticatedRequest = serde_json::from_str(&decrypted)?;
    if authenticated_request.timestamp < get_current_timestamp() - 60 {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            anyhow!("unauthorized"),
        ));
    }

    let user_id = blake3_hash(authenticated_request.user_id.as_bytes());
    req.extensions_mut().insert(user_id);
    Ok(next.run(req).await)
}

pub async fn admin_only(
    req: Request, next: Next
) -> Result<Response<Body>, AppError> {
    let token = extract_bearer_token(&req)?;
    let env = ApiServerEnv::load();
    let hash = blake3_hash(env.get_env_var("SECRET_SALT").as_bytes());
    let decrypted = decrypt(&token, &env.get_env_var("SECRET_SALT"))?;

    if decrypted != hash.to_string() {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            anyhow!("unauthorized"),
        ));
    }

    Ok(next.run(req).await)
}
