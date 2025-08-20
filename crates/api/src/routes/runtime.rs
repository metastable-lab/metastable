use anyhow::anyhow;
use metastable_common::get_current_timestamp;
use metastable_runtime::{AgentRouter, CardPool, Character, CharacterFeature, CharacterStatus, ChatSession, DrawHistory, DrawType, Prompt, User, UserPointsConsumption, UserPointsConsumptionType};
use metastable_runtime_roleplay::RoleplayInput;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use sqlx::types::Uuid;

use metastable_database::{OrderDirection, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use metastable_common::ModuleClient;

use crate::{
    ensure_account, global_state::{AgentRouterInput, AgentRouterOutput}, middleware::authenticate, response::{AppError, AppSuccess}, GlobalState
};

pub fn runtime_routes() -> Router<GlobalState> {
    Router::new()
        .route("/runtime/call", 
            post(call_agent)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/runtime/cards/draw/{card_pool_id}",
            post(draw_card)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeCallType {
    CharacterCreation,
    CharacterReview,
    RoleplayV1,
    RoleplayV1Regenerate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCallRequest {
    pub call_type: RuntimeCallType,

    pub character_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub message: Option<String>
}

async fn call_agent(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<RuntimeCallRequest>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent] User not found")))?;

    let price = match payload.call_type {
        RuntimeCallType::CharacterCreation => 3,
        RuntimeCallType::CharacterReview => 0,
        RuntimeCallType::RoleplayV1 => 3,
        RuntimeCallType::RoleplayV1Regenerate => 1,
    };

    let usage = user.pay(price)
        .map_err(|e| AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent] {}", e)))?;

    let mut tx = state.db.get_client().begin().await?;    
    let result = (async || match payload.call_type {
        RuntimeCallType::CharacterCreation => {
            // REQUIRED: session_id
            let session_id = payload.session_id
                .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent::CharacterCreation] session_id is required")))?;

            let payload = AgentRouterInput::CharacterCreation(session_id);
            let response = state.agents_router.route(&user.id, payload).await?;
            if let AgentRouterOutput::CharacterCreation(m, _, val) = response {
                let value = val
                    .ok_or_else(|| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::CharacterCreation] Value is required")))?;

                let usage = UserPointsConsumption::from_points_consumed(
                    UserPointsConsumptionType::LlmCharacterCreation(m.id),
                    &user.id, usage, None, 0
                );
                usage.create(&mut *tx).await?;
                user.update(&mut *tx).await?;

                Ok(value)
            } else {
                Err(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::CharacterCreation] Unexpected response")))
            }
        }
        RuntimeCallType::CharacterReview => {
            // REQUIRED: character_id
            let character_id = payload.character_id
                .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent::CharacterReview] character_id is required")))?;
            let mut character = Character::find_one_by_criteria(
                QueryCriteria::new().add_filter("id", "=", Some(character_id)),
                &mut *tx
            ).await?
                .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::CharacterReview] Character not found")))?;

            character.status = CharacterStatus::Reviewing;
            character.update(&mut *tx).await?;

            Ok(json!(()))
        }
        RuntimeCallType::RoleplayV1 | RuntimeCallType::RoleplayV1Regenerate => {
            // Option 1: session_id, message << just chat
            // Option 2: character_id, message << create a new session, then chat
            // THEN, dependes on character_features, decide if it is with roleplay_char or pure roleplay
            let (session_id, is_pure_roleplay, character_creator) = if let Some(session_id) = payload.session_id {
                let session = ChatSession::find_one_by_criteria(
                    QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
                    &mut *tx
                ).await?
                    .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::RoleplayV1] Session not found")))?;

                let character = session.fetch_character(&mut *tx).await?
                    .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::RoleplayV1] Character not found")))?;

                (session_id, character.features.contains(&CharacterFeature::Roleplay), character.creator)
            } else if let Some(character_id) = payload.character_id {
                let character = Character::find_one_by_criteria(
                    QueryCriteria::new().add_filter("id", "=", Some(character_id)),
                    &mut *tx
                ).await?
                    .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::RoleplayV1] Character not found")))?;

                let session = ChatSession::new(
                    character_id, user.id, 
                    character.features.contains(&CharacterFeature::Roleplay));
                let session = session.create(&mut *tx).await?;

                (session.id, character.features.contains(&CharacterFeature::Roleplay), character.creator)
            } else {
                return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent::RoleplayV1] session_id or character_id is required")));
            };

            let roleplay_input = match payload.call_type {
                RuntimeCallType::RoleplayV1 => {
                    let message = payload.message.clone()
                        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent::RoleplayV1] message is required")))?;
                    let prompt = Prompt::new_user(&message);

                    RoleplayInput::ContinueSession(session_id, prompt)
                }
                RuntimeCallType::RoleplayV1Regenerate => {
                    RoleplayInput::RegenerateSession(session_id)
                }
                _ => unreachable!(),
            };

            let input = match is_pure_roleplay {
                true => AgentRouterInput::RoleplayV1(roleplay_input),
                false => AgentRouterInput::RoleplayCharacterCreationV1(roleplay_input),
            };

            let consumtpion_type = match state.agents_router.route(&user.id, input).await? {
                AgentRouterOutput::RoleplayV1(m, _, _) => UserPointsConsumptionType::LlmCall(m.id),
                AgentRouterOutput::RoleplayCharacterCreationV1(m, _, _) => UserPointsConsumptionType::LlmCharacterCreation(m.id),
                _ => {
                    return Err(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::RoleplayV1] Unexpected response")));
                }
            };

            let mut creator = User::find_one_by_criteria(
                QueryCriteria::new().add_valued_filter("id", "=", character_creator),
                &mut *tx
            ).await?
                .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent] Creator not found")))?;

            creator.running_misc_balance += 1;
            let creator = creator.update(&mut *tx).await?;
            
            let usage = UserPointsConsumption::from_points_consumed(
                consumtpion_type, &user.id,
                usage,
                Some(creator.id), 1
            );
            usage.create(&mut *tx).await?;
            user.update(&mut *tx).await?;

            Ok(json!(()))
        }
    })().await;

    match result {
        Ok(value) => {
            tx.commit().await?;
            Ok(AppSuccess::new(StatusCode::OK, "agent call success", value))
        }
        Err(e) => {
            tx.rollback().await?;
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrawCardRequest {  pub draw_type: DrawType }
async fn draw_card(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(card_pool_id): Path<Uuid>,
    Json(payload): Json<DrawCardRequest>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[draw_card] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;

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
        let user_usage = UserPointsConsumption::from_points_consumed(
            UserPointsConsumptionType::Others(String::new()),
            &user.id, usage, None, 0
        );
        user_usage.create(&mut *tx).await?;
        user.update(&mut *tx).await?;
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