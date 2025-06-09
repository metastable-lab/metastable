use anyhow::anyhow;
use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use serde::{Deserialize, Serialize};
use voda_common::{decrypt, get_current_timestamp, EnvVars};
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
    let user_id_string = extract_bearer_token(&req)
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
            Ok(authenticated_request.user_id)
        })
        .unwrap_or_default();

    req.extensions_mut().insert(user_id_string);
    Ok(next.run(req).await)
}

pub async fn ensure_account<S: RuntimeClient>(
    state: &S, user_id_str: &String, price: i64,
) -> Result<Option<User>, AppError> {
    if user_id_str.is_empty() {
        return Ok(None);
    }

    let mut tx = state.get_db().begin().await?;
    let user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", user_id_str.clone())?,
        &mut *tx
    ).await?;

    if user.is_none() {
        tx.rollback().await?;
        return Ok(None)
    }

    let user = user.unwrap();

    if price > 0 {
        let mut user_points = UserPoints::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", user.id)?,
            &mut *tx
        ).await?
            .unwrap_or_default();

        if user_points.pay(price) {
            user_points.update(&mut *tx).await?;
        } else {
            tx.rollback().await?;
            return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("[ensure_account] Insufficient points")));
        }
    }

    tx.commit().await?;

    Ok(Some(user))
}