use anyhow::anyhow;
use axum::extract::Path;
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
use metastable_runtime_roleplay::{Character, CharacterFeature, CharacterGender, CharacterHistory, CharacterLanguage, CharacterStatus, CharacterSub};

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
        .route("/user/update_character/{character_id}",
            post(update_character)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/user/character/sub/{character_id}",
            post(create_character_sub)
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
    let mut referer = referral_code.fetch_user_id(&mut *tx).await?
        .ok_or(anyhow!("[/user/register] Referral code not found not valid"))?;

    let mut user = User::default();
    user.user_id = payload.user_id.clone();
    user.user_aka = "nono".to_string();
    user.provider = payload.provider.clone();
    let _ = user.try_claim_free_balance(100); // infallable

    user.running_misc_balance += 20;
    let user = user.create(&mut *tx).await?;

    referral_code.used_by = Some(user.id);
    referral_code.used_at = Some(get_current_timestamp());
    referral_code.update(&mut *tx).await?;

    referer.running_misc_balance += 20;
    referer.update(&mut *tx).await?;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCharacterRequest {
    pub avatar_url: Option<String>,
    pub background_url: Option<String>,

    pub name: Option<String>,
    pub description: Option<String>,
    
    pub gender: Option<CharacterGender>,
    pub language: Option<CharacterLanguage>,
    
    pub prompts_scenario: Option<String>,
    pub prompts_personality: Option<String>,
    pub prompts_example_dialogue: Option<String>,
    pub prompts_first_message: Option<String>,
    pub prompts_background_stories: Option<Vec<String>>,
    pub prompts_behavior_traits: Option<Vec<String>>,

    pub creator_notes: Option<String>,

    pub tags: Option<Vec<String>>,
}
async fn update_character(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
    Json(payload): Json<UpdateCharacterRequest>,
) -> Result<AppSuccess, AppError> {
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let user = maybe_user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[update_character] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;

    let mut old_character = Character::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", character_id),
        &mut *tx
    ).await?
        .ok_or(anyhow::anyhow!("[update_character] Character not found"))?;

    if old_character.creator != user.id {
        return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("[update_character] Character not found")));
    }

    let character_history = CharacterHistory::new(old_character.clone());
    character_history.create(&mut *tx).await?;

    old_character.name = payload.name.unwrap_or(old_character.name);
    old_character.description = payload.description.unwrap_or(old_character.description);
    old_character.gender = payload.gender.unwrap_or(old_character.gender);
    old_character.language = payload.language.unwrap_or(old_character.language);
    old_character.prompts_scenario = payload.prompts_scenario.unwrap_or(old_character.prompts_scenario);
    old_character.prompts_personality = payload.prompts_personality.unwrap_or(old_character.prompts_personality);
    old_character.prompts_example_dialogue = payload.prompts_example_dialogue.unwrap_or(old_character.prompts_example_dialogue);
    old_character.prompts_first_message = payload.prompts_first_message.unwrap_or(old_character.prompts_first_message);
    old_character.prompts_background_stories = payload.prompts_background_stories.unwrap_or(old_character.prompts_background_stories);
    old_character.prompts_behavior_traits = payload.prompts_behavior_traits.unwrap_or(old_character.prompts_behavior_traits);
    old_character.creator_notes = payload.creator_notes;
    old_character.tags = payload.tags.unwrap_or(old_character.tags);

    if let Some(avatar_url) = payload.avatar_url {
        let mut found = false;
        for feature in &mut old_character.features {
            if let CharacterFeature::AvatarImage(ref mut url) = feature {
                *url = avatar_url.clone();
                found = true;
                break;
            }
        }
        if !found {
            old_character.features.push(CharacterFeature::AvatarImage(avatar_url));
        }
    }
    if let Some(background_url) = payload.background_url {
        let mut found = false;
        for feature in &mut old_character.features {
            if let CharacterFeature::BackgroundImage(ref mut url) = feature {
                *url = background_url.clone();
                found = true;
                break;
            }
        }
        if !found {
            old_character.features.push(CharacterFeature::BackgroundImage(background_url));
        }
    }

    if old_character.status != CharacterStatus::Draft {
        old_character.status = CharacterStatus::Reviewing;
    }

    old_character.version += 1;
    old_character.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character updated successfully", json!(())))
    
}


async fn create_character_sub(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let user = maybe_user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_character_sub] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;

    let character_sub = CharacterSub::new(user.id, character_id, vec![]);
    character_sub.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character sub created successfully", json!(())))
}