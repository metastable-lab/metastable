use anyhow::anyhow;
use metastable_common::get_current_timestamp;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use sqlx::types::Uuid;
use metastable_runtime::{user::UserUsagePoints, CardPool, DrawHistory, DrawType, RuntimeClient, UserUsage};
use metastable_runtime_character_creation::CharacterCreationMessage;
use metastable_runtime_roleplay::{Character, CharacterStatus, RoleplayMessage, RoleplaySession};
use metastable_database::{OrderDirection, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use metastable_runtime::SystemConfig;

use crate::{
    ensure_account, 
    middleware::authenticate, 
    response::{AppError, AppSuccess},
    GlobalState
};

pub fn runtime_routes() -> Router<GlobalState> {
    Router::new()
        .route("/runtime/roleplay/create_session",
            post(roleplay_create_session)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/roleplay/chat/{session_id}",
            post(roleplay_chat)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/roleplay/rollback/{session_id}",
            post(roleplay_rollback)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/character-creation/create",
            post(character_creation_create)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/character-creation/review/{character_id}",
            post(character_creation_review)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/cards/draw/{card_pool_id}",
            post(draw_card)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionRequest { pub character_id: Uuid, pub system_config_id: Uuid }
async fn roleplay_create_session(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<CreateSessionRequest>,
) -> Result<AppSuccess, AppError> {
    let (user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let user = user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_create_session] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let _character = Character::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", payload.character_id),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_create_session] Character not found")))?;
    let _system_config = SystemConfig::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", payload.system_config_id),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_create_session] System config not found")))?;

    let mut session = RoleplaySession::default();
    session.character = payload.character_id;
    session.system_config = payload.system_config_id;
    session.owner = user.id;
    session.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Session created successfully", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest { pub message: String }
async fn roleplay_chat(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ChatRequest>,
) -> Result<AppSuccess, AppError> {
    let (user, points_consumed) = ensure_account(&state.roleplay_client, &user_id_str, 3).await?;
    let user = user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] User not found")))?;

    let message = RoleplayMessage::user_message(
        &payload.message, &session_id,  &user.id
    );

    let response = state.roleplay_client.on_new_message(&message).await?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let user_usage = UserUsage::from_llm_response(&response, points_consumed.clone());
    user_usage.create(&mut *tx).await?;

    if points_consumed.points_consumed_purchased > 0 {
        let mut creator = RoleplaySession::find_one_by_criteria(
            QueryCriteria::new().add_filter("id", "=", Some(session_id)),
            &mut *tx
        ).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Character not found")))?
            .fetch_character(&mut *tx).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Character not found")))?
            .fetch_creator(&mut *tx).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Creator not found")))?;

        creator.running_misc_balance += 1;
        creator.update(&mut *tx).await?;
    }

    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Chat completed successfully", json!(response)))
}

async fn roleplay_rollback(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(session_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let (user, points_consumed) = ensure_account(&state.roleplay_client, &user_id_str, 3).await?;
    let user = user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_rollback] User not found")))?;

    let message = RoleplayMessage::user_message(
        "rollback", &session_id,  &user.id
    );

    let response = state.roleplay_client.on_rollback(&message).await?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    let user_usage = UserUsage::from_llm_response(&response, points_consumed.clone());
    user_usage.create(&mut *tx).await?;

    if points_consumed.points_consumed_purchased > 0 {
        let mut creator = RoleplaySession::find_one_by_criteria(
            QueryCriteria::new().add_filter("id", "=", Some(session_id)),
            &mut *tx
        ).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Character not found")))?
            .fetch_character(&mut *tx).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Character not found")))?
            .fetch_creator(&mut *tx).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[roleplay_chat] Creator not found")))?;

        creator.running_misc_balance += 1;
        creator.update(&mut *tx).await?;
    }

    tx.commit().await?;
    Ok(AppSuccess::new(StatusCode::OK, "Last message regenerated successfully", json!(())))
}


#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCharacterRequest { pub roleplay_session_id: Uuid }
async fn character_creation_create(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<CreateCharacterRequest>,
) -> Result<AppSuccess, AppError> {
    let (user, _) = ensure_account(&state.character_creation_client, &user_id_str, 1).await?;
    let user = user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[character_creation_create] User not found")))?;

    let message = CharacterCreationMessage::blank_user_message(
        &payload.roleplay_session_id, &user.id
    );
    let response = state.character_creation_client.on_new_message(&message).await?;
    let misc_value = response.clone().misc_value.ok_or(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[character_creation_create] Character creation response misc value not found")))?;

    let mut tx = state.character_creation_client.get_db().begin().await?;
    let user_usage = UserUsage::from_llm_response(&response, UserUsagePoints::default());
    user_usage.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character creation completed successfully", misc_value))
}

async fn character_creation_review(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let (user, _) = ensure_account(&state.character_creation_client, &user_id_str, 0).await?;
    let _ = user.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[character_creation_review] User not found")))?;

    let mut tx = state.character_creation_client.get_db().begin().await?;
    let mut character = Character::find_one_by_criteria(
        QueryCriteria::new().add_filter("id", "=", Some(character_id)),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[character_creation_review] Character not found")))?;

    character.status = CharacterStatus::Reviewing;
    character.update(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Character creation review completed successfully", json!(())))
}


#[derive(Debug, Serialize, Deserialize)]
pub struct DrawCardRequest {  pub draw_type: DrawType }
async fn draw_card(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(card_pool_id): Path<Uuid>,
    Json(payload): Json<DrawCardRequest>,
) -> Result<AppSuccess, AppError> {

    let (user, _) = ensure_account(&state.roleplay_client, &user_id_str, 1).await?;
    let mut user = user.ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[draw_card] User not found")))?;

    let mut tx = state.roleplay_client.get_db().begin().await?;
    
    let card_pool = CardPool::find_one_by_criteria(
        QueryCriteria::new().add_filter("id", "=", Some(card_pool_id)),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[draw_card] Card pool not found")))?;

    let cards = card_pool.fetch_card_ids(&mut *tx).await?;
    let current_time = get_current_timestamp();
    if current_time > card_pool.end_time { 
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[draw_card] Card pool is not active")));
    }

    let last_draw = if let Some(latest_draw) = DrawHistory::find_one_by_criteria(
        QueryCriteria::new()
            .add_filter("card_pool_id", "=", Some(card_pool_id))
            .add_filter("user", "=", Some(user.id))
            .order_by("created_at", OrderDirection::Desc)
            .limit(1),
        &mut *tx
    ).await? {
        latest_draw
    } else {
        let mut draw = DrawHistory::default();
        draw.user = user.id;
        draw.card_pool_id = card_pool_id;
        draw
    };

    let draw_cost = match payload.draw_type {
        DrawType::Single => 10,
        DrawType::Ten => 100,
    };

    if let Ok(usage) = user.pay(draw_cost) {
        let user_usage = UserUsage::from_points_consumed(user.id, usage);
        user_usage.create(&mut *tx).await?;
    } else {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[draw_card] Insufficient balance")));
    }

    // begin draw cards
    let results = match payload.draw_type {
        DrawType::Single => {
            vec![DrawHistory::execute_single_draw(&last_draw, &card_pool, &cards)?]
        },
        DrawType::Ten => {
            DrawHistory::draw_ten_cards(&last_draw, &card_pool, &cards)?
        },
    };

    for result in results {
        result.create(&mut *tx).await?;
    }
    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Draw card completed successfully", json!(())))
}