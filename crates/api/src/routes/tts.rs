use anyhow::anyhow;
use axum::{
    extract::{Extension, Path, State}, 
    http::StatusCode, middleware, response::IntoResponse, routing::post, Json, Router
};
use voda_common::CryptoHash;

use serde_json::Value;
use voda_database::MongoDbObject;
use voda_runtime::{Character, RuntimeClient};

use crate::{ensure_account, middleware::authenticate, voice::TTSRequest};
use crate::response::AppError;

pub fn voice_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/tts/{character_id}",
            post(tts::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
}

async fn tts<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(character_id): Path<CryptoHash>,
    Json(value): Json<Value>,
) -> Result<impl IntoResponse, AppError> {
    ensure_account(&state, &user_id, false, false, 5).await?;

    let message = value["message"].as_str().ok_or(anyhow!("message is required"))?.to_string();
    let character = Character::select_one_by_index(&state.get_db(), &character_id)
        .await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    if character.voice_model_id.is_none() || !character.metadata.enable_voice {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("Character does not have a voice")));
    }

    TTSRequest::send_request(&message, character.voice_model_id.unwrap()).await
}
