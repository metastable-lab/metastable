use anyhow::Result;
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use voda_common::get_current_timestamp;
use voda_database::SqlxObject;

use crate::{user::User, LLMRunResponse};

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "user_usages"]
pub struct UserUsage {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub model_name: String,
    pub usage: Json<CompletionUsage>,
    pub finish_reason: Option<String>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserUsage {
    pub fn from_llm_response(llm_response: &LLMRunResponse) -> Self {
        Self {
            id: Uuid::default(),
            user_id: llm_response.caller,
            model_name: llm_response.system_config.openai_model.clone(),
            usage: Json(llm_response.usage.clone()),
            finish_reason: llm_response.finish_reason
                .map(|finish_reason| format!("{:?}", finish_reason)),
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}
