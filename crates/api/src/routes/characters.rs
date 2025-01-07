use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State}, 
    http::StatusCode, middleware,
    routing::{delete, get, post, put}, 
    Json, Router
};
use voda_common::{get_current_timestamp, CryptoHash};
use serde::{Deserialize, Serialize};
use serde_json::json;
use voda_database::{doc, MongoDbObject};
use voda_runtime::character::Character;

use crate::middleware::admin_only;
use crate::global_state::GlobalState;
use crate::response::{AppError, AppSuccess};

pub fn character_routes() -> Router<GlobalState> {
    Router::new()
        .route("/characters", get(list_characters))
        .route("/characters/with_filters", 
            get(list_characters_with_filters)
            .route_layer(middleware::from_fn(admin_only))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCharactersQuery {
    only_roleplay: Option<bool>,
    only_chatroom: Option<bool>,
    limit: Option<u64>,
    offset: Option<u64>,
}

async fn list_characters(
    State(state): State<GlobalState>,
    Query(query): Query<ListCharactersQuery>,
) -> Result<AppSuccess, AppError> {
    let limit = query.limit.unwrap_or(10);
    let offset = query.offset.unwrap_or(0);

    let mut filter = doc! {};

    if query.only_roleplay.unwrap_or(false) || query.only_chatroom.unwrap_or(false) {
        let mut or_conditions = Vec::new();
        
        if query.only_roleplay.unwrap_or(false) {
            or_conditions.push(doc! { "metadata.enable_roleplay": true });
        }
        
        if query.only_chatroom.unwrap_or(false) {
            or_conditions.push(doc! { "metadata.enable_chatroom": true });
        }
        
        filter.insert("$or", or_conditions);
    }

    let chars = Character::select_many(
        &state.db, filter, Some(limit as i64), Some(offset)
    ).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!(chars)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCharactersWithFiltersQuery {
    has_image: Option<bool>,
    has_roleplay_enabled: Option<bool>,
    has_chatroom_enabled: Option<bool>,

    limit: Option<u64>,
    offset: Option<u64>,
}

async fn list_characters_with_filters(
    State(state): State<GlobalState>,
    Query(query): Query<ListCharactersWithFiltersQuery>,
) -> Result<AppSuccess, AppError> {
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

    if let Some(has_chatroom_enabled) = query.has_chatroom_enabled {
        filter.insert("metadata.enable_chatroom", has_chatroom_enabled);
    }

    let characters = Character::select_many(
        &state.db, 
        filter, 
        Some(limit as i64), 
        Some(offset)
    ).await?;

    Ok(AppSuccess::new(StatusCode::OK, "Characters fetched successfully", json!(characters)))
}
