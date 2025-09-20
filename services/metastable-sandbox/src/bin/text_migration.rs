use anyhow::Result;
use sqlx::{PgPool, Row};
use tracing::{info, warn, error};

/// Migration script to convert TEXT columns storing enum values to JSONB
/// This fixes the double-quoting issue where enum values were stored as "\"Published\""
/// instead of "Published" due to a mismatch between SQL type (TEXT) and SQLx encoding (JSONB)

#[derive(Debug, Clone)]
struct ColumnMigration {
    table_name: &'static str,
    column_name: &'static str,
    enum_type: &'static str,
}

impl ColumnMigration {
    const fn new(table_name: &'static str, column_name: &'static str, enum_type: &'static str) -> Self {
        Self { table_name, column_name, enum_type }
    }
}

// All columns that need migration from TEXT to JSONB
const MIGRATIONS: &[ColumnMigration] = &[
    // Character table enums
    ColumnMigration::new("roleplay_characters", "status", "CharacterStatus"),
    ColumnMigration::new("roleplay_characters", "gender", "CharacterGender"),
    ColumnMigration::new("roleplay_characters", "orientation", "CharacterOrientation"),
    ColumnMigration::new("roleplay_characters", "language", "CharacterLanguage"),

    // Character history table enums (correct table name)
    ColumnMigration::new("roleplay_characters_history", "status", "CharacterStatus"),
    ColumnMigration::new("roleplay_characters_history", "gender", "CharacterGender"),
    ColumnMigration::new("roleplay_characters_history", "language", "CharacterLanguage"),

    // Audit log table enums (correct table name)
    ColumnMigration::new("roleplay_character_audit_logs", "previous_status", "CharacterStatus"),
    ColumnMigration::new("roleplay_character_audit_logs", "new_status", "CharacterStatus"),

    // User table enums
    ColumnMigration::new("users", "role", "UserRole"),

    // User notification table enums
    ColumnMigration::new("user_notifications", "notification_type", "NotificationType"),

    // User payment table enums
    ColumnMigration::new("user_payments", "status", "UserPaymentStatus"),

    // User points log table enums
    ColumnMigration::new("user_points_logs", "add_reason", "UserPointsLogAddReason"),
    ColumnMigration::new("user_points_logs", "deduct_reason", "UserPointsLogDeductReason"),
    ColumnMigration::new("user_points_logs", "reward_reason", "UserPointsLogRewardReason"),

    // Message table enums
    ColumnMigration::new("messages", "user_message_content_type", "MessageType"),
    ColumnMigration::new("messages", "assistant_message_content_type", "MessageType"),
];

async fn check_column_type(pool: &PgPool, table: &str, column: &str) -> Result<String> {
    let query = r#"
        SELECT data_type
        FROM information_schema.columns
        WHERE table_name = $1 AND column_name = $2
    "#;

    let row = sqlx::query(query)
        .bind(table)
        .bind(column)
        .fetch_optional(pool)
        .await?;

    if let Some(row) = row {
        Ok(row.get::<String, _>("data_type"))
    } else {
        Err(anyhow::anyhow!("Column {}.{} not found", table, column))
    }
}

async fn clean_double_quoted_values(pool: &PgPool, table: &str, column: &str) -> Result<u64> {
    // First, count how many rows need cleaning
    let count_query = format!(
        r#"SELECT COUNT(*) as count FROM "{}" WHERE "{}" LIKE '"%"'"#,
        table, column
    );

    let count: i64 = sqlx::query(&count_query)
        .fetch_one(pool)
        .await?
        .get("count");

    if count > 0 {
        info!("Found {} rows with double-quoted values in {}.{}", count, table, column);

        // Clean the double-quoted values
        let update_query = format!(
            r#"UPDATE "{}" SET "{}" = TRIM(BOTH '"' FROM "{}") WHERE "{}" LIKE '"%"'"#,
            table, column, column, column
        );

        let result = sqlx::query(&update_query)
            .execute(pool)
            .await?;

        info!("Cleaned {} rows in {}.{}", result.rows_affected(), table, column);
        Ok(result.rows_affected())
    } else {
        info!("No double-quoted values found in {}.{}", table, column);
        Ok(0)
    }
}

