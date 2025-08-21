use anyhow::Result;
use axum::Router;
use metastable_runtime_roleplay::MemoryUpdater;
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use metastable_service_api::{
    graphql_route, misc_routes, runtime_routes, setup_tracing, voice_routes, user_routes, auth_routes, GlobalState
};

use metastable_database::init_databases;

init_databases!(
    default: [
        metastable_runtime::User,
        metastable_runtime::UserUrl,
        metastable_runtime::UserReferral,
        metastable_runtime::UserBadge,
        metastable_runtime::UserFollow,

        metastable_runtime::SystemConfig,

        metastable_runtime::CardPool,
        metastable_runtime::Card,
        metastable_runtime::DrawHistory,

        metastable_runtime::Message,
        metastable_runtime::ChatSession,
        metastable_runtime::UserPointsLog,

        metastable_runtime::Character,
        metastable_runtime::CharacterHistory,
        metastable_runtime::CharacterSub,
        metastable_runtime::AuditLog,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing();

    let cors = CorsLayer::very_permissive();
    let trace = TraceLayer::new_for_http();

    let (global_state, memory_updater_rx) = GlobalState::new().await?;
    println!("Global State Init");
    tokio::spawn(async move {
        let memory_updater = MemoryUpdater::new().await.unwrap();
        memory_updater.run(memory_updater_rx).await.unwrap();
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
