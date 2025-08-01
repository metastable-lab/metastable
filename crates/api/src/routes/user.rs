use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use metastable_runtime::{user::{UserReferral, UserUrl}, RuntimeClient, User, UserFollow};

use crate::{
    ensure_account, 
    middleware::authenticate, 
    response::{AppError, AppSuccess},
    GlobalState
};

pub fn user_routes() -> Router<GlobalState> {
    Router::new()
        .route("/user/try_login",
            post(try_login)
        )
        .route("/user/register",
            post(register)
        )

        .route("/user/referral/buy",
            post(buy_referral)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/user/url/create",
            post(create_url)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/user/follow",
            post(follow)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TryLoginRequest {
    pub user_id: String,
}

async fn try_login(
    State(state): State<GlobalState>,
    Json(payload): Json<TryLoginRequest>,
) -> Result<AppSuccess, AppError> {
    let db = state.roleplay_client.get_db();
    tracing::info!("db: {:?}", db);
    let mut tx = db.begin().await?;

    // 1. check if the user already exists
    let user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", payload.user_id.clone()),
        &mut *tx
    ).await?;

    match user {
        Some(mut user) => {
            let _ = user.try_claim_free_balance(100); // whatever, we don't care about the error
            user.update(&mut *tx).await?;
            tx.commit().await?;
            return Ok(AppSuccess::new(
                StatusCode::OK, 
                "User already exists", 
                json!({ "registration_required": false })
            ));
        }
        None => {
            return Ok(AppSuccess::new(
                StatusCode::OK, 
                "[/user/try_login] User not found", 
                json!({ "registration_required": true })
            ));
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub user_id: String,
    pub referral_code: String,
    pub provider: String,
}

async fn register(
    State(state): State<GlobalState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<AppSuccess, AppError> {
    let mut tx = state.roleplay_client.get_db().begin().await?;

    // 1. check if the user already exists
    let user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", payload.user_id.clone()),
        &mut *tx
    ).await?;

    if user.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/user/register] User already exists")));
    }

    let mut referral_code = UserReferral::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("code", "=", payload.referral_code.clone()),
        &mut *tx
    ).await?
        .ok_or(anyhow::anyhow!("[/user/register] Referral code not found"))?;

    if referral_code.used_by.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/user/register] Referral code already used")));
    }

    let mut user = User::default();
    user.user_id = payload.user_id.clone();
    user.user_aka = "nono".to_string();
    user.provider = payload.provider.clone();
    let _ = user.try_claim_free_balance(100); // infallable
    let user = user.create(&mut *tx).await?;

    referral_code.used_by = Some(user.id);
    referral_code.used_at = Some(get_current_timestamp());
    referral_code.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "User registered successfully", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BuyReferralRequest {
    pub count: Option<i64>,
}
async fn buy_referral(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<BuyReferralRequest>,
) -> Result<AppSuccess, AppError> {
    let count = payload.count.unwrap_or(1);
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let mut user = maybe_user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[buy_referral] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;

    let referrals = user.buy_referral_code(count)?;
    for referral in &referrals {
        referral.clone().create(&mut *tx).await?;
    }

    user.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Referral bought successfully", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUrlRequest {
    pub url_type: String,
    pub path: String,
}
async fn create_url(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<CreateUrlRequest>,
) -> Result<AppSuccess, AppError> {
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let user = maybe_user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_url] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let url = UserUrl::new(user.id, payload.path, payload.url_type);
    let url = url.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "URL created successfully", json!({
        "url_id": url.id,
    })))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FollowRequest {
    pub following_id: Uuid,
}

async fn follow(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<FollowRequest>,
) -> Result<AppSuccess, AppError> {
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let follower = maybe_user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[follow] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let follow = UserFollow::new(follower.id, payload.following_id);
    follow.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Followed successfully", json!(())))
}