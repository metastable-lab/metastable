use anyhow::Result;
use redis::{Client, AsyncCommands, aio::ConnectionManager};
use serde::{Serialize, de::DeserializeOwned};
use tracing::{info, warn, error};

#[derive(Clone)]
pub struct RedisClient {
    manager: ConnectionManager,
}

impl RedisClient {
    pub async fn setup_connection() -> Self {
        let redis_url = std::env::var("REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

        info!("Connecting to Redis at {}", redis_url);

        let client = Client::open(redis_url)
            .expect("Failed to create Redis client");

        let manager = ConnectionManager::new(client)
            .await
            .expect("Failed to create Redis connection manager");

        info!("Redis connection established");

        Self {
            manager,
        }
    }

    // Generic get with deserialization
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let mut conn = self.manager.clone();

        match conn.get::<&str, String>(key).await {
            Ok(data) => {
                match serde_json::from_str(&data) {
                    Ok(value) => Some(value),
                    Err(e) => {
                        warn!("Failed to deserialize cache key {}: {}", key, e);
                        None
                    }
                }
            }
            Err(e) => {
                if !e.to_string().contains("nil") {
                    warn!("Redis GET error for key {}: {}", key, e);
                }
                None
            }
        }
    }

    // Generic set with serialization and TTL
    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl_secs: u64) -> Result<()> {
        let mut conn = self.manager.clone();

        let json_data = serde_json::to_string(value)?;

        conn.set_ex::<&str, String, ()>(key, json_data, ttl_secs)
            .await
            .map_err(|e| {
                error!("Redis SET error for key {}: {}", key, e);
                anyhow::anyhow!("Redis SET failed: {}", e)
            })?;

        Ok(())
    }

    // Delete a key (for invalidation)
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut conn = self.manager.clone();

        conn.del::<&str, ()>(key)
            .await
            .map_err(|e| {
                error!("Redis DEL error for key {}: {}", key, e);
                anyhow::anyhow!("Redis DEL failed: {}", e)
            })?;

        Ok(())
    }

    // Delete multiple keys matching a pattern
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u32> {
        let mut conn = self.manager.clone();

        // Get all keys matching pattern
        let keys: Vec<String> = conn.keys(pattern)
            .await
            .map_err(|e| anyhow::anyhow!("Redis KEYS failed: {}", e))?;

        if keys.is_empty() {
            return Ok(0);
        }

        let count = keys.len() as u32;

        // Delete all matching keys
        conn.del::<Vec<String>, ()>(keys)
            .await
            .map_err(|e| anyhow::anyhow!("Redis DEL failed: {}", e))?;

        Ok(count)
    }

    // Check if a key exists
    pub async fn exists(&self, key: &str) -> bool {
        let mut conn = self.manager.clone();

        conn.exists::<&str, bool>(key)
            .await
            .unwrap_or(false)
    }

    // Increment a counter (useful for metrics)
    pub async fn incr(&self, key: &str) -> Result<i64> {
        let mut conn = self.manager.clone();

        conn.incr::<&str, i64, i64>(key, 1)
            .await
            .map_err(|e| anyhow::anyhow!("Redis INCR failed: {}", e))
    }

    // Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheStats> {
        let mut conn = self.manager.clone();

        let hit_count = conn.get::<&str, i64>("cache:stats:hits")
            .await
            .unwrap_or(0);

        let miss_count = conn.get::<&str, i64>("cache:stats:misses")
            .await
            .unwrap_or(0);

        let total = hit_count + miss_count;
        let hit_rate = if total > 0 {
            (hit_count as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(CacheStats {
            hits: hit_count,
            misses: miss_count,
            hit_rate,
        })
    }

    // Record cache hit
    pub async fn record_hit(&self) {
        let _ = self.incr("cache:stats:hits").await;
    }

    // Record cache miss
    pub async fn record_miss(&self) {
        let _ = self.incr("cache:stats:misses").await;
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheStats {
    pub hits: i64,
    pub misses: i64,
    pub hit_rate: f64,
}

// Cache key builders for type safety
pub mod keys {
    use sqlx::types::Uuid;

    pub fn user_session(user_id: &str) -> String {
        format!("user:session:{}", user_id)
    }

    pub fn character(character_id: &Uuid) -> String {
        format!("character:{}", character_id)
    }

    pub fn session(session_id: &Uuid) -> String {
        format!("session:{}", session_id)
    }

    pub fn character_prompt(character_id: &Uuid, user_name: &str) -> String {
        format!("character:prompt:{}:{}", character_id, user_name)
    }

    pub fn user_balance(user_id: &Uuid) -> String {
        format!("user:balance:{}", user_id)
    }

    pub fn graphql_query(query_hash: &str, user_id: &str) -> String {
        format!("gql:query:{}:{}", query_hash, user_id)
    }
}

// TTL constants (in seconds)
pub mod ttl {
    pub const USER_SESSION: u64 = 1800; // 30 minutes
    pub const CHARACTER: u64 = 3600; // 1 hour
    pub const SESSION: u64 = 1800; // 30 minutes
    pub const CHARACTER_PROMPT: u64 = 3600; // 1 hour
    pub const USER_BALANCE: u64 = 300; // 5 minutes
    pub const GRAPHQL_QUERY: u64 = 180; // 3 minutes
}
