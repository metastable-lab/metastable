use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use metastable_database::SqlxObject;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "system_configs"]
pub struct SystemConfig {
    pub id: Uuid,

    pub name: String,
    
    pub system_prompt: String,
    pub system_prompt_version: i64,

    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_temperature: f32,
    pub openai_max_tokens: i32,

    pub updated_at: i64,
    pub created_at: i64,
}
