use std::sync::Arc;

use anyhow::Result;
use metastable_database::init_databases;

init_databases!(
    default: [],
    pgvector: []
);

#[tokio::main]
async fn main() -> Result<()> {
    let db = Arc::new(connect(false, false).await.clone());
    let pgvector_db = Arc::new(connect_pgvector(false, false).await.clone());
    // Remove all tables, listed or not listed
    // This will drop all tables in the current schema (Postgres)
    sqlx::query(
        r#"
        DO $$
        DECLARE
            r RECORD;
        BEGIN
            -- Drop all tables in the current schema
            FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = current_schema()) LOOP
                EXECUTE 'DROP TABLE IF EXISTS "' || r.tablename || '" CASCADE';
            END LOOP;
        END $$;
        "#,
    )
    .execute(&*db)
    .await?;

    sqlx::query(
        r#"
        DO $$
        DECLARE
            r RECORD;
        BEGIN
            -- Drop all tables in the current schema
            FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = current_schema()) LOOP
                EXECUTE 'DROP TABLE IF EXISTS "' || r.tablename || '" CASCADE';
            END LOOP;
        END $$;
        "#,
    )
    .execute(&*pgvector_db)
    .await?;

    println!("Database reset successfully");
    Ok(())
} 