

/// Initializes all necessary database connection pools for the application.
///
/// This macro serves as the single point of entry for setting up database connections.
/// It creates and configures a default pool and a separate pool for pgvector,
/// and ensures that tables for the specified types are created in the correct database.
///
/// # Arguments
/// - `default: [$($default_type:ty),*]`: A comma-separated list of types for the default database.
/// - `pgvector: [$($pgvector_type:ty),*]`: A comma-separated list of types for the pgvector database.
///
/// # Generated Functions
/// - `async fn connect(drop_tables: bool, create_tables: bool) -> &'static PgPool`: Connects to the default database.
/// - `async fn connect_pgvector(drop_tables: bool, create_tables: bool) -> &'static PgPool`: Connects to the pgvector database.
///
/// # Example
/// ```rust,ignore
/// // Assume User, Post, and Embedding implement SqlxSchema
/// init_databases!(
///     default: [User, Post],
///     pgvector: [Embedding]
/// );
///
/// #[tokio::main]
/// async fn main() {
///     let default_pool = connect(false, true).await;
///     let pgvector_pool = connect_pgvector(false, true).await;
///     // ... use pools
/// }
/// ```
#[macro_export]
macro_rules! init_databases {
    (
        default: [$($default_type:ty),* $(,)?],
        pgvector: [$($pgvector_type:ty),* $(,)?]
    ) => {
        use $crate::{SqlxSchema, SchemaMigrator};

        // --- Default Pool Setup ---
        static POOL: tokio::sync::OnceCell<sqlx::PgPool> = tokio::sync::OnceCell::const_new();

        async fn connect(drop_tables: bool, create_tables: bool, run_migrations: bool) -> &'static sqlx::PgPool {
            POOL.get_or_init(|| async {
                let database_url = std::env::var("DATABASE_URL")
                    .expect("DATABASE_URL environment variable not set");
                
                let pool = sqlx::PgPool::connect(&database_url).await
                    .expect("Failed to connect to default database");

                if drop_tables {
                    $( 
                        let drop_table_sql_str = <$default_type as $crate::SqlxSchema>::drop_table_sql();
                        if !drop_table_sql_str.trim().is_empty() { 
                            sqlx::query(&drop_table_sql_str).execute(&pool).await
                                .unwrap_or_else(|e| {
                                    eprintln!("Warning: Failed to drop table for '{}'. Error: {:?}", stringify!($default_type), e);
                                    sqlx::postgres::PgQueryResult::default()
                                });
                        }
                    )*
                }

                if create_tables {
                    let trigger_func_sql = r#"
                    CREATE OR REPLACE FUNCTION set_updated_at_unix_timestamp()
                    RETURNS TRIGGER AS $$
                    BEGIN NEW.updated_at = floor(extract(epoch from now())); RETURN NEW; END;
                    $$ language 'plpgsql';
                    "#;
                    sqlx::query(trigger_func_sql).execute(&pool).await
                        .expect("Failed to create timestamp helper function.");

                    $( 
                        let create_table_sql_str = <$default_type as $crate::SqlxSchema>::create_table_sql();
                        if !create_table_sql_str.trim().is_empty() {
                            sqlx::query(&create_table_sql_str).execute(&pool).await
                                .unwrap_or_else(|e| panic!("Failed to create table for '{}'. Error: {:?}", stringify!($default_type), e));
                        }
                    )*

                    $( 
                        let trigger_sql_str = <$default_type as $crate::SqlxSchema>::trigger_sql();
                        if !trigger_sql_str.trim().is_empty() {
                            for statement in trigger_sql_str.split(';').filter(|s| !s.trim().is_empty()) {
                                sqlx::query(statement).execute(&pool).await
                                    .unwrap_or_else(|e| panic!("Failed to execute trigger for '{}'. SQL: {}. Error: {:?}", stringify!($default_type), statement, e));
                            }
                        }
                    )*

                    $(
                        for index_sql in <$default_type as $crate::SqlxSchema>::INDEXES_SQL {
                            sqlx::query(index_sql).execute(&pool).await
                                .unwrap_or_else(|e| panic!("Failed to create index for '{}'. SQL: {}. Error: {:?}", stringify!($default_type), index_sql, e));
                        }
                    )*
                }

                if run_migrations {
                    $(
                        if let Err(e) = <$default_type as SchemaMigrator>::migrate(&pool).await {
                            eprintln!("[MIGRATE][ERROR] Failed to migrate '{}'. Error: {:?}", stringify!($default_type), e);
                        }
                    )*
                }

                pool
            }).await
        }

        // --- Pgvector Pool Setup ---
        static PGVECTOR_POOL: tokio::sync::OnceCell<sqlx::PgPool> = tokio::sync::OnceCell::const_new();

        async fn connect_pgvector(drop_tables: bool, create_tables: bool, run_migrations: bool) -> &'static sqlx::PgPool {
            PGVECTOR_POOL.get_or_init(|| async {
                let database_url = std::env::var("PGVECTOR_URI")
                    .expect("PGVECTOR_URI environment variable not set");
                
                let pool = sqlx::PgPool::connect(&database_url).await
                    .expect("Failed to connect to pgvector database");

                sqlx::query("CREATE EXTENSION IF NOT EXISTS vector").execute(&pool).await
                    .expect("Failed to create vector extension.");

                if drop_tables {
                    $( 
                        let drop_table_sql_str = <$pgvector_type as $crate::SqlxSchema>::drop_table_sql();
                        if !drop_table_sql_str.trim().is_empty() { 
                            sqlx::query(&drop_table_sql_str).execute(&pool).await
                                .unwrap_or_else(|e| {
                                    eprintln!("Warning: Failed to drop table for '{}'. Error: {:?}", stringify!($pgvector_type), e);
                                    sqlx::postgres::PgQueryResult::default()
                                });
                        }
                    )*
                }

                if create_tables {
                     let trigger_func_sql = r#"
                    CREATE OR REPLACE FUNCTION set_updated_at_unix_timestamp()
                    RETURNS TRIGGER AS $$
                    BEGIN NEW.updated_at = floor(extract(epoch from now())); RETURN NEW; END;
                    $$ language 'plpgsql';
                    "#;
                    sqlx::query(trigger_func_sql).execute(&pool).await
                        .expect("Failed to create timestamp helper function.");
                        
                    $( 
                        let create_table_sql_str = <$pgvector_type as $crate::SqlxSchema>::create_table_sql();
                        if !create_table_sql_str.trim().is_empty() {
                            sqlx::query(&create_table_sql_str).execute(&pool).await
                                .unwrap_or_else(|e| panic!("Failed to create table for '{}'. Error: {:?}", stringify!($pgvector_type), e));
                        }
                    )*

                    $( 
                        let trigger_sql_str = <$pgvector_type as $crate::SqlxSchema>::trigger_sql();
                        if !trigger_sql_str.trim().is_empty() {
                            for statement in trigger_sql_str.split(';').filter(|s| !s.trim().is_empty()) {
                                sqlx::query(statement).execute(&pool).await
                                    .unwrap_or_else(|e| panic!("Failed to execute trigger for '{}'. SQL: {}. Error: {:?}", stringify!($pgvector_type), statement, e));
                            }
                        }
                    )*

                    $(
                        for index_sql in <$pgvector_type as $crate::SqlxSchema>::INDEXES_SQL {
                            sqlx::query(index_sql).execute(&pool).await
                                .unwrap_or_else(|e| panic!("Failed to create index for '{}'. SQL: {}. Error: {:?}", stringify!($pgvector_type), index_sql, e));
                        }
                    )*
                }

                if run_migrations {
                    $(
                        if let Err(e) = <$pgvector_type as SchemaMigrator>::migrate(&pool).await {
                            eprintln!("[MIGRATE][ERROR] Failed to migrate '{}'. Error: {:?}", stringify!($pgvector_type), e);
                        }
                    )*
                }

                pool
            }).await
        }
    };
}
