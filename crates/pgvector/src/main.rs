use std::sync::Arc;
use sqlx::{PgPool, postgres::PgPoolOptions, query};
use uuid::Uuid;
use futures::future::join_all;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("PGVECTOR_URI")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost:5432/pgvector".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    println!("Connected to PostgreSQL database");

    init_database(&pool).await?;

    test_concurrent_operations(&pool).await?;

    println!("All tests completed successfully!");
    Ok(())
}

async fn init_database(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(pool)
        .await?;

    query(r#"
        CREATE TABLE IF NOT EXISTS test_embeddings (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            content TEXT NOT NULL,
            embedding vector(1024),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    "#)
    .execute(pool)
    .await?;

    query("CREATE INDEX IF NOT EXISTS idx_test_embeddings_embedding ON test_embeddings USING ivfflat (embedding vector_cosine_ops)")
        .execute(pool)
        .await?;

    println!("Database initialized successfully");
    Ok(())
}

async fn test_concurrent_operations(pool: &PgPool) -> Result<(), Box<dyn std::error::Error>> {
    let id = Uuid::new_v4();
    
    query("INSERT INTO test_embeddings (id, content, embedding) VALUES ($1, $2, $3)")
        .bind(id)
        .bind("Test content")
        .bind(vec![0.1f32; 1024]) // 简单的测试向量
        .execute(pool)
        .await?;

    println!("Created record with id: {}", id);

    let mut handles = Vec::new();
    let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    
    for _ in 1..=10 {
        let pool = pool.clone();
        let id = id;
        let count = count.clone();
        
        let handle = tokio::spawn(async move {
            let result = query("SELECT content FROM test_embeddings WHERE id = $1")
                .bind(id)
                .fetch_one(&pool)
                .await;
                
            if result.is_ok() {
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        });
        
        handles.push(handle);
    }

    join_all(handles).await;
    
    let final_count = count.load(std::sync::atomic::Ordering::Relaxed);
    println!("Concurrent queries completed. Count: {}", final_count);
    
    assert_eq!(final_count, 10);
    
    let search_vector = vec![0.1f32; 1024];
    let result = query("SELECT content FROM test_embeddings ORDER BY embedding <=> $1 LIMIT 1")
        .bind(&search_vector)
        .fetch_one(pool)
        .await?;
    
    println!("Vector similarity search result: {:?}", result);
    
    Ok(())
}