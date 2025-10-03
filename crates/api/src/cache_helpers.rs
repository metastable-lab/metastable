use anyhow::Result;
use metastable_clients::RedisClient;
use metastable_database::{QueryCriteria, SqlxFilterQuery};
use metastable_runtime::{Character, ChatSession};
use sqlx::{PgConnection, types::Uuid};
use tracing::info;

/// Get character from cache or database
pub async fn get_character_cached(
    redis: &RedisClient,
    db_conn: &mut PgConnection,
    character_id: &Uuid,
) -> Result<Option<Character>> {
    let cache_key = format!("character:{}", character_id);

    // Try cache first
    if let Some(character) = redis.get_json::<Character>(&cache_key).await {
        redis.record_hit().await;
        return Ok(Some(character));
    }

    // Cache miss - fetch from database
    redis.record_miss().await;
    let character = Character::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", character_id.clone()),
        db_conn
    ).await?;

    // Store in cache if found
    if let Some(ref ch) = character {
        let _ = redis.set_json(&cache_key, ch, 3600).await; // 1 hour TTL
    }

    Ok(character)
}

/// Get session from cache or database
pub async fn get_session_cached(
    redis: &RedisClient,
    db_conn: &mut PgConnection,
    session_id: &Uuid,
) -> Result<Option<ChatSession>> {
    let cache_key = format!("session:{}", session_id);

    // Try cache first
    if let Some(session) = redis.get_json::<ChatSession>(&cache_key).await {
        redis.record_hit().await;
        return Ok(Some(session));
    }

    // Cache miss - fetch from database
    redis.record_miss().await;
    let session = ChatSession::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
        db_conn
    ).await?;

    // Store in cache if found
    if let Some(ref s) = session {
        let _ = redis.set_json(&cache_key, s, 1800).await; // 30 min TTL
    }

    Ok(session)
}

/// Invalidate user cache
pub async fn invalidate_user_cache(redis: &RedisClient, user_id: &str) {
    let cache_key = format!("user:session:{}", user_id);
    if let Err(e) = redis.delete(&cache_key).await {
        info!("Failed to invalidate user cache for {}: {}", user_id, e);
    }
}

/// Invalidate character cache
pub async fn invalidate_character_cache(redis: &RedisClient, character_id: &Uuid) {
    let cache_key = format!("character:{}", character_id);
    if let Err(e) = redis.delete(&cache_key).await {
        info!("Failed to invalidate character cache for {}: {}", character_id, e);
    }

    // Also invalidate any character prompt caches
    let prompt_pattern = format!("character:prompt:{}:*", character_id);
    if let Err(e) = redis.delete_pattern(&prompt_pattern).await {
        info!("Failed to invalidate character prompt cache for {}: {}", character_id, e);
    }
}

/// Invalidate session cache
pub async fn invalidate_session_cache(redis: &RedisClient, session_id: &Uuid) {
    let cache_key = format!("session:{}", session_id);
    if let Err(e) = redis.delete(&cache_key).await {
        info!("Failed to invalidate session cache for {}: {}", session_id, e);
    }
}
