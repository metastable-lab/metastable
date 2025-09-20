use std::collections::HashMap;
use std::io::Read;
use std::{fs::File, io::Write, sync::Arc};

use anyhow::Result;
use metastable_database::{init_databases, QueryCriteria, SqlxCrud, SqlxFilterQuery};

use metastable_runtime::{Character as NewCharacter, CharacterHistory as NewCharacterHistory, Message};
use metastable_runtime_roleplay::{try_prase_message};
use sqlx::{types::Json, PgPool};
use metastable_sandbox::legacy::Character as LegacyCharacter;
use metastable_sandbox::legacy::CharacterHistory as LegacyCharacterHistory;

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

        Message,
        metastable_runtime::ChatSession,
        metastable_runtime::UserPointsLog,

        metastable_runtime::CharacterSub,
        metastable_runtime::AuditLog,

        LegacyCharacter,
        LegacyCharacterHistory,
    ],
    pgvector: [ 
        metastable_clients::EmbeddingMessage
    ]
);

fn write_to_file(content: &str, file_name: &str) -> Result<()> {
    let mut file = File::create(format!("data/{}", file_name))?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

fn read_from_file(file_name: &str) -> Result<String> {
    let mut file = File::open(format!("data/{}", file_name))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

async fn migrate_messages(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    let mut tx = db.begin().await?;
    Message::toggle_trigger(&mut *tx, false).await?;
    let messages = Message::find_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("is_migrated", "=", false),
        &mut *tx
    ).await?;
    tx.commit().await?;

    write_to_file(&serde_json::to_string(&messages).unwrap(), format!("original_messages_{}.json", environment).as_str())?;

    for mm in messages.chunks(100) {
        let mut tx = db.begin().await?;
        for m in mm {
            let mut m = m.clone();
            let t = try_prase_message(&m)?;
            m.assistant_message_tool_call = Json(Some(t));
            m.is_migrated = true;
            m.update(&mut *tx).await?;
        }
        tx.commit().await?;
        println!("{} messages processed", mm.len());
    }

    let mut tx = db.begin().await?;
    Message::toggle_trigger(&mut *tx, true).await?;
    tx.commit().await?;

    Ok(())
}

async fn offline_migrate_characters(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    let mut tx = db.begin().await?;
    let characters = LegacyCharacter::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await?;
    tracing::info!("Found {} characters", characters.len());
    let historical_characters = LegacyCharacterHistory::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await?;
    tx.commit().await?;

    tracing::info!("Found {} historical characters", historical_characters.len());
    write_to_file(&serde_json::to_string(&characters).unwrap(), &format!("original_characters_{}.json", environment))?;
    write_to_file(&serde_json::to_string(&historical_characters).unwrap(), &format!("original_historical_characters_{}.json", environment))?;

    let mut migrated_characters = Vec::new();
    let mut migrated_historical_characters = Vec::new();
    // 1. migrate characters 
    for legacy_character in characters {
        let char = legacy_character.into_new_character();
        tracing::info!("Migrated character: {}", char.id);
        migrated_characters.push(char);
    }
    for legacy_char_history in historical_characters {
        let char_history = legacy_char_history.into_new_character_history();
        tracing::info!("Migrated character history: {}", char_history.id);
        migrated_historical_characters.push(char_history);
    }
    write_to_file(&serde_json::to_string(&migrated_characters).unwrap(), &format!("migrated_characters_{}.json", environment))?;
    write_to_file(&serde_json::to_string(&migrated_historical_characters).unwrap(), &format!("migrated_historical_characters_{}.json", environment))?;
    Ok(())
}

async fn online_migrate_characters(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    // 1. disable updated_at trigger for now
    let mut tx = db.begin().await?;
    NewCharacter::toggle_trigger(&mut *tx, false).await?;
    NewCharacterHistory::toggle_trigger(&mut *tx, false).await?;
    tx.commit().await?;

    // 2. read from offline processing files
    let content = read_from_file(format!("migrated_characters_{}.json", environment).as_str())?;
    let migrated_characters = serde_json::from_str::<Vec<NewCharacter>>(&content)?;
    let mut migrate_characters_mapping = HashMap::new();
    for character in migrated_characters.clone() {
        migrate_characters_mapping.insert(character.id, character);
    }

    let content = read_from_file(format!("migrated_historical_characters_{}.json", environment).as_str())?;
    let migrated_historical_characters = serde_json::from_str::<Vec<NewCharacterHistory>>(&content)?;
    let mut migrate_historical_characters_mapping = HashMap::new();
    for character_history in migrated_historical_characters.clone() {
        migrate_historical_characters_mapping.insert(character_history.id, character_history);
    }

    let content = read_from_file(format!("original_characters_{}.json", environment).as_str())?;
    let original_characters = serde_json::from_str::<Vec<LegacyCharacter>>(&content)?;


    let content = read_from_file(format!("original_historical_characters_{}.json", environment).as_str())?;
    let original_historical_characters = serde_json::from_str::<Vec<LegacyCharacterHistory>>(&content)?;

    // 3. update the first_message first
    let mut tx = db.begin().await?;
    for mut char in original_characters {
        let new_character = migrate_characters_mapping.get(&char.id).unwrap();
        char.prompts_first_message = serde_json::to_string(&new_character.prompts_first_message.clone()).unwrap();
        char.update(&mut *tx).await?;
    }
    tx.commit().await?;

    // 4. update the historical characters
    let mut tx = db.begin().await?;
    for mut char_history in original_historical_characters {
        let new_character_history = migrate_historical_characters_mapping.get(&char_history.id).unwrap();
        char_history.prompts_first_message = serde_json::to_string(&new_character_history.prompts_first_message.clone()).unwrap();
        char_history.update(&mut *tx).await?;
    }
    tx.commit().await?;

    // 5. migrater db schema
    NewCharacter::migrate(&db).await?;
    NewCharacterHistory::migrate(&db).await?;

    // 6. populate the rest of the fields
    let mut tx = db.begin().await?;
    for character in migrated_characters.clone() {
        character.update(&mut *tx).await?;
    }
    for character_history in migrated_historical_characters.clone() {
        character_history.update(&mut *tx).await?;
    }
    tx.commit().await?;

    let mut tx = db.begin().await?;
    NewCharacter::toggle_trigger(&mut *tx, true).await?;
    NewCharacterHistory::toggle_trigger(&mut *tx, true).await?;
    tx.commit().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
    
    let run_migrations = false;
    let db = Arc::new(connect(false, false, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

    // manually migrate the message table
    Message::migrate(&db).await?;
    LegacyCharacter::migrate(&db).await?;
    LegacyCharacterHistory::migrate(&db).await?;

    migrate_messages(&db, "staging").await?;
    offline_migrate_characters(&db, "staging").await?;
    online_migrate_characters(&db, "staging").await?;

    println!("Database initialized successfully");
    Ok(())
}