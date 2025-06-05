use anyhow::Result;
use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};

use sqlx::types::Json;
use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "system_configs"]
pub struct SystemConfig {
    pub id: CryptoHash,

    pub name: String,
    
    pub system_prompt: String,
    pub system_prompt_version: i64,

    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_temperature: f32,
    pub openai_max_tokens: i32,

    pub functions: Json<Vec<FunctionObject>>,
    pub updated_at: i64,
    pub created_at: i64,
}

impl SqlxPopulateId for SystemConfig {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}
