use std::sync::Arc;

use anyhow::Result;
use metastable_database::{init_databases, QueryCriteria, SqlxCrud, SqlxFilterQuery};

use metastable_runtime::{Character, Message};
use metastable_runtime_roleplay::{try_parse_content, try_prase_message};
use sqlx::types::Json;

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
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = true;
    let db = Arc::new(connect(false, false, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

    let mut tx = db.begin().await?;
    Message::toggle_trigger(&mut *tx, false).await?;
    let messages = Message::find_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("is_migrated", "=", false),
        &mut *tx
    ).await?;
    tx.commit().await?;

    for mm in messages.chunks(100) {
        let mut tx = db.begin().await?;
        for m in mm {
            let mut m = m.clone();
            let t = try_prase_message(&m)?;
            println!("{:?}", t);
            m.assistant_message_tool_call = Json(Some(t));
            m.is_migrated = true;
            m.update(&mut *tx).await?;
        }
        tx.commit().await?;
        println!("{} messages processed", mm.len());
    }

    let mut tx = db.begin().await?;
    let characters = Character::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await?;

    for character in characters {
        let first_message = &character.prompts_first_message;
        let fm = match serde_json::from_str(&first_message) {
            Ok(tc) => {
                try_parse_content(&Some(tc), &"")
            }
            Err(_) => {
                try_parse_content(&None, &first_message)
            }
        };

        let mut c = character.clone();
        c.prompts_first_message = serde_json::to_string(&fm.unwrap()).unwrap();
        c.update(&mut *tx).await?;
    }
    Message::toggle_trigger(&mut *tx, true).await?;
    tx.commit().await?;

    println!("Database initialized successfully");
    Ok(())
}