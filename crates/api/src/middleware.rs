use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use voda_common::EnvVars;
use voda_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use voda_runtime::{RuntimeClient, User};

use crate::response::AppError;
use crate::utils::extract_bearer_token;
use crate::env::ApiServerEnv;

pub async fn authenticate(
    mut req: Request, next: Next
) -> Result<Response<Body>, AppError> {

    let env = ApiServerEnv::load();
    let maybe_bearer_token = extract_bearer_token(&req);

    let user_id = maybe_bearer_token.and_then(|token| {
        match User::verify_auth_token(&token, &env.get_env_var("SECRET_SALT")) {
            Ok(uid) => {
                Ok(uid)
            }
            Err(e) => {
                Err(AppError::new(StatusCode::UNAUTHORIZED, e))
            }
        }
    }).unwrap_or_default();

    req.extensions_mut().insert(user_id.clone());

    let response = next.run(req).await;
    Ok(response)
}

pub async fn ensure_account<S: RuntimeClient>(
    state: &S, user_id_str: &String, price: i64,
) -> Result<Option<User>, AppError> {

    if user_id_str.is_empty() {
        return Ok(None);
    }

    let mut tx = state.get_db().begin().await?;
    match User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", user_id_str.clone())?,
        &mut *tx
    ).await? {
        Some(mut user) => {
            if price > 0 {
                let _ = user.try_claim_free_balance(100);
                let paid = user.pay(price);
                if !paid {
                    return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow::anyhow!("Insufficient balance")));
                }
                user.clone().update(&mut *tx).await?;
                tx.commit().await?;
            }
            Ok(Some(user))
        }
        None => {
            tx.rollback().await?;
            Ok(None)
        },
    }
}