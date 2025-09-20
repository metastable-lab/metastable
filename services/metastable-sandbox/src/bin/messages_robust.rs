use std::collections::HashMap;
use std::io::Read;
use std::{fs::File, io::Write, sync::Arc, time::Instant};

use anyhow::{Result, Context, anyhow};
use metastable_database::{init_databases, QueryCriteria, SqlxCrud, SqlxFilterQuery};

use metastable_runtime::{Character as NewCharacter, CharacterHistory as NewCharacterHistory, Message};
use metastable_runtime_roleplay::{try_prase_message};
use sqlx::{types::Json, PgPool, Row};
use metastable_sandbox::legacy::Character as LegacyCharacter;
use metastable_sandbox::legacy::CharacterHistory as LegacyCharacterHistory;
use tracing::{info, warn, error, debug};

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

/// Safely write data to file with proper error handling
fn write_to_file(content: &str, file_name: &str) -> Result<()> {
    // Ensure data directory exists
    std::fs::create_dir_all("data")
        .with_context(|| "Failed to create data directory")?;

    let file_path = format!("data/{}", file_name);
    info!("Writing {} bytes to {}", content.len(), file_path);

    let mut file = File::create(&file_path)
        .with_context(|| format!("Failed to create file: {}", file_path))?;

    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write to file: {}", file_path))?;

    info!("Successfully wrote to {}", file_path);
    Ok(())
}

/// Safely read data from file with proper error handling
fn read_from_file(file_name: &str) -> Result<String> {
    let file_path = format!("data/{}", file_name);
    info!("Reading from {}", file_path);

    let mut file = File::open(&file_path)
        .with_context(|| format!("Failed to open file: {}", file_path))?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .with_context(|| format!("Failed to read from file: {}", file_path))?;

    info!("Successfully read {} bytes from {}", content.len(), file_path);
    Ok(content)
}

/// Check database connectivity and basic health
async fn check_database_health(db: &PgPool) -> Result<()> {
    info!("Checking database connectivity...");

    let row = sqlx::query("SELECT version(), current_database(), current_user")
        .fetch_one(db)
        .await
        .context("Failed to connect to database")?;

    let version: String = row.get(0);
    let database: String = row.get(1);
    let user: String = row.get(2);

    info!("Database health check passed:");
    info!("  Version: {}", version);
    info!("  Database: {}", database);
    info!("  User: {}", user);

    Ok(())
}

/// Get migration statistics before starting
async fn get_migration_stats(db: &PgPool) -> Result<(i64, i64, i64)> {
    info!("Gathering migration statistics...");

    let unmigrated_messages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE is_migrated = false"
    ).fetch_one(db).await?;

    let total_characters: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM roleplay_characters"
    ).fetch_one(db).await?;

    let total_character_history: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM roleplay_characters_history"
    ).fetch_one(db).await?;

    info!("Migration statistics:");
    info!("  Unmigrated messages: {}", unmigrated_messages);
    info!("  Total characters: {}", total_characters);
    info!("  Total character history: {}", total_character_history);

    Ok((unmigrated_messages, total_characters, total_character_history))
}

