use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, Path, Query, State}, 
    http::StatusCode, middleware, 
    routing::{delete, get, post}, Router
};
use voda_common::CryptoHash;
use voda_database::MongoDbObject;
use voda_runtime::{Character, ConversationMemory, ExecutableFunctionCall, RuntimeClient, User};

use crate::{middleware::authenticate, response::{AppError, AppSuccess}};

pub fn conversation_routes<S: RuntimeClient<F>, F: ExecutableFunctionCall>() -> Router<S> {
    Router::new()
        .route("/conversations/public/{character_id}",get(get_public_conversation_history::<S, F>))
        .route("/conversation/public/{conversation_id}",get(get_public_conversation::<S, F>))

        .route("/conversation/{conversation_id}",
            get(get_conversation_history::<S, F>)
            .route_layer(middleware::from_fn(authenticate))
        )

        // profile
        .route("/conversations/character_list",
            get(get_character_list::<S, F>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/conversations/{character_id}",
            get(get_conversations::<S, F>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/conversations/{character_id}",
            post(new_chat::<S, F>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/conversation/{conversation_id}",
            delete(delete_chat::<S, F>)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListConversationHistoryQuery { limit: Option<u64>, offset: Option<u64> }
async fn get_public_conversation_history<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Path(character_id): Path<CryptoHash>,
    Query(payload): Query<ListConversationHistoryQuery>,
) -> Result<AppSuccess, AppError> {
    let limit = payload.limit.unwrap_or(10);
    let offset = payload.offset.unwrap_or(0);
    let conversations = ConversationMemory::find_public_conversations_by_character(
        &state.get_db(), &character_id, limit, offset
    ).await?;
    Ok(AppSuccess::new(StatusCode::OK, "Public conversation history fetched successfully", json!(conversations)))
}
async fn get_public_conversation<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Path(conversation_id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let conversation = ConversationMemory::select_one_by_index(&state.get_db(), &conversation_id).await?;
    Ok(AppSuccess::new(StatusCode::OK, "Public conversation fetched successfully", json!(conversation)))
}

async fn get_conversation_history<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(conversation_id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let _ = User::select_one_by_index(&state.get_db(), &user_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("User not found")))?;
    let conversation_memory = ConversationMemory::select_one_by_index(&state.get_db(), &conversation_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Conversation not found")))?;

    if !conversation_memory.public && conversation_memory.owner_id != user_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("You are not allowed to access this conversation")));
    }

    Ok(AppSuccess::new(StatusCode::OK, "Conversation history fetched successfully", json!(conversation_memory)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterListBrief {
    pub character_id: CryptoHash,
    pub character_name: String,
    pub character_image: Option<String>,
    pub count: usize,
}

async fn get_character_list<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let _ = User::select_one_by_index(&state.get_db(), &user_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("User not found")))?;

    let mut results = Vec::new();

    // select all conversations that the user chatted with
    let character_list = ConversationMemory::find_character_list_of_user(&state.get_db(), &user_id).await?;
    for character_id in character_list.keys() {
        let character = Character::select_one_by_index(&state.get_db(), &character_id).await?
            .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;
        results.push(CharacterListBrief {
            character_id: character_id.clone(),
            character_name: character.name,
            character_image: character.avatar_image_url,
            count: character_list[&character_id],
        });
    }

    Ok(AppSuccess::new(StatusCode::OK, "Character list fetched successfully", json!(results)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetConversationsRequest { pub limit: Option<u64> }
async fn get_conversations<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(character_id): Path<CryptoHash>,
    Query(payload): Query<GetConversationsRequest>,
) -> Result<AppSuccess, AppError> {
    let limit = payload.limit.unwrap_or(1);
    let conversations = ConversationMemory::find_latest_conversations(&state.get_db(), &user_id, &character_id, limit)
        .await?;

    Ok(AppSuccess::new(StatusCode::OK, "Conversations fetched successfully", json!({
        "conversations": conversations
    })))
}

async fn new_chat<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(character_id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    User::select_one_by_index(&state.get_db(), &user_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("User not found")))?;

    let conversation_memory = ConversationMemory::new(
        false, 
        user_id, 
        character_id
    );
    conversation_memory.save(&state.get_db()).await?;

    Ok(AppSuccess::new(
        StatusCode::CREATED, 
        "Chat created successfully", 
        json!(())
    ))
}

async fn delete_chat<S: RuntimeClient<F>, F: ExecutableFunctionCall>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(conversation_id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let conversation_memory = ConversationMemory::select_one_by_index(&state.get_db(), &conversation_id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Conversation not found")))?;

    if conversation_memory.owner_id != user_id {
        return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("You are not allowed to delete this conversation")));
    }
    conversation_memory.delete(&state.get_db()).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Chat deleted successfully", json!(())))
}
