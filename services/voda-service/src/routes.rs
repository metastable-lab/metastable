use anyhow::anyhow;
use axum::{extract::{Path, State}, http::StatusCode, middleware, routing::{get, post}, Extension, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use voda_common::CryptoHash;
use voda_database::{doc, MongoDbObject};
use voda_runtime::{RuntimeClient, User};
use voda_service_api::{authenticate, AppError, AppSuccess};

use crate::db_ext::{GitcoinGrant, Url};

pub fn voda_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/url", 
            post(create_url::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/url/{url_id}", 
            get(get_url::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/gitcoin/all", 
            get(get_all_gitcoin_grants::<S>)
        )

        .route("/gitcoin/{grant_id}", 
            get(get_gitcoin_grant::<S>)
        )
}

async fn get_all_gitcoin_grants<S: RuntimeClient>(
    State(state): State<S>
) -> Result<AppSuccess, AppError> {
    let gitcoin_grants = GitcoinGrant::select_many_simple(&state.get_db(), doc!{}).await?;

    // place name = "Takara Lend" to the top of the list
    let mut gitcoin_grants = gitcoin_grants;
    if let Some(pos) = gitcoin_grants.iter().position(|g| g.name == "Takara Lend") {
        let grant = gitcoin_grants.remove(pos);
        gitcoin_grants.insert(0, grant);
    }

    Ok(AppSuccess::new(StatusCode::OK, "Gitcoin grants fetched successfully", json!(gitcoin_grants)))
}

async fn get_gitcoin_grant<S: RuntimeClient>(
    State(state): State<S>,
    Path(grant_id): Path<CryptoHash>
) -> Result<AppSuccess, AppError> {
    let gitcoin_grant = GitcoinGrant::select_one_by_index(&state.get_db(), &grant_id).await?;
    Ok(AppSuccess::new(StatusCode::OK, "Gitcoin grant fetched successfully", json!(gitcoin_grant)))
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateUrlRequest {
    path: String,
    url_type: String,
}

async fn create_url<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Json(request): Json<CreateUrlRequest>
) -> Result<AppSuccess, AppError> {
    let url = Url::new(user_id, request.path, request.url_type);
    let url_id = url.id.clone();
    url.save(&state.get_db()).await?;
    Ok(AppSuccess::new(StatusCode::OK, "URL created successfully", json!({
        "url_id": url_id.to_string()
    })))
}

async fn get_url<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(url_id): Path<CryptoHash>
) -> Result<AppSuccess, AppError> {
    let mut url = Url::select_one_by_index(&state.get_db(), &url_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("URL not found")))?;
    let referral_success = url.url_type == "referral" && !url.used_by.contains(&user_id);

    url.used_by.insert(user_id.clone());    
    url.update(&state.get_db()).await?;

    if referral_success {
        User::record_misc_balance(&state.get_db(), &user_id, 10).await?;
        User::record_misc_balance(&state.get_db(), &url.created_by, 10).await?;
    }

    Ok(AppSuccess::new(StatusCode::OK, "URL fetched successfully", json!({
        "url": url,
        "referral_success": referral_success
    })))
}