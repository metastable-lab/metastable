use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use reqwest;

use metastable_service_api::{
    graphql_route, misc_routes, runtime_routes, setup_tracing, voice_routes, user_routes, auth_routes, GlobalState
};

use metastable_database::init_databases;
use metastable_runtime_character_creation::CharacterCreationRuntimeClient;
use metastable_runtime_roleplay::RoleplayRuntimeClient;
use metastable_runtime::Memory;

init_databases!(
    default: [
        metastable_runtime::User,
        metastable_runtime::UserUsage,
        metastable_runtime::UserUrl,
        metastable_runtime::UserReferral,
        metastable_runtime::UserBadge,
        metastable_runtime::UserFollow,
        metastable_runtime::SystemConfig,

        metastable_runtime_roleplay::Character,
        metastable_runtime_roleplay::RoleplaySession,
        metastable_runtime_roleplay::RoleplayMessage,
        metastable_runtime_roleplay::AuditLog,

        metastable_runtime_character_creation::CharacterCreationMessage
    ],
    pgvector: [
        metastable_runtime_mem0::EmbeddingMessage
    ]
);

#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing();

    let cors = CorsLayer::very_permissive();
    let trace = TraceLayer::new_for_http();

    let db_pool = Arc::new(connect(false, false, false).await.clone());
    let pgvector_db = Arc::new(connect_pgvector(false, false, false).await.clone());

    let (roleplay_client, mut mem0_messages_rx) = RoleplayRuntimeClient::new(db_pool.clone(), pgvector_db.clone()).await?;
    let character_creation_client = CharacterCreationRuntimeClient::new(db_pool.clone(), "character_creation_v0".to_string()).await?;
    let mem0_engine = roleplay_client.get_mem0_engine_clone();

    let global_state = GlobalState {
        roleplay_client: roleplay_client,
        character_creation_client: character_creation_client,
        http_client: reqwest::Client::new(),
    };

    tokio::spawn(async move {
        while let Some(mem0_messages) = mem0_messages_rx.recv().await {
            let adding_result = mem0_engine.add_messages(&mem0_messages).await;
            if let Err(e) = adding_result {
                tracing::warn!("[Mem0Engine::add_messages] Failed to add messages: {:?}", e);
            }
        }
    });

    let app = Router::new()
        .merge(misc_routes())
        .merge(runtime_routes())
        .merge(voice_routes())
        .merge(graphql_route())
        .merge(user_routes())
        .merge(auth_routes())
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
