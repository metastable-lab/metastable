use anyhow::anyhow;
use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, decrypt, get_current_timestamp, CryptoHash, EnvVars};
use voda_database::MongoDbObject;
use voda_runtime::{RuntimeClient, User, UserRole};

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

pub async fn ensure_account<S: RuntimeClient>(
    state: &S, user_id: &CryptoHash,
    allow_unregistered: bool,
    admin_only: bool,
    price: u64,
) -> Result<Option<User>, AppError> {
    let user = User::select_one_by_index(&state.get_db(), &user_id).await?;
    if user.is_none() {
        if admin_only {
            return Err(AppError::new(
                StatusCode::FORBIDDEN,
                anyhow!("unauthorized"),
            ));
        } else if allow_unregistered {
            return Ok(None);
        } else {
            return Err(AppError::new(
                StatusCode::FORBIDDEN,
                anyhow!("unauthorized"),
            ));
        }
    }

    let user = user.unwrap();
    User::pay_and_update(&state.get_db(), &user.id, price).await?;

    if admin_only && user.role != UserRole::Admin {
        return Err(AppError::new(
            StatusCode::FORBIDDEN,
            anyhow!("unauthorized"),
        ));
    }

    Ok(Some(user))
}