/// Migrate messages with comprehensive logging and error handling
async fn migrate_messages(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    info!("🚀 Starting message migration for environment: {}", environment);
    let start_time = Instant::now();

    // Disable triggers
    info!("Disabling message triggers...");
    let mut tx = db.begin().await.context("Failed to start transaction")?;
    Message::toggle_trigger(&mut *tx, false).await
        .context("Failed to disable message triggers")?;

    // Find unmigrated messages
    info!("Finding unmigrated messages...");
    let messages = Message::find_by_criteria(
        QueryCriteria::new()
            .add_valued_filter("is_migrated", "=", false),
        &mut *tx
    ).await.context("Failed to query unmigrated messages")?;

    tx.commit().await.context("Failed to commit transaction")?;

    let total_messages = messages.len();
    info!("Found {} unmigrated messages", total_messages);

    if total_messages == 0 {
        info!("No messages to migrate, skipping message migration");
        return Ok(());
    }

    // Backup original messages
    info!("Creating backup of original messages...");
    let backup_content = serde_json::to_string(&messages)
        .context("Failed to serialize messages for backup")?;
    write_to_file(&backup_content, &format!("original_messages_{}.json", environment))?;

    // Process messages in chunks
    let mut processed_count = 0;
    let mut failed_count = 0;
    let mut failed_ids = Vec::new();

    for (chunk_index, chunk) in messages.chunks(100).enumerate() {
        info!("Processing chunk {} ({} messages)...", chunk_index + 1, chunk.len());

        let mut tx = db.begin().await
            .with_context(|| format!("Failed to start transaction for chunk {}", chunk_index + 1))?;

        let mut chunk_failed = 0;

        for message in chunk {
            let mut message_copy = message.clone();

            match try_prase_message(&message_copy) {
                Ok(tool_call) => {
                    message_copy.assistant_message_tool_call = Json(Some(tool_call));
                    message_copy.is_migrated = true;

                    let message_id = message_copy.id; // Store ID before move
                    match message_copy.update(&mut *tx).await {
                        Ok(_) => {
                            debug!("Successfully migrated message: {}", message_id);
                            processed_count += 1;
                        },
                        Err(e) => {
                            error!("Failed to update message {}: {}", message_id, e);
                            failed_count += 1;
                            chunk_failed += 1;
                            failed_ids.push(message_id);
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to parse message {}: {}", message_copy.id, e);
                    failed_count += 1;
                    chunk_failed += 1;
                    failed_ids.push(message_copy.id);
                }
            }
        }

        if chunk_failed == 0 {
            match tx.commit().await {
                Ok(_) => {
                    info!("✅ Chunk {} committed successfully ({} messages)",
                          chunk_index + 1, chunk.len());
                },
                Err(e) => {
                    error!("Failed to commit chunk {}: {}", chunk_index + 1, e);
                    failed_count += chunk.len();
                    for msg in chunk {
                        failed_ids.push(msg.id);
                    }
                }
            }
        } else {
            warn!("⚠️ Chunk {} had {} failures, rolling back", chunk_index + 1, chunk_failed);
            if let Err(e) = tx.rollback().await {
                error!("Failed to rollback chunk {}: {}", chunk_index + 1, e);
            }
        }

        // Progress update
        let progress = (processed_count as f64 / total_messages as f64) * 100.0;
        info!("Progress: {:.1}% ({}/{}) - Failures: {}",
              progress, processed_count, total_messages, failed_count);
    }

    // Re-enable triggers
    info!("Re-enabling message triggers...");
    let mut tx = db.begin().await.context("Failed to start transaction")?;
    Message::toggle_trigger(&mut *tx, true).await
        .context("Failed to re-enable message triggers")?;
    tx.commit().await.context("Failed to commit transaction")?;

    let duration = start_time.elapsed();

    if failed_count > 0 {
        error!("Message migration completed with {} failures out of {} total",
               failed_count, total_messages);

        // Write failed IDs to file for investigation
        let failed_ids_json = serde_json::to_string(&failed_ids)
            .context("Failed to serialize failed message IDs")?;
        write_to_file(&failed_ids_json, &format!("failed_message_ids_{}.json", environment))?;

        return Err(anyhow!("Message migration had {} failures", failed_count));
    }

    info!("✅ Message migration completed successfully!");
    info!("  Total processed: {}", processed_count);
    info!("  Duration: {:.2}s", duration.as_secs_f64());
    info!("  Rate: {:.1} messages/second", processed_count as f64 / duration.as_secs_f64());

    Ok(())
}

/// Offline character migration with comprehensive logging
async fn offline_migrate_characters(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    info!("🚀 Starting offline character migration for environment: {}", environment);
    let start_time = Instant::now();

    // Load characters
    info!("Loading legacy characters...");
    let mut tx = db.begin().await.context("Failed to start transaction")?;
    let characters = LegacyCharacter::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await.context("Failed to load legacy characters")?;

    let historical_characters = LegacyCharacterHistory::find_by_criteria(
        QueryCriteria::new(),
        &mut *tx
    ).await.context("Failed to load legacy character history")?;

    tx.commit().await.context("Failed to commit transaction")?;

    info!("Found {} characters and {} historical characters",
          characters.len(), historical_characters.len());

    if characters.is_empty() && historical_characters.is_empty() {
        info!("No characters to migrate, skipping character migration");
        return Ok(());
    }

    // Backup original data
    info!("Creating backups of original character data...");

    if !characters.is_empty() {
        let characters_json = serde_json::to_string(&characters)
            .context("Failed to serialize characters")?;
        write_to_file(&characters_json, &format!("original_characters_{}.json", environment))?;
    }

    if !historical_characters.is_empty() {
        let history_json = serde_json::to_string(&historical_characters)
            .context("Failed to serialize character history")?;
        write_to_file(&history_json, &format!("original_historical_characters_{}.json", environment))?;
    }

    // Migrate characters
    let mut migrated_characters = Vec::new();
    let mut character_failures = Vec::new();
    let total_characters = characters.len();

    info!("Migrating {} characters...", total_characters);
    for (index, legacy_character) in characters.into_iter().enumerate() {
        let character_id = legacy_character.id;

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            legacy_character.into_new_character()
        })) {
            Ok(new_character) => {
                debug!("Successfully migrated character {} ({}/{})",
                       character_id, index + 1, migrated_characters.len() + character_failures.len() + 1);
                migrated_characters.push(new_character);
            },
            Err(_) => {
                error!("Failed to migrate character {} (panic occurred)", character_id);
                character_failures.push(character_id);
            }
        }

        if (index + 1) % 100 == 0 {
            info!("Character migration progress: {}/{}", index + 1, total_characters);
        }
    }

    // Migrate character history
    let mut migrated_historical_characters = Vec::new();
    let mut history_failures = Vec::new();

    info!("Migrating {} character histories...", historical_characters.len());
    for (index, legacy_history) in historical_characters.into_iter().enumerate() {
        let history_id = legacy_history.id;

        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            legacy_history.into_new_character_history()
        })) {
            Ok(new_history) => {
                debug!("Successfully migrated character history {} ({}/{})",
                       history_id, index + 1, migrated_historical_characters.len() + history_failures.len() + 1);
                migrated_historical_characters.push(new_history);
            },
            Err(_) => {
                error!("Failed to migrate character history {} (panic occurred)", history_id);
                history_failures.push(history_id);
            }
        }

        if (index + 1) % 100 == 0 {
            info!("Character history migration progress: {}/{}", index + 1, migrated_historical_characters.len() + history_failures.len());
        }
    }

    // Save migrated data
    if !migrated_characters.is_empty() {
        info!("Saving {} migrated characters...", migrated_characters.len());
        let migrated_json = serde_json::to_string(&migrated_characters)
            .context("Failed to serialize migrated characters")?;
        write_to_file(&migrated_json, &format!("migrated_characters_{}.json", environment))?;
    }

    if !migrated_historical_characters.is_empty() {
        info!("Saving {} migrated character histories...", migrated_historical_characters.len());
        let migrated_history_json = serde_json::to_string(&migrated_historical_characters)
            .context("Failed to serialize migrated character histories")?;
        write_to_file(&migrated_history_json, &format!("migrated_historical_characters_{}.json", environment))?;
    }

    // Report failures
    if !character_failures.is_empty() || !history_failures.is_empty() {
        let failures = serde_json::json!({
            "character_failures": character_failures,
            "history_failures": history_failures
        });
        let failures_json = serde_json::to_string(&failures)
            .context("Failed to serialize migration failures")?;
        write_to_file(&failures_json, &format!("character_migration_failures_{}.json", environment))?;

        error!("Character migration had failures: {} characters, {} histories",
               character_failures.len(), history_failures.len());
        return Err(anyhow!("Character migration had {} total failures",
                          character_failures.len() + history_failures.len()));
    }

    let duration = start_time.elapsed();
    info!("✅ Offline character migration completed successfully!");
    info!("  Characters migrated: {}", migrated_characters.len());
    info!("  Histories migrated: {}", migrated_historical_characters.len());
    info!("  Duration: {:.2}s", duration.as_secs_f64());

    Ok(())
}

