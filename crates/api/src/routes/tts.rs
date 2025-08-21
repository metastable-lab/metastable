use anyhow::anyhow;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, response::IntoResponse, routing::post, Json, Router
};
use sqlx::types::Uuid;

use serde_json::Value;
use metastable_common::ModuleClient;
use metastable_database::{QueryCriteria, SqlxFilterQuery};
use metastable_runtime::{Character, CharacterFeature};

use crate::{
    ensure_account, 
    middleware::authenticate, 
    voice::TTSRequest,
    GlobalState
};
use crate::response::AppError;

pub fn voice_routes() -> Router<GlobalState> {
    Router::new()
        .route("/tts/{character_id}",
            post(tts)
            .route_layer(middleware::from_fn(authenticate))
        )
}

async fn tts(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(character_id): Path<Uuid>,
    Json(value): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    let _ = ensure_account(&state.db, &user_id_str).await?
        .ok_or(AppError::new(StatusCode::FORBIDDEN, anyhow!("[/tts] user not found")))?;

    let message = value["message"].as_str().ok_or(anyhow!("[/tts] message is required"))?.to_string();

    let mut tx = state.db.get_client().begin().await?;
    let character = Character::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", character_id),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[/tts] Character not found")))?;

    let voice_model_id = character.features
        .iter()
        .find_map(|feature| {
            if let CharacterFeature::Voice(voice_model_id) = feature {
                Some(voice_model_id)
            } else {
                None
            }
        })
        .ok_or(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/tts] Character does not have a voice")))?;

    tx.commit().await?;

    TTSRequest::send_request(&message, voice_model_id.to_owned()).await
}
