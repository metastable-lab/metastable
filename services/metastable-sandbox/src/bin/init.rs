use std::sync::Arc;

use anyhow::Result;

use metastable_database::init_databases;

init_databases!(
    default: [
        metastable_runtime::User,
        metastable_runtime::UserUrl,
        metastable_runtime::UserReferral,
        metastable_runtime::UserBadge,
        metastable_runtime::UserFollow,
        metastable_runtime::UserPayment,
        metastable_runtime::UserNotification,

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
        metastable_runtime::CharacterMask,
        metastable_runtime::CharacterPost,
        metastable_runtime::CharacterPostComments,
        metastable_runtime::AuditLog,

        metastable_runtime::MultimodelMessage,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = true;
    let _db = Arc::new(connect(false, true, run_migrations).await.clone());
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
    
    println!("Database initialized successfully");
    Ok(())
}