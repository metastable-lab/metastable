use std::sync::Arc;

use anyhow::Result;
use metastable_database::{init_databases, QueryCriteria, SqlxFilterQuery, SqlxCrud};

use metastable_runtime::{ChatSession, Message, MessageRole};
use metastable_sandbox::legacy::{
    RoleplayMessage, 
    RoleplaySession as LegacyRoleplaySession, 
    CharacterCreationMessage as LegacyCharacterCreationMessage
};
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

        metastable_runtime::ChatSession,
        metastable_runtime::Message,
        metastable_runtime::UserPointsLog,

        metastable_runtime::Character,
        metastable_runtime::CharacterHistory,
        metastable_runtime::CharacterSub,
        metastable_runtime::AuditLog,
    

        LegacyCharacterCreationMessage,
        LegacyRoleplaySession,
        RoleplayMessage,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

async fn migrate_messages(db: &Arc<PgPool>) -> Result<()> {
    let mut tx = db.begin().await?;
    LegacyRoleplaySession::toggle_trigger(&mut *tx, false).await?;
    Message::toggle_trigger(&mut *tx, false).await?;
    ChatSession::toggle_trigger(&mut *tx, false).await?;

    let sessions = LegacyRoleplaySession::find_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("is_migrated", "=", false)
            ,
        &mut *tx
    ).await?;
    tx.commit().await?;

    tracing::info!("Found {} sessions", sessions.len());
    let mut count = 0;
    for session in sessions {
        let mut tx = db.begin().await?;

        let mut s = session.clone();
        
        tracing::info!("Migrating session {}", session.id);
        let new_session = ChatSession {
            id: session.id,
            public: false,
            owner: session.owner,
            character: session.character,
            use_character_memory: session.use_character_memory,
            hidden: session.hidden,
            nonce: 0,
            user_mask: None,
            updated_at: session.updated_at,
            created_at: session.created_at,
        };
        let new_session = new_session.create(&mut *tx).await?;
        new_session.clone().force_set_timestamp(&mut *tx, session.created_at, session.updated_at).await?;

        tracing::info!("Fetching messages for session {}", count);
        count += 1;
        let mut messages = session.fetch_history(&mut *tx).await?;

        // Order messages by created_at
        messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        // Separate user and assistant messages
        let mut user_messages = Vec::new();
        let mut assistant_messages = Vec::new();

        for msg in &messages {
            let mut updated_msg = msg.clone();
            updated_msg.is_migrated = true;
            updated_msg.update(&mut *tx).await?;

            match msg.role {
                MessageRole::User => user_messages.push(msg),
                MessageRole::Assistant => assistant_messages.push(msg),
                _ => {}
            }
        }

        // Pair user and assistant messages by order
        let pairs = user_messages.iter().zip(assistant_messages.iter()).map(|(u, a)| (*u, *a)).collect::<Vec<_>>();

        // Now `pairs` contains tuples of (user_message, assistant_message) in order
        // You can process these pairs as needed
        let system_config = session.fetch_system_config(&mut *tx).await?
            .ok_or(anyhow::anyhow!("No system config found for session {}", session.id))?;

        for (user_msg, assistant_msg) in pairs {
            let mut message = RoleplayMessage::to_message(&system_config, &user_msg, &assistant_msg);
            message.session = Some(new_session.id);
            let msg = message.create(&mut *tx).await?;
            msg.force_set_timestamp(&mut *tx, user_msg.created_at, assistant_msg.created_at).await?;
        }
        tracing::info!("session {:?} migrated to {:?}", session.id, new_session.id);

        s.is_migrated = true;
        s.update(&mut *tx).await?;
        tx.commit().await?;
    }

    let mut tx = db.begin().await?;
    LegacyRoleplaySession::toggle_trigger(&mut *tx, false).await?;
    Message::toggle_trigger(&mut *tx, false).await?;
    ChatSession::toggle_trigger(&mut *tx, false).await?;
    tx.commit().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = true;
    let db = Arc::new(connect(false, true, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

    migrate_messages(&db).await?;
    // migrate_characters(&db).await?;

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