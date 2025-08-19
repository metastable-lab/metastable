use std::sync::Arc;

use anyhow::Result;
use metastable_database::init_databases;
use sqlx::PgPool;

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

        metastable_runtime_roleplay::Character,
        metastable_runtime_roleplay::CharacterHistory,
        metastable_runtime_roleplay::CharacterSub,
        metastable_runtime_roleplay::RoleplaySession,
        metastable_runtime_roleplay::AuditLog,
    ],
    pgvector: [ 
        metastable_runtime_mem0::EmbeddingMessage
    ]
);

pub async fn migrate_database(old_db: &Arc<PgPool>, new_db: &Arc<PgPool>) -> Result<()> {
    let mut tx = new_db.begin().await?;
    let old_db = old_db.clone();
    let new_db = new_db.clone();
    
    
}


#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    // let run_migrations = true;
    // let db = Arc::new(connect(false, true, run_migrations).await.clone());
    // let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

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
    
    println!("Database initialized successfully");
    Ok(())
}