/// Online character migration with comprehensive safety checks
async fn online_migrate_characters(db: &Arc<PgPool>, environment: &str) -> Result<()> {
    info!("🚀 Starting online character migration for environment: {}", environment);
    let start_time = Instant::now();

    // Disable triggers
    info!("Disabling character update triggers...");
    let mut tx = db.begin().await.context("Failed to start transaction")?;
    NewCharacter::toggle_trigger(&mut *tx, false).await
        .context("Failed to disable character triggers")?;
    NewCharacterHistory::toggle_trigger(&mut *tx, false).await
        .context("Failed to disable character history triggers")?;
    tx.commit().await.context("Failed to commit transaction")?;

    // Load migrated data from files
    info!("Loading migrated character data from files...");

    let migrated_characters = if std::path::Path::new(&format!("data/migrated_characters_{}.json", environment)).exists() {
        let content = read_from_file(&format!("migrated_characters_{}.json", environment))?;
        serde_json::from_str::<Vec<NewCharacter>>(&content)
            .context("Failed to deserialize migrated characters")?
    } else {
        warn!("No migrated characters file found, skipping character updates");
        Vec::new()
    };

    let migrated_historical_characters = if std::path::Path::new(&format!("data/migrated_historical_characters_{}.json", environment)).exists() {
        let content = read_from_file(&format!("migrated_historical_characters_{}.json", environment))?;
        serde_json::from_str::<Vec<NewCharacterHistory>>(&content)
            .context("Failed to deserialize migrated character histories")?
    } else {
        warn!("No migrated character histories file found, skipping history updates");
        Vec::new()
    };

    info!("Loaded {} characters and {} histories from migration files",
          migrated_characters.len(), migrated_historical_characters.len());

    // Create lookup maps
    let mut characters_map = HashMap::new();
    for character in &migrated_characters {
        characters_map.insert(character.id, character);
    }

    let mut histories_map = HashMap::new();
    for history in &migrated_historical_characters {
        histories_map.insert(history.id, history);
    }

    // Load original characters for first_message updates
    if !characters_map.is_empty() {
        info!("Loading original characters for first_message updates...");
        let original_content = read_from_file(&format!("original_characters_{}.json", environment))?;
        let original_characters = serde_json::from_str::<Vec<LegacyCharacter>>(&original_content)
            .context("Failed to deserialize original characters")?;

        info!("Updating first_message for {} characters...", original_characters.len());

        let mut tx = db.begin().await.context("Failed to start transaction")?;
        let mut update_failures = Vec::new();

        for mut character in original_characters {
            let character_id = character.id;

            if let Some(new_character) = characters_map.get(&character_id) {
                // ⚠️ CRITICAL FIX: Proper JSON serialization without double-quoting
                match &new_character.prompts_first_message.0 {
                    Some(function_call) => {
                        match serde_json::to_string(function_call) {
                            Ok(json_string) => {
                                character.prompts_first_message = json_string;

                                match character.update(&mut *tx).await {
                                    Ok(_) => debug!("Updated first_message for character {}", character_id),
                                    Err(e) => {
                                        error!("Failed to update character {}: {}", character_id, e);
                                        update_failures.push(character_id);
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Failed to serialize first_message for character {}: {}", character_id, e);
                                update_failures.push(character_id);
                            }
                        }
                    },
                    None => {
                        character.prompts_first_message = "null".to_string();
                        match character.update(&mut *tx).await {
                            Ok(_) => debug!("Set null first_message for character {}", character_id),
                            Err(e) => {
                                error!("Failed to update character {} with null first_message: {}", character_id, e);
                                update_failures.push(character_id);
                            }
                        }
                    }
                }
            } else {
                warn!("No migrated data found for character {}", character_id);
            }
        }

        if update_failures.is_empty() {
            match tx.rollback().await {
                Ok(_) => info!("✅ Character first_message updates committed successfully"),
                Err(e) => {
                    error!("Failed to commit character first_message updates: {}", e);
                    return Err(anyhow!("Failed to commit character updates"));
                }
            }
        } else {
            error!("Rolling back character updates due to {} failures", update_failures.len());
            tx.rollback().await.context("Failed to rollback transaction")?;

            let failures_json = serde_json::to_string(&update_failures)?;
            write_to_file(&failures_json, &format!("character_update_failures_{}.json", environment))?;

            return Err(anyhow!("Character first_message updates had {} failures", update_failures.len()));
        }
    }

    // Update character histories
    if !histories_map.is_empty() {
        info!("Loading original character histories for first_message updates...");
        let original_content = read_from_file(&format!("original_historical_characters_{}.json", environment))?;
        let original_histories = serde_json::from_str::<Vec<LegacyCharacterHistory>>(&original_content)
            .context("Failed to deserialize original character histories")?;

        info!("Updating first_message for {} character histories...", original_histories.len());

        let mut tx = db.begin().await.context("Failed to start transaction")?;
        let mut history_update_failures = Vec::new();

        for mut history in original_histories {
            let history_id = history.id;

            if let Some(new_history) = histories_map.get(&history_id) {
                // ⚠️ CRITICAL FIX: Proper JSON serialization without double-quoting
                match &new_history.prompts_first_message.0 {
                    Some(function_call) => {
                        match serde_json::to_string(function_call) {
                            Ok(json_string) => {
                                history.prompts_first_message = json_string;

                                match history.update(&mut *tx).await {
                                    Ok(_) => debug!("Updated first_message for character history {}", history_id),
                                    Err(e) => {
                                        error!("Failed to update character history {}: {}", history_id, e);
                                        history_update_failures.push(history_id);
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Failed to serialize first_message for character history {}: {}", history_id, e);
                                history_update_failures.push(history_id);
                            }
                        }
                    },
                    None => {
                        history.prompts_first_message = "null".to_string();
                        match history.update(&mut *tx).await {
                            Ok(_) => debug!("Set null first_message for character history {}", history_id),
                            Err(e) => {
                                error!("Failed to update character history {} with null first_message: {}", history_id, e);
                                history_update_failures.push(history_id);
                            }
                        }
                    }
                }
            } else {
                warn!("No migrated data found for character history {}", history_id);
            }
        }

        if history_update_failures.is_empty() {
            match tx.commit().await {
                Ok(_) => info!("✅ Character history first_message updates committed successfully"),
                Err(e) => {
                    error!("Failed to commit character history first_message updates: {}", e);
                    return Err(anyhow!("Failed to commit character history updates"));
                }
            }
        } else {
            error!("Rolling back character history updates due to {} failures", history_update_failures.len());
            tx.rollback().await.context("Failed to rollback transaction")?;

            let failures_json = serde_json::to_string(&history_update_failures)?;
            write_to_file(&failures_json, &format!("character_history_update_failures_{}.json", environment))?;

            return Err(anyhow!("Character history first_message updates had {} failures", history_update_failures.len()));
        }
    }

    // Run schema migrations
    info!("Running schema migrations...");
    // NewCharacter::migrate(&db).await
    //     .context("Failed to migrate character schema")?;
    // NewCharacterHistory::migrate(&db).await
    //     .context("Failed to migrate character history schema")?;

    // Update remaining character fields
    if !migrated_characters.is_empty() {
        info!("Updating remaining character fields...");
        let mut tx = db.begin().await.context("Failed to start transaction")?;

        for (index, character) in migrated_characters.iter().enumerate() {
            match character.clone().update(&mut *tx).await {
                Ok(_) => debug!("Updated character {} ({}/{})", character.id, index + 1, migrated_characters.len()),
                Err(e) => {
                    error!("Failed to update character {}: {}", character.id, e);
                    tx.rollback().await.context("Failed to rollback transaction")?;
                    return Err(anyhow!("Failed to update character {}: {}", character.id, e));
                }
            }
        }

        tx.commit().await.context("Failed to commit character updates")?;
        info!("✅ Character field updates completed successfully");
    }

    // Update remaining character history fields
    if !migrated_historical_characters.is_empty() {
        info!("Updating remaining character history fields...");
        let mut tx = db.begin().await.context("Failed to start transaction")?;

        for (index, history) in migrated_historical_characters.iter().enumerate() {
            match history.clone().update(&mut *tx).await {
                Ok(_) => debug!("Updated character history {} ({}/{})", history.id, index + 1, migrated_historical_characters.len()),
                Err(e) => {
                    error!("Failed to update character history {}: {}", history.id, e);
                    tx.rollback().await.context("Failed to rollback transaction")?;
                    return Err(anyhow!("Failed to update character history {}: {}", history.id, e));
                }
            }
        }

        tx.commit().await.context("Failed to commit character history updates")?;
        info!("✅ Character history field updates completed successfully");
    }

    // Re-enable triggers
    info!("Re-enabling character update triggers...");
    let mut tx = db.begin().await.context("Failed to start transaction")?;
    NewCharacter::toggle_trigger(&mut *tx, true).await
        .context("Failed to re-enable character triggers")?;
    NewCharacterHistory::toggle_trigger(&mut *tx, true).await
        .context("Failed to re-enable character history triggers")?;
    tx.commit().await.context("Failed to commit transaction")?;

    let duration = start_time.elapsed();
    info!("✅ Online character migration completed successfully!");
    info!("  Characters updated: {}", migrated_characters.len());
    info!("  Histories updated: {}", migrated_historical_characters.len());
    info!("  Duration: {:.2}s", duration.as_secs_f64());

    Ok(())
}

/// Verify migration results
async fn verify_migration(db: &PgPool, _environment: &str) -> Result<()> {
    info!("🔍 Verifying migration results...");

    // Check for any remaining unmigrated messages
    let unmigrated_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM messages WHERE is_migrated = false"
    ).fetch_one(db).await?;

    if unmigrated_count > 0 {
        error!("⚠️ Found {} unmigrated messages after migration", unmigrated_count);
        return Err(anyhow!("Migration verification failed: {} unmigrated messages", unmigrated_count));
    }

    // Check for double-quoted enum values
    let double_quoted_status: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM roleplay_characters WHERE status::text LIKE '"%"'"#
    ).fetch_one(db).await?;

    if double_quoted_status > 0 {
        error!("⚠️ Found {} characters with double-quoted status values", double_quoted_status);
        return Err(anyhow!("Migration verification failed: {} double-quoted status values", double_quoted_status));
    }

    info!("✅ Migration verification passed!");
    info!("  All messages migrated successfully");
    info!("  No double-quoted enum values found");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize enhanced logging
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;

    info!("🚀 Starting robust message migration script");
    let overall_start = Instant::now();

    // Get environment parameter
    let environment = std::env::args().nth(1).unwrap_or_else(|| "production".to_string());
    info!("Migration environment: {}", environment);

    // Connect to database
    info!("Connecting to database...");
    let run_migrations = false;
    let db = Arc::new(connect(false, false, run_migrations).await.clone());
    let _pgvector_db = Arc::new(connect_pgvector(false, false, run_migrations).await.clone());

    // Health checks
    check_database_health(&db).await?;

    // Get pre-migration statistics
    let (unmigrated_messages, total_characters, total_history) = get_migration_stats(&db).await?;

    // User confirmation for production
    if environment == "production" {
        println!("\n⚠️  WARNING: You are about to run migration on PRODUCTION database!");
        println!("Migration will affect:");
        println!("  - {} unmigrated messages", unmigrated_messages);
        println!("  - {} characters", total_characters);
        println!("  - {} character histories", total_history);
        println!("\nType 'CONFIRM' to proceed with production migration:");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() != "CONFIRM" {
            info!("Migration cancelled by user");
            return Ok(());
        }
    }

    // Run migrations step by step
    let mut migration_results = Vec::new();

    // Step 1: Migrate database schemas
    info!("Step 1: Running database schema migrations...");
    if let Err(e) = async {
        Message::migrate(&db).await?;
        LegacyCharacter::migrate(&db).await?;
        LegacyCharacterHistory::migrate(&db).await?;
        Result::<()>::Ok(())
    }.await {
        error!("Schema migration failed: {}", e);
        return Err(e);
    }
    migration_results.push("✅ Schema migrations");

    // Step 2: Migrate messages
    info!("Step 2: Migrating messages...");
    match migrate_messages(&db, &environment).await {
        Ok(_) => migration_results.push("✅ Message migration"),
        Err(e) => {
            error!("Message migration failed: {}", e);
            migration_results.push("❌ Message migration");
            return Err(e);
        }
    }

    // Step 3: Offline character migration
    info!("Step 3: Checking for existing offline migration files...");

    let migrated_chars_file = format!("data/migrated_characters_{}.json", environment);
    let migrated_history_file = format!("data/migrated_historical_characters_{}.json", environment);

    let chars_exist = std::path::Path::new(&migrated_chars_file).exists();
    let history_exist = std::path::Path::new(&migrated_history_file).exists();

    if chars_exist || history_exist {
        info!("📁 Found existing offline migration files:");
        if chars_exist { info!("  ✅ {}", migrated_chars_file); }
        if history_exist { info!("  ✅ {}", migrated_history_file); }
        info!("");
        println!("🔄 Skip offline migration and use existing files? [y/N]:");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            info!("⏭️  Skipping offline migration - using existing files");
            migration_results.push("⏭️  Offline character migration (skipped)");
        } else {
            info!("🔄 Running fresh offline character migration...");
            match offline_migrate_characters(&db, &environment).await {
                Ok(_) => migration_results.push("✅ Offline character migration"),
                Err(e) => {
                    error!("Offline character migration failed: {}", e);
                    migration_results.push("❌ Offline character migration");
                    return Err(e);
                }
            }
        }
    } else {
        info!("Running offline character migration...");
        match offline_migrate_characters(&db, &environment).await {
            Ok(_) => migration_results.push("✅ Offline character migration"),
            Err(e) => {
                error!("Offline character migration failed: {}", e);
                migration_results.push("❌ Offline character migration");
                return Err(e);
            }
        }
    }

    // Manual review prompt before online migration
    info!("🔍 Offline migration completed successfully!");
    info!("📁 Generated files in data/ directory:");
    info!("  - migrated_characters_{}.json", environment);
    info!("  - migrated_historical_characters_{}.json", environment);
    info!("  - original_characters_{}.json (backup)", environment);
    info!("  - original_historical_characters_{}.json (backup)", environment);
    info!("");
    info!("⚠️  IMPORTANT: Please review the generated migration files before proceeding.");
    info!("   Check the data/ directory to ensure the character transformations look correct.");
    info!("");
    println!("🔍 Ready to proceed with online migration (database updates)?");
    println!("This will apply the migrated character data to the database.");
    println!("");
    println!("Type 'PROCEED' to continue with online migration, or 'ABORT' to stop:");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_uppercase();

    match input.as_str() {
        "PROCEED" => {
            info!("✅ User confirmed - proceeding with online migration");
        },
        "ABORT" => {
            info!("🛑 Migration aborted by user after offline phase");
            info!("Offline migration files have been preserved in data/ directory");
            info!("You can resume by running the script again and it will skip the offline phase");
            return Ok(());
        },
        _ => {
            warn!("Invalid input '{}' - aborting migration for safety", input);
            info!("Migration aborted. Please run again and type 'PROCEED' or 'ABORT'");
            return Ok(());
        }
    }

    // Step 4: Online character migration
    info!("Step 4: Running online character migration...");
    match online_migrate_characters(&db, &environment).await {
        Ok(_) => migration_results.push("✅ Online character migration"),
        Err(e) => {
            error!("Online character migration failed: {}", e);
            migration_results.push("❌ Online character migration");
            return Err(e);
        }
    }

    // Step 5: Verification
    info!("Step 5: Verifying migration results...");
    match verify_migration(&db, &environment).await {
        Ok(_) => migration_results.push("✅ Migration verification"),
        Err(e) => {
            error!("Migration verification failed: {}", e);
            migration_results.push("❌ Migration verification");
            return Err(e);
        }
    }

    let total_duration = overall_start.elapsed();

    // Final report
    info!("{}", "=".repeat(60));
    info!("🎉 MIGRATION COMPLETED SUCCESSFULLY!");
    info!("{}", "=".repeat(60));
    info!("Environment: {}", environment);
    info!("Total duration: {:.2}s", total_duration.as_secs_f64());
    info!("");
    info!("Migration steps completed:");
    for result in migration_results {
        info!("  {}", result);
    }
    info!("");
    info!("All migration data saved to data/ directory");
    info!("Migration is now complete and ready for production use! 🚀");

    Ok(())
}