use anyhow::anyhow;
use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, decrypt, get_current_timestamp, CryptoHash, EnvVars};
use voda_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use voda_runtime::{RuntimeClient, User, UserPoints};

use crate::response::AppError;
use crate::utils::extract_bearer_token;
use crate::env::ApiServerEnv;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedRequest {
    pub user_id: String,
    pub timestamp: i64,
    pub origin: String,
}

pub async fn authenticate(
    mut req: Request, next: Next
) -> Result<Response<Body>, AppError> {

    let env = ApiServerEnv::load();
    let user_id = extract_bearer_token(&req)
        .and_then(|token| decrypt(&token, &env.get_env_var("SECRET_SALT"))
            .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))
        )
        .and_then(|decrypted| serde_json::from_str::<AuthenticatedRequest>(&decrypted)
            .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))
        )
        .and_then(|authenticated_request| {
            if authenticated_request.timestamp < get_current_timestamp() - 60 {
                return Err(AppError::new(StatusCode::UNAUTHORIZED, anyhow!("authenticate expired")));
            }
            Ok(blake3_hash(authenticated_request.user_id.as_bytes()))
        })
        .unwrap_or(CryptoHash::default());

    req.extensions_mut().insert(user_id);
    Ok(next.run(req).await)
}

pub async fn ensure_account<S: RuntimeClient>(
    state: &S, user_id: &CryptoHash, price: i64,
) -> Result<Option<User>, AppError> {
    if *user_id == CryptoHash::default() {
        return Ok(None);
    }

    let mut tx = state.get_db().begin().await?;
    let user = User::find_one_by_criteria(
        QueryCriteria::by_id(user_id)?,
        &mut *tx
    ).await?;

    if price > 0 {
        let mut user_points = UserPoints::find_one_by_criteria(
            QueryCriteria::by_id(user_id)?,
            &mut *tx
        ).await?
            .unwrap_or_default();

        if user_points.pay(price) {
            user_points.update(&mut *tx).await?;
            tx.commit().await?;
        } else {
            tx.rollback().await?;
            return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("[/tts] Insufficient points")));
        }
    }

    Ok(user)
}