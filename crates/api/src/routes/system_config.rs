use anyhow::anyhow;
use axum::extract::Path;
use axum::routing::{delete, post, put, get};
use axum::Json;
use axum::{extract::State, http::StatusCode, middleware, Router};
use serde_json::json;
use voda_common::CryptoHash;
use voda_database::{doc, MongoDbObject};
use voda_runtime::{RuntimeClient, SystemConfig};

use crate::middleware::admin_only;
use crate::response::{AppError, AppSuccess};

pub fn system_config_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/system_config", 
            get(get_system_config::<S>)
            .route_layer(middleware::from_fn(admin_only))
        )

        .route("/system_config", 
            post(create_new_system_config::<S>)
            .route_layer(middleware::from_fn(admin_only))
        )
        .route("/system_config", 
            put(update_system_config::<S>)
            .route_layer(middleware::from_fn(admin_only))
        )
        
        .route("/system_config/{id}", 
            delete(delete_system_config::<S>)
            .route_layer(middleware::from_fn(admin_only))
        )
}

async fn get_system_config<S: RuntimeClient>(
    State(state): State<S>,
) -> Result<AppSuccess, AppError> {
    let system_configs = SystemConfig::select_many_simple(&state.get_db(), doc! {}).await?;
    Ok(AppSuccess::new(
        StatusCode::OK, 
        "System config fetched successfully", 
        json!(system_configs)
    ))
}

async fn update_system_config<S: RuntimeClient>(
    State(state): State<S>,
    Json(payload): Json<SystemConfig>,
) -> Result<AppSuccess, AppError> {
    payload.update(&state.get_db()).await?;
    Ok(AppSuccess::new(
        StatusCode::OK, 
        "System config updated successfully", 
        json!(())
    ))
}

async fn create_new_system_config<S: RuntimeClient>(
    State(state): State<S>,
    Json(payload): Json<SystemConfig>,
) -> Result<AppSuccess, AppError> {
    payload.save(&state.get_db()).await?;
    Ok(AppSuccess::new(
        StatusCode::CREATED, 
        "System config created successfully", 
        json!(())
    ))
}

async fn delete_system_config<S: RuntimeClient>(
    State(state): State<S>,
    Path(id): Path<CryptoHash>,
) -> Result<AppSuccess, AppError> {
    let system_config = SystemConfig::select_one_by_index(&state.get_db(), &id).await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, anyhow!("System config not found")))?;
    system_config.delete(&state.get_db()).await?;
    Ok(AppSuccess::new(
        StatusCode::OK, 
        "System config deleted successfully", 
        json!(())
    ))
}
