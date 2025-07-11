use std::sync::Arc;

use anyhow::Result;
use voda_database::{init_db_pool, SqlxCrud};
use voda_runtime::{RuntimeClient, SystemConfig, User, UserBadge, UserFollow, UserReferral, UserUrl, UserUsage};
use voda_runtime_roleplay::{AuditLog, Character, RoleplayMessage, RoleplayRuntimeClient, RoleplaySession};
use voda_runtime_character_creation::{CharacterCreationMessage, CharacterCreationRuntimeClient};

use voda_sandbox::config::{ get_admin_user, get_normal_user };

init_db_pool!(
    UserFollow
);

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let db = Arc::new(connect(false, false).await.clone());
    // let mut tx = db.begin().await?;

    // // start dumping shit into the db
    // let admin = get_admin_user();
    // admin.create(&mut *tx).await?;

    // let normal_user = get_normal_user();
    // normal_user.create(&mut *tx).await?;
    // tx.commit().await?;

    RoleplayRuntimeClient::preload(db.clone()).await?;
    CharacterCreationRuntimeClient::preload(db.clone()).await?;

    println!("Database initialized successfully");
    Ok(())
}