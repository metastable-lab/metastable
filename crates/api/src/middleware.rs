use anyhow::anyhow;
use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use voda_common::{blake3_hash, decrypt};

use crate::response::AppError;
use crate::utils::extract_bearer_token;
use crate::env::EnvVars;

pub async fn authenticate(
    mut req: Request, next: Next
) -> Result<Response<Body>, AppError> {
    let token = extract_bearer_token(&req)?;
    let env = EnvVars::load();
    let decrypted = decrypt(&token, &env.secret_salt)?;
    let user_id = blake3_hash(decrypted.as_bytes());
    req.extensions_mut().insert(user_id);
    Ok(next.run(req).await)
}

pub async fn admin_only(
    req: Request, next: Next
) -> Result<Response<Body>, AppError> {
    let token = extract_bearer_token(&req)?;
    let env = EnvVars::load();
    let hash = blake3_hash(env.secret_salt.as_bytes());
    let decrypted = decrypt(&token, &env.secret_salt)?;

    if decrypted != hash.to_string() {
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            anyhow!("unauthorized"),
        ));
    }

    Ok(next.run(req).await)
}
