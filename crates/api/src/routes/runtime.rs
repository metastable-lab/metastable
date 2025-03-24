use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use voda_common::CryptoHash;
use voda_database::MongoDbObject;
use voda_runtime::{Character, ConversationMemory, HistoryMessage, RuntimeClient, User};

use crate::{middleware::authenticate, response::{AppError, AppSuccess}};

pub fn runtime_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/runtime/chat/{conversation_id}",
            post(chat::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/regenerate_last_message/{conversation_id}",
            post(regenerate::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest { pub message: String }
async fn chat<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(conversation_id): Path<CryptoHash>,
    Json(payload): Json<ChatRequest>,
) -> Result<AppSuccess, AppError> {
    let mut user = User::select_one_by_index(&state.get_db(), &user_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("User not found")))?;
    User::pay_and_update(&state.get_db(), &user_id, state.get_price()).await?;

    let mut conversation_memory = ConversationMemory::select_one_by_index(&state.get_db(), &conversation_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Conversation not found")))?;
    
    if !conversation_memory.public && conversation_memory.owner_id != user_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("You are not allowed to chat in this conversation")));
    }
    let character = Character::select_one_by_index(&state.get_db(), &conversation_memory.character_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    let mut new_message = HistoryMessage::default();
    new_message.content = payload.message;
    new_message.owner = user_id;
    new_message.character_id = conversation_memory.character_id.clone();

    let system_config = state.find_system_config_by_character(&character).await?;
    let response_message = state
        .run(
            &character, &mut user, &system_config, 
            &mut conversation_memory, &new_message
        ).await?;

    conversation_memory.update(&state.get_db()).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Chat completed successfully", json!(response_message)))
}

async fn regenerate<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(conversation_id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let mut user = User::select_one_by_index(&state.get_db(), &user_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("User not found")))?;
    User::pay_and_update(&state.get_db(), &user_id, 5).await?;

    let mut conversation_memory = ConversationMemory::select_one_by_index(&state.get_db(), &conversation_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Conversation not found")))?;

    if !conversation_memory.public && conversation_memory.owner_id != user_id {
        return Err(AppError::new(
            StatusCode::FORBIDDEN, 
            anyhow!("You are not allowed to regenerate messages in this conversation")
        ));
    }

    if conversation_memory.history.is_empty() {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("No messages to regenerate")));
    }

    let character = Character::select_one_by_index(&state.get_db(), &conversation_memory.character_id)
        .await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    let system_config = state
        .find_system_config_by_character(&character).await?;
    
    let response_message = state
        .regenerate(
            &character, &mut user, &system_config, 
            &mut conversation_memory
        ).await?;

    conversation_memory.update(&state.get_db()).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Last message regenerated successfully", json!(response_message)))
}
