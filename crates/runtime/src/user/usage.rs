use anyhow::Result;
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};

use metastable_common::get_current_timestamp;
use metastable_database::SqlxObject;

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

    pub points_consumed_claimed: i64,
    pub points_consumed_purchased: i64,
    pub points_consumed_misc: i64,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserUsage {
    pub fn from_points_consumed(
        user_id: Uuid,
        points_consumed: UserUsagePoints,
    ) -> Self {
        Self {
            id: Uuid::default(),
            user_id,
            model_name: "".to_string(),
            usage: Json(CompletionUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            finish_reason: None,
            points_consumed_claimed: points_consumed.points_consumed_claimed,
            points_consumed_purchased: points_consumed.points_consumed_purchased,
            points_consumed_misc: points_consumed.points_consumed_misc,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }

    pub fn from_llm_response(
        llm_response: &LLMRunResponse, 
        points_consumed: UserUsagePoints,
    ) -> Self {
        Self {
            id: Uuid::default(),
            user_id: llm_response.caller,
            model_name: llm_response.system_config.openai_model.clone(),
            usage: Json(llm_response.usage.clone()),
            finish_reason: llm_response.finish_reason
                .map(|finish_reason| format!("{:?}", finish_reason)),
            points_consumed_claimed: points_consumed.points_consumed_claimed,
            points_consumed_purchased: points_consumed.points_consumed_purchased,
            points_consumed_misc: points_consumed.points_consumed_misc,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}
