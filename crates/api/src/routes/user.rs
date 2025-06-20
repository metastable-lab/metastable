use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use voda_runtime::{user::{UserReferral, UserUrl}, RuntimeClient, User};

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
        .route("/user/update_profile",
            post(update_profile)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/user/claim/free",
            post(claim_free)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/user/referral/buy",
            post(buy_referral)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/user/url/create",
            post(create_url)
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
    let mut tx = state.roleplay_client.get_db().begin().await?;

    // 1. check if the user already exists
    let user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", payload.user_id.clone())?,
        &mut *tx
    ).await?;

    match user {
        Some(mut user) => {
            let _ = user.try_claim_free_balance(100); // whatever, we don't care about the error
            user.update(&mut *tx).await?;
            tx.commit().await?;
            return Ok(AppSuccess::new(StatusCode::OK, "User already exists", json!(())));
        }
        None => {
            return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/user/try_login] User not found")));
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
        QueryCriteria::new().add_valued_filter("user_id", "=", payload.user_id.clone())?,
        &mut *tx
    ).await?;

    if user.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/user/register] User already exists")));
    }

    let mut referral_code = UserReferral::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("code", "=", payload.referral_code.clone())?,
        &mut *tx
    ).await?
        .ok_or(anyhow::anyhow!("[/user/register] Referral code not found"))?;

    if referral_code.used_by.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/user/register] Referral code already used")));
    }

    let mut user = User::default();
    user.user_id = payload.user_id.clone();
    user.user_aka = CryptoHash::random().to_hex_string();
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
pub struct UpdateProfileRequest {
    pub user_aka: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
}
async fn update_profile(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.roleplay_client, &user_id_str, 0).await?
        .expect("[update_profile] User not found");

    let mut tx = state.roleplay_client.get_db().begin().await?;
    user.user_aka = payload.user_aka.clone().unwrap_or(user.user_aka.clone());
    if payload.first_name.is_some() { user.first_name = payload.first_name.clone(); }
    if payload.last_name.is_some() { user.last_name = payload.last_name.clone(); }
    if payload.email.is_some() { user.email = payload.email.clone(); }
    if payload.phone.is_some() { user.phone = payload.phone.clone(); }
    if payload.avatar.is_some() { user.avatar = payload.avatar.clone(); }
    if payload.bio.is_some() { user.bio = payload.bio.clone(); }

    let _ = user.try_claim_free_balance(100); // whatever, we don't care about the error
    user.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Profile updated successfully", json!(())))
}

async fn claim_free(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.roleplay_client, &user_id_str, 0).await?
        .expect("[claim_free] User not found");

    // TODO: technicaly - we should not use roleplay_client but a user db directly
    let mut tx = state.roleplay_client.get_db().begin().await?;
    user.try_claim_free_balance(100)?;
    user.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Points claimed successfully", json!(())))
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
    let mut user = ensure_account(&state.roleplay_client, &user_id_str, 0).await?
        .expect("[buy_referral] User not found");

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
    let user = ensure_account(&state.roleplay_client, &user_id_str, 0).await?
        .expect("[create_url] User not found");

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let url = UserUrl::new(user.id, payload.path, payload.url_type);
    url.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "URL created successfully", json!(())))
}
