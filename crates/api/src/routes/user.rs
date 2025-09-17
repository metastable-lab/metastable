use anyhow::anyhow;
use async_openai::types::FunctionCall;
use axum::extract::Path;
use metastable_runtime_roleplay::agents::SendMessage;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use sqlx::types::Uuid;

use metastable_common::{get_current_timestamp, ModuleClient};
use metastable_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};

use metastable_runtime::{
    BackgroundStories, BehaviorTraits, Character, CharacterFeature, CharacterGender, CharacterHistory, CharacterLanguage, CharacterOrientation, CharacterPost, CharacterPostComments, CharacterStatus, CharacterSub, Relationships, SkillsAndInterests, ToolCall, User, UserFollow, UserReferral, UserUrl
};
use crate::{
    ensure_account, 
    middleware::authenticate, 
    response::{AppError, AppSuccess},
    GlobalState
};

pub fn user_routes() -> Router<GlobalState> {
    Router::new()
        .route("/user/register",
            post(register)
        )

        .route("/user/checkin",
            post(daily_checkin)
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
        .route("/user/follow/{following_id}",
            post(follow)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/user/character/new",
            post(new_character)
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

        .route("/user/post",
            post(create_post)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/user/post/comment/{post_id}",
            post(create_post_comment)
            .route_layer(middleware::from_fn(authenticate))
        )
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
    let mut tx = state.db.get_client().begin().await?;

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

    let mut user = user.create(&mut *tx).await?;
    let claimed_log = user.daily_checkin().expect("user MUST be able to claim on account creation"); // infallable
    let invitaion_log = user.invitation_reward(&referer.id, 200, 100);
    let invitation_reward_log = referer.invitation_reward(&user.id, 100, 200);

    referral_code.used_by = Some(user.id);
    referral_code.used_at = Some(get_current_timestamp());
    referral_code.update(&mut *tx).await?;
    referer.update(&mut *tx).await?;
    user.update(&mut *tx).await?;

    claimed_log.create(&mut *tx).await?;
    invitaion_log.create(&mut *tx).await?;
    invitation_reward_log.create(&mut *tx).await?;

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
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[buy_referral] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;

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
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_url] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let url = UserUrl::new(user.id, payload.path, payload.url_type);
    let url = url.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "URL created successfully", json!({
        "url_id": url.id,
    })))
}

