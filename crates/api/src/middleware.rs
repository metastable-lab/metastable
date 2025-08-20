use axum::body::Body;
use axum::http::StatusCode;
use axum::{extract::Request, response::Response};
use axum::middleware::Next;

use metastable_common::ModuleClient;
use metastable_common::EnvVars;
use metastable_clients::PostgresClient;
use metastable_database::{QueryCriteria, SqlxFilterQuery};
use metastable_runtime::User;

use crate::response::AppError;
use crate::utils::extract_auth_token;
use crate::env::ApiServerEnv;

pub async fn authenticate(
    mut req: Request, next: Next
) -> Result<Response<Body>, AppError> {
    let env = ApiServerEnv::load();
    let maybe_auth_token = extract_auth_token(&req);

    let user_id = maybe_auth_token.and_then(|token| {
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

pub async fn ensure_account(
    db: &PostgresClient, user_id_str: &String
) -> Result<Option<User>, AppError> {

    if user_id_str.is_empty() || user_id_str == "anonymous" {
        return Ok(None);
    }

    let mut tx = db.get_client().begin().await?;
    let maybe_user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", user_id_str.clone()),
        &mut *tx
    ).await?;
    tx.commit().await?;

    Ok(maybe_user)
}