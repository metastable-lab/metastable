use anyhow::anyhow;
use metastable_common::get_current_timestamp;
use metastable_runtime::{AgentRouter, CardPool, CharacterFeature, ChatSession, DrawHistory, DrawType, Prompt, User};
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
    ensure_account,
    cache_helpers::{get_character_cached, get_session_cached},
    global_state::{AgentRouterInput, AgentRouterOutput},
    middleware::authenticate,
    response::{AppError, AppSuccess},
    GlobalState
};

pub fn runtime_routes() -> Router<GlobalState> {
    Router::new()
        .route("/runtime/call", 
            post(call_agent)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/runtime/create_session/{character_id}",
            post(create_session)
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
    RoleplayV1,
    RoleplayV1Regenerate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCallRequest {
    pub call_type: RuntimeCallType,

    pub session_id: Uuid,
    pub character_id: Option<Uuid>,
    pub message: Option<String>
}

async fn call_agent(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<RuntimeCallRequest>,
) -> Result<AppSuccess, AppError> {
    let mut user = ensure_account(&state.db, &state.redis, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent] User not found")))?;

    let price = match payload.call_type {
        RuntimeCallType::CharacterCreation => user.try_pay(3),
        RuntimeCallType::RoleplayV1 => user.try_pay(3),
        RuntimeCallType::RoleplayV1Regenerate => user.try_pay(1),
    }?;

    let mut tx = state.db.get_client().begin().await?;    
    let result = (async || match payload.call_type {
        RuntimeCallType::CharacterCreation => {
            let payload = AgentRouterInput::CharacterCreation(payload.session_id);
            let response = state.agents_router.route(&user.id, payload).await?;
            if let AgentRouterOutput::CharacterCreation(m, _, val) = response {
                let value = val
                    .ok_or_else(|| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::CharacterCreation] Value is required")))?;

                let log = user.pay_for_character_creation(price, m.id.clone())?;
                log.create(&mut *tx).await?;
                user.update(&mut *tx).await?;

                Ok(value)
            } else {
                Err(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::CharacterCreation] Unexpected response")))
            }
        }
        RuntimeCallType::RoleplayV1 | RuntimeCallType::RoleplayV1Regenerate => {
            let session = get_session_cached(&state.redis, &mut *tx, &payload.session_id).await?
                .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::RoleplayV1] Session not found")))?;

            let character = get_character_cached(&state.redis, &mut *tx, &session.character).await?
                .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent::RoleplayV1] Character not found")))?;

            let is_pure_roleplay = character.features.contains(&CharacterFeature::Roleplay);
            let character_creator = character.creator;

            let roleplay_input = match payload.call_type {
                RuntimeCallType::RoleplayV1 => {
                    let message = payload.message.clone()
                        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("[call_agent::RoleplayV1] message is required")))?;
                    let prompt = Prompt::new_user(&message);

                    RoleplayInput::ContinueSession(payload.session_id, prompt)
                }
                RuntimeCallType::RoleplayV1Regenerate => {
                    RoleplayInput::RegenerateSession(payload.session_id)
                }
                _ => unreachable!(),
            };

            let input = match is_pure_roleplay {
                true => AgentRouterInput::RoleplayV1(roleplay_input),
                false => AgentRouterInput::RoleplayCharacterCreationV1(roleplay_input),
            };

            let response = state.agents_router.route(&user.id, input).await?;
            let message_id = match response {
                AgentRouterOutput::RoleplayV1(m, _, _) => m.id,
                AgentRouterOutput::RoleplayCharacterCreationV1(m, _, _) => m.id,
                _ => {
                    return Err(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[call_agent::RoleplayV1] Unexpected response")));
                }
            };

            let mut creator = User::find_one_by_criteria(
                QueryCriteria::new().add_valued_filter("id", "=", character_creator),
                &mut *tx
            ).await?
                .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[call_agent] Creator not found")))?;

            match payload.call_type {
                RuntimeCallType::RoleplayV1 => {
                    let log = user.pay_for_chat_message(price, message_id, character_creator, 1)?;
                    if log.reward_to.is_some() {
                        let creator_log = creator.creator_reward(1);
                        creator_log.create(&mut *tx).await?;
                        creator.update(&mut *tx).await?;
                    }
                    log.create(&mut *tx).await?;
                    user.update(&mut *tx).await?;
                },
                RuntimeCallType::RoleplayV1Regenerate => {
                    let log = user.pay_for_chat_message_regenerate(price, message_id)?;
                    log.create(&mut *tx).await?;
                    user.update(&mut *tx).await?;
                },
                _ => unreachable!(),
            }

            state.memory_update_tx.send(payload.session_id).await?;

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

async fn create_session(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &state.redis, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_session] User not found")))?;

    let mut tx = state.db.get_client().begin().await?;
    let character = get_character_cached(&state.redis, &mut *tx, &character_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_session] Character not found")))?;

    let session = ChatSession::new(character_id, user.id, character.features.contains(&CharacterFeature::Roleplay));
    let session = session.create(&mut *tx).await?;

    tx.commit().await?;

    Ok(AppSuccess::new(StatusCode::OK, "Session created successfully", json!({
        "session_id": session.id,
    })))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrawCardRequest {  pub draw_type: DrawType }
async fn draw_card(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(card_pool_id): Path<Uuid>,
    Json(payload): Json<DrawCardRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &state.redis, &user_id_str).await?
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

    let _draw_cost = match payload.draw_type {
        DrawType::Single => 10,
        DrawType::Ten => 100,
    };

    // if let Ok(usage) = user.pay(draw_cost) {
    //     let user_usage = UserPointsConsumption::from_points_consumed(
    //         UserPointsConsumptionType::Others(String::new()),
    //         &user.id, usage, None, 0
    //     );
    //     user_usage.create(&mut *tx).await?;
    //     user.update(&mut *tx).await?;
    // } else {
    //     return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[draw_card] Insufficient balance")));
    // }

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