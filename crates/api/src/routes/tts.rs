use anyhow::anyhow;
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode, middleware, response::IntoResponse, routing::post, Json, Router
};
use sqlx::types::Uuid;

use metastable_common::ModuleClient;
use metastable_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use metastable_runtime::{CharacterFeature, Message, MultimodelMessage, MultimodelMessageType, ToolCall};
use metastable_runtime_roleplay::agents::{RoleplayMessageType, SendMessage};
use metastable_clients::{TTSConfig, AudioFormat};

use crate::{
    ensure_account,
    middleware::authenticate,
    GlobalState
};
use crate::response::AppError;

pub fn voice_routes() -> Router<GlobalState> {
    Router::new()
        .route("/tts/{message_id}",
            post(tts)
            .route_layer(middleware::from_fn(authenticate))
        )
}

async fn tts(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(message_id): Path<Uuid>
) -> Result<impl IntoResponse, AppError> {
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or(AppError::new(StatusCode::FORBIDDEN, anyhow!("[/tts] user not found")))?;

    user.try_pay(6)?;
    let mut tx = state.db.get_client().begin().await?;
    let message = Message::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", message_id),
        &mut *tx
    ).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("[/tts] Message not found")))?;

    let session = message.fetch_session(&mut *tx).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("UNEXPECTED [/tts] Session not found")))?;

    let character = session.fetch_character(&mut *tx).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("UNEXPECTED [/tts] Character not found")))?;

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

    let toolcall = message.assistant_message_tool_call.0.as_ref()
        .ok_or(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[/tts] Tool call not found")))?;
    let chat_mesasges_toolcall = SendMessage::try_from_tool_call(&toolcall)?;
    let text = chat_mesasges_toolcall.messages.iter()
        .filter_map(|message| {
            if let RoleplayMessageType::Chat(content) = message {
                if !content.is_empty() { Some(content) } 
                else { None }
            } else { None }
        })
        .cloned()
        .collect::<Vec<String>>()
        .join("\n");

    // Create TTS configuration
    let tts_config = TTSConfig {
        reference_id: Some(voice_model_id.to_owned()),
        model_name: Some("s1".to_string()),
        text,
        format: Some(AudioFormat::Mp3),
        ..Default::default()
    };

    // Generate TTS and upload to R2
    let audio_url = state.fish_audio_client
        .generate_and_upload_to_r2(tts_config, &state.r2_client)
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[/tts] Failed to generate and upload audio: {}", e)))?;

    let multimodel_message = MultimodelMessage {
        id: Uuid::default(),
        owner: message.owner,
        character_id: character.id,
        message_id,
        message_type: MultimodelMessageType::Voice,
        r2_url: audio_url.clone(),
        created_at: 0,
        updated_at: 0,
    };
    multimodel_message.create(&mut *tx).await?;
    
    let log = user.pay_for_voice_generation(6, message_id)?;
    log.create(&mut *tx).await?;
    user.update(&mut *tx).await?;
    tx.commit().await?;

    // Return the audio URL as JSON
    Ok(Json(serde_json::json!({
        "audio_url": audio_url
    })))
}
