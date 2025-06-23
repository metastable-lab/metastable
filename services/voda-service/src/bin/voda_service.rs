use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::sync::mpsc;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use reqwest;

use voda_service_api::{
    graphql_route, misc_routes, runtime_routes, setup_tracing, voice_routes, user_routes, GlobalState
};

use voda_database::init_db_pool;
use voda_runtime::{SystemConfig, User, UserBadge, UserReferral, UserUrl, UserUsage, RuntimeClient};
use voda_runtime_character_creation::{CharacterCreationMessage, CharacterCreationRuntimeClient};
use voda_runtime_roleplay::{AuditLog, Character, RoleplayMessage, RoleplayRuntimeClient, RoleplaySession};

init_db_pool!(
    User, UserUsage, UserUrl, UserReferral, UserBadge, SystemConfig,
    Character, RoleplaySession, RoleplayMessage, AuditLog,
    CharacterCreationMessage
);

#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing();

    let cors = CorsLayer::very_permissive();
    let trace = TraceLayer::new_for_http();

    let db_pool = Arc::new(connect(false, false).await.clone());

    let (roleplay_executor, _execution_queue) = mpsc::channel(100);
    let roleplay_client = RoleplayRuntimeClient::new(db_pool.clone(), roleplay_executor).await?;
    let (character_creation_executor, character_creation_queue) = mpsc::channel(100);
    let character_creation_client = CharacterCreationRuntimeClient::new(
        db_pool.clone(), "character_creation_v0".to_string(), 
        character_creation_executor
    ).await?;

    tokio::spawn(async move { 
        let _ = CharacterCreationRuntimeClient::init_function_executor(character_creation_queue).await; 
    });

    let global_state = GlobalState {
        roleplay_client: roleplay_client,
        character_creation_client: character_creation_client,
        http_client: reqwest::Client::new(),
    };

    let app = Router::new()
        .merge(misc_routes())
        .merge(runtime_routes())
        .merge(voice_routes())
        .merge(graphql_route())
        .merge(user_routes())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(3600)))
        .layer(cors)
        .layer(trace)
        .with_state(global_state);

    let port: u16 = std::env::var("PORT")
        .unwrap_or("3033".into())
        .parse()
        .expect("failed to convert to number");

    let listener = tokio::net::TcpListener::bind(format!(":::{port}"))
        .await
        .unwrap();

    tracing::info!("LISTENING ON {port}");
    axum::serve(listener, app.into_make_service()).await.unwrap();
    Ok(())
}
