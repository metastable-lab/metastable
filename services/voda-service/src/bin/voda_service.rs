use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::sync::mpsc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use voda_service_api::{
    graphql_route, misc_routes, runtime_routes, setup_tracing, voice_routes, GlobalState
};

use voda_database::init_db_pool;
use voda_runtime::user::{UserProfile, UserUrl};
use voda_runtime::{SystemConfig, User, UserMetadata, UserPoints, UserUsage};
use voda_runtime_roleplay::{AuditLog, Character, RoleplayMessage, RoleplayRawMemory, RoleplayRuntimeClient, RoleplaySession};

init_db_pool!(
    User, UserUsage, UserProfile, SystemConfig, UserPoints, UserMetadata, UserUrl,
    Character, RoleplaySession, RoleplayMessage, AuditLog
);


#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing();

    let cors = CorsLayer::very_permissive();
    let trace = TraceLayer::new_for_http();

    let db_pool = Arc::new(connect(false).await.clone());

    let (executor, _execution_queue) = mpsc::channel(100);
    let memory = RoleplayRawMemory::new(db_pool.clone());
    let client = RoleplayRuntimeClient::new(db_pool.clone(), Arc::new(memory), executor).await;

    let global_state = GlobalState {
        roleplay_client: client,
    };

    let app = Router::new()
        .merge(misc_routes())
        .merge(runtime_routes())
        .merge(voice_routes())
        .merge(graphql_route())

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