async fn follow(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(following_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let follower = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[follow] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let following = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", following_id),
        &mut *tx
    ).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[follow] User not found")))?;

    let maybe_follow = UserFollow::find_one_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("follower_id", "=", follower.id)
            .add_valued_filter("following_id", "=", following.id),
        &mut *tx
    ).await?;
    if maybe_follow.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[follow] Already followed")));
    }

    let follow = UserFollow::new(follower.id, following.id);
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
    
    pub orientation: Option<CharacterOrientation>,
    pub language: Option<CharacterLanguage>,
 
    pub prompts_scenario: Option<String>,
    pub prompts_personality: Option<String>,
    pub prompts_example_dialogue: Option<String>,
    pub prompts_first_message: Option<FunctionCall>,

    pub prompts_additional_example_dialogue: Option<Vec<String>>,
    pub prompts_background_stories: Option<Vec<BackgroundStories>>,
    pub prompts_behavior_traits: Option<Vec<BehaviorTraits>>,
    pub prompts_relationships: Option<Vec<Relationships>>,
    pub prompts_skills_and_interests: Option<Vec<SkillsAndInterests>>,
    pub prompts_additional_info: Option<Vec<String>>,

    pub creator_notes: Option<String>,

    pub tags: Option<Vec<String>>,
}
async fn update_character(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
    Json(payload): Json<UpdateCharacterRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[update_character] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;

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

    let maybe_send_message= SendMessage::try_from_tool_call(
        &payload.prompts_first_message.unwrap_or(
            old_character.prompts_first_message.0.ok_or(anyhow!("[update_character] Invalid first message"))?
        )
    )?;
    let first_message = maybe_send_message.into_tool_call()?;

    old_character.name = payload.name.unwrap_or(old_character.name);
    old_character.description = payload.description.unwrap_or(old_character.description);
    old_character.orientation = payload.orientation.unwrap_or(old_character.orientation);
    old_character.language = payload.language.unwrap_or(old_character.language);
    old_character.prompts_scenario = payload.prompts_scenario.unwrap_or(old_character.prompts_scenario);
    old_character.prompts_personality = payload.prompts_personality.unwrap_or(old_character.prompts_personality);
    old_character.prompts_example_dialogue = payload.prompts_example_dialogue.unwrap_or(old_character.prompts_example_dialogue);
    old_character.prompts_first_message = sqlx::types::Json(Some(first_message));
    old_character.prompts_background_stories = sqlx::types::Json(payload.prompts_background_stories.unwrap_or(old_character.prompts_background_stories.0));
    old_character.prompts_behavior_traits = sqlx::types::Json(payload.prompts_behavior_traits.unwrap_or(old_character.prompts_behavior_traits.0));
    old_character.prompts_relationships = sqlx::types::Json(payload.prompts_relationships.unwrap_or(old_character.prompts_relationships.0));
    old_character.prompts_skills_and_interests = sqlx::types::Json(payload.prompts_skills_and_interests.unwrap_or(old_character.prompts_skills_and_interests.0));
    old_character.prompts_additional_info = sqlx::types::Json(payload.prompts_additional_info.unwrap_or(old_character.prompts_additional_info.0));
    old_character.prompts_additional_example_dialogue = sqlx::types::Json(payload.prompts_additional_example_dialogue.unwrap_or(old_character.prompts_additional_example_dialogue.0));
    old_character.creator_notes = payload.creator_notes.or(old_character.creator_notes);
    old_character.tags = payload.tags.unwrap_or(old_character.tags);

    if let Some(avatar_url) = payload.avatar_url {
        let mut found = false;
        for feature in &mut old_character.features.0 {
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
        for feature in &mut old_character.features.0 {
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

async fn new_character(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<UpdateCharacterRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[new_character] User not found")))?;

    let mut features = vec![CharacterFeature::Roleplay];
    if let Some(avatar_url) = payload.avatar_url {
        features.push(CharacterFeature::AvatarImage(avatar_url));
    }
    if let Some(background_url) = payload.background_url {
        features.push(CharacterFeature::BackgroundImage(background_url));
    }

    let first_message = {
        let tc = payload.prompts_first_message
            .ok_or(anyhow!("[new_character] Invalid first message"))?;
        SendMessage::try_from_tool_call(&tc)?.into_tool_call()?
    };

    let character = Character {
        id: Uuid::default(),
        name: payload.name.unwrap_or("Unknown".to_string()),
        description: payload.description.unwrap_or("Unknown".to_string()),
        creator: user.id,
        creation_message: None,
        creation_session: None,
        version: 1,
        status: CharacterStatus::Draft,

        gender: CharacterGender::default(),
        orientation: payload.orientation.unwrap_or_default(),
        language: payload.language.unwrap_or(CharacterLanguage::English),
        features: sqlx::types::Json(features),
        prompts_scenario: payload.prompts_scenario.unwrap_or("Unknown".to_string()),
        prompts_personality: payload.prompts_personality.unwrap_or("Unknown".to_string()),
        prompts_example_dialogue: payload.prompts_example_dialogue.unwrap_or("Unknown".to_string()),
        prompts_first_message: sqlx::types::Json(Some(first_message)),
        prompts_background_stories: sqlx::types::Json(payload.prompts_background_stories.unwrap_or(vec![BackgroundStories::default()])),
        prompts_behavior_traits: sqlx::types::Json(payload.prompts_behavior_traits.unwrap_or(vec![BehaviorTraits::default()])),
        prompts_additional_example_dialogue: sqlx::types::Json(payload.prompts_additional_example_dialogue.unwrap_or(vec![String::default()])),
        prompts_relationships: sqlx::types::Json(payload.prompts_relationships.unwrap_or(vec![Relationships::default()])),
        prompts_skills_and_interests: sqlx::types::Json(payload.prompts_skills_and_interests.unwrap_or(vec![SkillsAndInterests::default()])),
        prompts_additional_info: sqlx::types::Json(payload.prompts_additional_info.unwrap_or(vec![String::default()])),
        creator_notes: payload.creator_notes,
        tags: payload.tags.unwrap_or(vec![]),
        created_at: get_current_timestamp(),
        updated_at: get_current_timestamp(),
    };

    let mut tx = state.db.get_client().begin().await?;
    let _ = character.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character created successfully", json!(())))

}

async fn create_character_sub(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_character_sub] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let maybe_sub = CharacterSub::find_one_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("user", "=", user.id)
            .add_valued_filter("character", "=", character_id),
        &mut *tx
    ).await?;
    if maybe_sub.is_some() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[create_character_sub] Already subscribed")));
    }

    let character_sub = CharacterSub::new(user.id, character_id, vec![]);
    character_sub.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character sub created successfully", json!(())))
}

// Daily checkin handler function
async fn daily_checkin(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[daily_checkin] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let checkin_log = user.daily_checkin()?;  // 100 points per checkin    
    checkin_log.create(&mut *tx).await?;
    user.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Daily checkin successful", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePostRequest {
    pub characters: Vec<Uuid>,
    pub content: String,
}
async fn create_post(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<CreatePostRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_post] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    if payload.characters.len() < 1 || payload.characters.len() > 3 {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[create_post] Invalid characters length")));
    }

    let mut post = CharacterPost::default();
    let mut character_ids = vec![];
    for character_id in payload.characters {
        let character = Character::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", character_id),
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[create_post] Character not found"))?;

        if character.creator != user.id {
            return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("[create_post] Character not found")));
        }

        character_ids.push(character_id);
    }

    for i in 0..3 {
        if i < character_ids.len() {
            match i {
                0 => post.character_0 = Some(character_ids[i]),
                1 => post.character_1 = Some(character_ids[i]),
                2 => post.character_2 = Some(character_ids[i]),
                _ => {}
            }
        }
    }

    post.user_id = user.id;
    post.content = payload.content;
    post.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Post created successfully", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePostCommentRequest {
    pub content: String,
}
async fn create_post_comment(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(post_id): Path<Uuid>,
    Json(payload): Json<CreatePostCommentRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_post_comment] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let post = CharacterPost::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", post_id),
        &mut *tx
    ).await?
        .ok_or(anyhow::anyhow!("[create_post_comment] Post not found"))?;

    let mut post_comment = CharacterPostComments::default();
    post_comment.post = post.id;
    post_comment.user_id = user.id;
    post_comment.content = payload.content;
    post_comment.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Post comment created successfully", json!(())))
}