async fn migrate_column_to_jsonb(pool: &PgPool, migration: &ColumnMigration) -> Result<()> {
    let table = migration.table_name;
    let column = migration.column_name;
    let enum_type = migration.enum_type;

    info!("Starting migration for {}.{} (type: {})", table, column, enum_type);

    // Check if column exists and get its current type
    let current_type = match check_column_type(pool, table, column).await {
        Ok(t) => t,
        Err(e) => {
            warn!("Skipping {}.{}: {}", table, column, e);
            return Ok(());
        }
    };

    info!("Current type for {}.{}: {}", table, column, current_type);

    // Skip if already JSONB
    if current_type.to_lowercase() == "jsonb" {
        info!("{}.{} is already JSONB, skipping migration", table, column);
        return Ok(());
    }

    // If it's TEXT, we need to migrate
    if current_type.to_lowercase() == "text" {
        // First clean any double-quoted values
        let cleaned = clean_double_quoted_values(pool, table, column).await?;
        info!("Cleaned {} rows before migration", cleaned);

        // Create a transaction for the migration
        let mut tx = pool.begin().await?;

        // Step 1: Add temporary JSONB column
        let temp_column = format!("{}_jsonb_temp", column);
        let add_temp_query = format!(
            r#"ALTER TABLE "{}" ADD COLUMN IF NOT EXISTS "{}" JSONB"#,
            table, temp_column
        );
        sqlx::query(&add_temp_query).execute(&mut *tx).await?;
        info!("Added temporary column {}", temp_column);

        // Step 2: Copy and convert data to JSONB
        // We need to handle both simple strings and already JSON-encoded values
        let copy_query = format!(
            r#"
            UPDATE "{}"
            SET "{}" =
                CASE
                    WHEN "{}" IS NULL THEN NULL
                    WHEN "{}" = '' THEN NULL
                    WHEN LEFT("{}", 1) = '{{' THEN "{}"::jsonb
                    ELSE to_jsonb("{}")
                END
            "#,
            table, temp_column, column, column, column, column, column
        );
        sqlx::query(&copy_query).execute(&mut *tx).await?;
        info!("Copied data to temporary column");

        // Step 3: Drop the original column
        let drop_query = format!(r#"ALTER TABLE "{}" DROP COLUMN "{}""#, table, column);
        sqlx::query(&drop_query).execute(&mut *tx).await?;
        info!("Dropped original column");

        // Step 4: Rename temporary column to original name
        let rename_query = format!(
            r#"ALTER TABLE "{}" RENAME COLUMN "{}" TO "{}""#,
            table, temp_column, column
        );
        sqlx::query(&rename_query).execute(&mut *tx).await?;
        info!("Renamed temporary column to original name");

        // Commit the transaction
        tx.commit().await?;
        info!("Successfully migrated {}.{} from TEXT to JSONB", table, column);
    } else {
        warn!("{}.{} has unexpected type '{}', skipping", table, column, current_type);
    }

    Ok(())
}

async fn verify_migration(pool: &PgPool, migration: &ColumnMigration) -> Result<()> {
    let table = migration.table_name;
    let column = migration.column_name;

    // Check the new column type
    let current_type = check_column_type(pool, table, column).await?;

    if current_type.to_lowercase() != "jsonb" {
        error!("Migration verification failed: {}.{} is {} instead of JSONB",
               table, column, current_type);
        return Err(anyhow::anyhow!("Column type is not JSONB after migration"));
    }

    // Check for any remaining double-quoted values
    let check_query = format!(
        r#"SELECT COUNT(*) as count FROM "{}" WHERE "{}"::text LIKE '"%"'"#,
        table, column
    );

    let count: i64 = sqlx::query(&check_query)
        .fetch_one(pool)
        .await?
        .get("count");

    if count > 0 {
        warn!("Found {} rows that still appear to have quotes in {}.{}", count, table, column);
    } else {
        info!("✓ {}.{} successfully migrated and verified", table, column);
    }

    Ok(())
}

async fn create_backup_table(pool: &PgPool, table_name: &str) -> Result<()> {
    // First check if the table exists
    let table_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_name = $1
        )
        "#
    )
    .bind(table_name)
    .fetch_one(pool)
    .await?;

    if !table_exists {
        info!("Table '{}' does not exist, skipping backup", table_name);
        return Ok(());
    }

    let backup_name = format!("{}_backup_{}", table_name, chrono::Utc::now().timestamp());
    let query = format!(
        r#"CREATE TABLE "{}" AS SELECT * FROM "{}""#,
        backup_name, table_name
    );

    sqlx::query(&query).execute(pool).await?;
    info!("Created backup table: {}", backup_name);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/metastable".to_string());

    info!("Connecting to database...");
    let pool = PgPool::connect(&database_url).await?;
    info!("Connected to database");

    // Collect unique table names for backup
    let mut tables_to_backup = std::collections::HashSet::new();
    for migration in MIGRATIONS {
        tables_to_backup.insert(migration.table_name);
    }

    // Ask for confirmation before proceeding
    println!("\n⚠️  WARNING: This migration will modify the following tables:");
    for table in &tables_to_backup {
        println!("  - {}", table);
    }
    println!("\nThe following columns will be migrated from TEXT to JSONB:");
    for migration in MIGRATIONS {
        println!("  - {}.{} ({})", migration.table_name, migration.column_name, migration.enum_type);
    }

    println!("\nDo you want to create backups of these tables? (recommended) [y/N]: ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        info!("Creating backup tables...");
        for table in &tables_to_backup {
            match create_backup_table(&pool, table).await {
                Ok(_) => info!("✓ Backed up {}", table),
                Err(e) => {
                    error!("Failed to backup {}: {}", table, e);
                    println!("\nBackup failed. Continue anyway? [y/N]: ");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if input.trim().to_lowercase() != "y" {
                        return Err(anyhow::anyhow!("Migration aborted"));
                    }
                }
            }
        }
    }

    println!("\nProceed with migration? This will alter the database schema. [y/N]: ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        info!("Migration cancelled by user");
        return Ok(());
    }

    info!("Starting migrations...");
    let mut success_count = 0;
    let mut fail_count = 0;

    for migration in MIGRATIONS {
        match migrate_column_to_jsonb(&pool, migration).await {
            Ok(_) => {
                match verify_migration(&pool, migration).await {
                    Ok(_) => success_count += 1,
                    Err(e) => {
                        error!("Verification failed for {}.{}: {}",
                               migration.table_name, migration.column_name, e);
                        fail_count += 1;
                    }
                }
            }
            Err(e) => {
                error!("Migration failed for {}.{}: {}",
                       migration.table_name, migration.column_name, e);
                fail_count += 1;
            }
        }
    }

    info!("{}", "=".repeat(60));
    info!("Migration complete!");
    info!("  Successful: {}", success_count);
    info!("  Failed: {}", fail_count);
    info!("  Total: {}", MIGRATIONS.len());

    if fail_count > 0 {
        error!("Some migrations failed. Please check the logs above.");
        return Err(anyhow::anyhow!("{} migrations failed", fail_count));
    } else {
        info!("All migrations completed successfully! 🎉");
        info!("\nIMPORTANT: Please restart your application to use the new schema.");
    }

    Ok(())
}