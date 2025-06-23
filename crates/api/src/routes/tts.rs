use anyhow::anyhow;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, response::IntoResponse, routing::post, Json, Router
};
use sqlx::types::Uuid;
use voda_runtime::RuntimeClient;

use serde_json::Value;
use voda_database::{QueryCriteria, SqlxFilterQuery};
use voda_runtime_roleplay::{Character, CharacterFeature};

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
    ensure_account(&state.roleplay_client, &user_id_str, 5).await?
        .ok_or(AppError::new(StatusCode::FORBIDDEN, anyhow!("[/tts] user not found")))?;

    let message = value["message"].as_str().ok_or(anyhow!("[/tts] message is required"))?.to_string();
    let character = Character::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", character_id)?,
        &*state.roleplay_client.get_db().clone()
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

    TTSRequest::send_request(&message, voice_model_id.to_owned()).await
}
