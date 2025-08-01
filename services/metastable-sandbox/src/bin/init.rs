use std::sync::Arc;

use anyhow::Result;
use metastable_database::init_databases;
use metastable_runtime::RuntimeClient;
use metastable_runtime_roleplay::RoleplayRuntimeClient;
use metastable_runtime_character_creation::CharacterCreationRuntimeClient;
// use metastable_sandbox::config::get_admin_users;

// use metastable_sandbox::config::{ get_admin_user, get_normal_user };

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
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = false;
    let db = Arc::new(connect(false, false, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

    // let mut tx = db.begin().await?;
    // // // start dumping shit into the db
    // // let admin = get_admin_user();
    // // admin.create(&mut *tx).await?;

    // let users = get_admin_users();
    // for user in users {
    //     user.create(&mut *tx).await?;
    // }
    // tx.commit().await?;

    // let normal_user = get_normal_user();
    // normal_user.create(&mut *tx).await?;
    // tx.commit().await?;

    RoleplayRuntimeClient::preload(db.clone()).await?;
    CharacterCreationRuntimeClient::preload(db.clone()).await?;

    println!("Database initialized successfully");
    Ok(())
}