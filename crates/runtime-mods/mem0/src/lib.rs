mod pgvector;
mod graph;

mod llm;

mod engine;
mod env;
mod raw_message;
mod message;

mod memory;

pub use engine::Mem0Engine;
pub use raw_message::{EmbeddingMessage, GraphEntities, EntityTag};
pub use message::Mem0Messages;

pub type Embedding = Vec<f32>;
pub const EMBEDDING_DIMS: i32 = 1024;
pub const EMBEDDING_MODEL: &str = "Qwen/Qwen3-Embedding-0.6B";

pub const DEFAULT_VECTOR_DB_SEARCH_LIMIT: usize = 100;
pub const DEFAULT_GRAPH_DB_SEARCH_LIMIT: usize = 100;

/// used for merge similar items in the graph db
pub const DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD: f32 = 0.9;
/// used for general search in the graph db 
pub const DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD: f32 = 0.7;

#[macro_export]
macro_rules! init_pgvector_pool {
    () => {
        static PGVECTOR_POOL: tokio::sync::OnceCell<sqlx::PgPool> = tokio::sync::OnceCell::const_new();

        // To use this macro, call it like this:
        // init_pgvector_pool!(MyType1, MyType2, MyType3);
        // 
        // It defines an async function with signature:
        // async fn connect() -> &'static PgPool
        // which will connect to the database and ensure tables for MyType1, MyType2, MyType3 are created.
        //
        // Example usage:
        // let pool = connect().await;
        async fn connect_pgvector(drop_tables: bool, create_tables: bool) -> &'static sqlx::PgPool {
            PGVECTOR_POOL.get_or_init(|| async {
                let database_url = std::env::var("PGVECTOR_URI").unwrap();
                
                let pool = sqlx::PgPool::connect(&database_url).await
                    .expect("Failed to connect to Postgres. Ensure DB is running and PGVECTOR_URI is correct.");

                // Drop tables first to ensure a clean schema for tests
                if drop_tables {
                    sqlx::query("DROP TABLE IF EXISTS embeddings")
                        .execute(&pool)
                        .await
                        .unwrap_or_else(|e| {
                            // Log a warning instead of panic for drop errors, as table might not exist
                            eprintln!(
                                "Warning: Failed to drop table for type '{}'. SQL: \"{}\". Error: {:?}. This might be okay if the table didn't exist.", 
                                "embeddings", 
                                "DROP TABLE IF EXISTS embeddings", 
                                e
                            );
                            // Return a default/dummy ExecutionResult or similar if needed, though for unwrap_or_else it expects the same type as Ok variant.
                            // Since we are not using the result of drop, just logging is fine here.
                            sqlx::postgres::PgQueryResult::default() // Provide a dummy result to satisfy unwrap_or_else type if it were strictly needed, but simple unwrap_or_else with eprintln is fine.
                        });
                }

                // Create tables for each specified type
                if create_tables {
                    let create_extension_sql = "CREATE EXTENSION IF NOT EXISTS vector;";
                    let create_table_sql = r#"
                    CREATE TABLE IF NOT EXISTS embeddings (
                        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                        user_id UUID NOT NULL,
                        agent_id UUID,
                        content TEXT NOT NULL,
                        embedding vector(1024),
                        created_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now())),
                        updated_at BIGINT NOT NULL DEFAULT floor(extract(epoch from now()))
                    );
                    "#;

                    let create_index_sql = r#"
                    CREATE INDEX IF NOT EXISTS idx_embeddings_user_id ON embeddings(user_id);
                    "#;

                    let mut tx = pool.begin().await.expect("Failed to begin transaction");
                    sqlx::query(create_extension_sql)
                        .execute(&mut *tx)
                        .await
                        .expect("Failed to create embeddings table.");

                    sqlx::query(create_table_sql)
                        .execute(&mut *tx)
                        .await
                        .expect("Failed to create embeddings table.");

                    sqlx::query(create_index_sql)
                        .execute(&mut *tx)
                        .await
                        .expect("Failed to create embeddings index.");
                        
                    tx.commit().await.expect("Failed to commit transaction");}

                pool
            }).await
        }
    };
}
