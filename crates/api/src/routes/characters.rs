use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State}, http::StatusCode, middleware, routing::{delete, get, post, put}, Extension, Json, Router
};
use voda_common::{get_current_timestamp, CryptoHash};
use serde::{Deserialize, Serialize};
use serde_json::json;
use voda_database::{doc, MongoDbObject};
use voda_runtime::{Character, RuntimeClient, UserRole};

use crate::middleware::{authenticate, ensure_account};
use crate::response::{AppError, AppSuccess};

pub fn character_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/characters", get(list_characters::<S>))
        .route("/characters/count", get(list_characters_count::<S>))
        .route("/character/{id}", get(get_character::<S>))

        .route("/characters/with_filters", 
            get(list_characters_with_filters::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/characters/with_filters/count", 
            get(list_characters_with_filters_count::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/character", post(create_character::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/character/{id}", put(update_character::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
        .route("/character/{id}", delete(delete_character::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
        
        .route("/character/status/{id}", post(set_character_status::<S>)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCharactersQuery {
    limit: Option<u64>, offset: Option<u64>,
}
async fn list_characters<S: RuntimeClient>(
    State(state): State<S>,
    Query(query): Query<ListCharactersQuery>,
) -> Result<AppSuccess, AppError> {
    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);

    let filter = doc! { "metadata.enable_roleplay": true };
    let chars = Character::select_many(
        &state.get_db(), filter, Some(limit as i64), Some(offset)
    ).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!(chars)))
}
async fn list_characters_count<S: RuntimeClient>(
    State(state): State<S>,
) -> Result<AppSuccess, AppError> {
    let filter = doc! { "metadata.enable_roleplay": true };
    let count = Character::total_count(&state.get_db(), filter).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!({
        "count": count
    })))
}
async fn get_character<S: RuntimeClient>(
    State(state): State<S>,
    Path(id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let character = Character::select_one_by_index(&state.get_db(), &id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    Ok(AppSuccess::new(
        StatusCode::OK, 
        "Character fetched successfully", 
        json!(character)
    ))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCharactersWithFiltersQuery {
    has_image: Option<bool>,
    has_roleplay_enabled: Option<bool>,

    limit: Option<u64>,
    offset: Option<u64>,
}
async fn list_characters_with_filters<S: RuntimeClient>(
    State(state): State<S>,
    Query(query): Query<ListCharactersWithFiltersQuery>,
    Extension(user_id): Extension<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    ensure_account(&state, &user_id, false, true, 0).await?;

    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);

    let mut filter = doc! {};

    if let Some(has_image) = query.has_image {
        filter.insert("$and", vec![
            doc! { "background_image_url": { "$exists": has_image, "$ne": null } },
            doc! { "avatar_image_url": { "$exists": has_image, "$ne": null } }
        ]);
    }

    if let Some(has_roleplay_enabled) = query.has_roleplay_enabled {
        filter.insert("metadata.enable_roleplay", has_roleplay_enabled);
    }

    let characters = Character::select_many(
        &state.get_db(),  filter, Some(limit as i64), Some(offset)
    ).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!(characters)))
}
async fn list_characters_with_filters_count<S: RuntimeClient>(
    State(state): State<S>,
    Query(query): Query<ListCharactersWithFiltersQuery>,
    Extension(user_id): Extension<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    ensure_account(&state, &user_id, false, true, 0).await?;

    let mut filter = doc! {};

    if let Some(has_image) = query.has_image {
        filter.insert("$and", vec![
            doc! { "background_image_url": { "$exists": has_image, "$ne": null } },
            doc! { "avatar_image_url": { "$exists": has_image, "$ne": null } }
        ]);
    }

    if let Some(true) = query.has_roleplay_enabled {
        filter.insert("metadata.enable_roleplay", true);
    }

    let count = Character::total_count(&state.get_db(), filter).await?;
    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!({
        "count": count
    })))
}

async fn create_character<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Json(mut payload): Json<Character>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state, &user_id, false, false, 100).await?
        .expect("user must have been registered");

    if user.role == UserRole::Admin || payload.metadata.creator == user_id {
        payload.clean()?;
        payload.published_at = get_current_timestamp();
        payload.created_at = get_current_timestamp();
        payload.updated_at = get_current_timestamp();

        payload.clone().save(&state.get_db()).await?;
    } else {
        return Err(AppError::new(
            StatusCode::FORBIDDEN, 
            anyhow!("You are not authorized to create a character")
        ));
    }

    Ok(AppSuccess::new(
        StatusCode::CREATED,
        "Character created successfully",
        json!(payload.id)
    ))
}

async fn update_character<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(id): Path<CryptoHash>,
    Json(mut payload): Json<Character>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state, &user_id, false, false, 0).await?
        .expect("user must have been registered");

    let character = Character::select_one_by_index(&state.get_db(), &id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    if user.role == UserRole::Admin || character.metadata.creator == user_id {
        payload.clean()?;
        payload.id = id;
        payload.updated_at = get_current_timestamp();
        payload.update(&state.get_db()).await?;
    } else {
        return Err(AppError::new(
            StatusCode::FORBIDDEN, 
            anyhow!("You are not authorized to update this character")
        ));
    }

    Ok(AppSuccess::new(
        StatusCode::OK,
        "Character updated successfully",
        json!({ "id": payload.id })
    ))
}

async fn delete_character<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state, &user_id, false, false, 0).await?
        .expect("user must have been registered");

    let character = Character::select_one_by_index(&state.get_db(), &id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    if user.role == UserRole::Admin || character.metadata.creator == user_id {
        character.delete(&state.get_db()).await?;
    } else {
        return Err(AppError::new(
            StatusCode::FORBIDDEN, 
            anyhow!("You are not authorized to delete this character")
        ));
    }
    Ok(AppSuccess::new(
        StatusCode::OK, 
        "Character deleted successfully", 
        json!({ "id": id })
    ))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetCharacterStatusPayload {
    roleplay_status: Option<bool>,
}
async fn set_character_status<S: RuntimeClient>(
    State(state): State<S>,
    Extension(user_id): Extension<CryptoHash>,
    Path(id): Path<CryptoHash>,
    Json(payload): Json<SetCharacterStatusPayload>,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state, &user_id, false, false, 0).await?
        .expect("user must have been registered");

    let mut character = Character::select_one_by_index(&state.get_db(), &id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("Character not found")))?;

    if user.role == UserRole::Admin || character.metadata.creator == user_id {
        character.metadata.enable_roleplay = payload.roleplay_status.unwrap_or(character.metadata.enable_roleplay);
        character.updated_at = get_current_timestamp();
        character.update(&state.get_db()).await?;
    } else {
        return Err(AppError::new(
            StatusCode::FORBIDDEN, 
            anyhow!("You are not authorized to update this character")
        ));
    }

    Ok(AppSuccess::new(StatusCode::OK, "Character status updated successfully", json!(())))
}
