use std::sync::Arc;

use anyhow::Result;
use voda_database::{init_db_pool, SqlxCrud};
use voda_runtime::{SystemConfig, User, UserBadge, UserReferral, UserUrl, UserUsage};
use voda_runtime_roleplay::{AuditLog, Character, RoleplayMessage, RoleplaySession};

use voda_sandbox::config::{
    get_admin_user, get_characters, get_normal_user, get_system_configs,
};

init_db_pool!(
    User,
    UserUsage,
    UserUrl,
    UserReferral,
    UserBadge,
    SystemConfig,
    Character,
    RoleplaySession,
    RoleplayMessage,
    AuditLog
);

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let db = Arc::new(connect(true, true).await.clone());
    let mut tx = db.begin().await?;

    // start dumping shit into the db
    let admin = get_admin_user();
    admin.create(&mut *tx).await?;

    let normal_user = get_normal_user();
    let normal_user = normal_user.create(&mut *tx).await?;

    let characters = get_characters(normal_user.id);
    for character in characters {
        character.create(&mut *tx).await?;
    }

    let system_configs = get_system_configs();
    for system_config in system_configs {
        system_config.create(&mut *tx).await?;
    }
    tx.commit().await?;

    println!("Database initialized successfully");
    Ok(())
}