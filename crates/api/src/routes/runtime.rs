use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};
use sqlx::types::Uuid;
use voda_runtime::RuntimeClient;
use voda_runtime_roleplay::RoleplayMessage;

use crate::{
    ensure_account, 
    middleware::authenticate, 
    response::{AppError, AppSuccess},
    GlobalState
};
// use crate::metrics::*;

pub fn runtime_routes() -> Router<GlobalState> {
    Router::new()
        .route("/runtime/roleplay/chat/{session_id}",
            post(roleplay_chat)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/runtime/roleplay/rollback/{session_id}",
            post(roleplay_rollback)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest { pub message: String }
async fn roleplay_chat(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ChatRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.roleplay_client, &user_id_str, 1).await?
        .expect("[roleplay_chat] User not found");

    let message = RoleplayMessage::user_message(
        &payload.message, &session_id,  &user.id
    );

    let response = state.roleplay_client.on_new_message(&message).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Chat completed successfully", json!(response)))
}

async fn roleplay_rollback(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ChatRequest>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.roleplay_client, &user_id_str, 1).await?
        .expect("[roleplay_rollback] User not found");

    let message = RoleplayMessage::user_message(
        &payload.message, &session_id,  &user.id
    );

    let response = state.roleplay_client.on_rollback(&message).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Last message regenerated successfully", json!(response)))
}
