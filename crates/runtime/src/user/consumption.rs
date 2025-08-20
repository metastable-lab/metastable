use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_database::{SqlxObject, TextCodecEnum};

use crate::{User, UserUsagePoints};

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, TextCodecEnum)]
#[text_codec(format = "paren", storage_lang = "en")]
pub enum UserPointsConsumptionType {
    #[prefix(lang = "en", content = "LlmCall")]
    LlmCall(Uuid),
    #[prefix(lang = "en", content = "LlmCallRegenerate")]
    LlmCallRegenerate(Uuid),
    #[prefix(lang = "en", content = "LlmCharacterCreation")]
    LlmCharacterCreation(Uuid),

    #[prefix(lang = "en", content = "MemoryUpdate")]
    MemoryUpdate(Uuid),
    #[prefix(lang = "en", content = "FactExtraction")]
    FactExtraction,

    #[catch_all(no_prefix = true)]
    Others(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_points_consumptions"]
pub struct UserPointsConsumption {
    pub id: Uuid,
    
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user: Uuid,

    pub consumption_type: UserPointsConsumptionType,

    pub from_claimed: i64,
    pub from_purchased: i64,
    pub from_misc: i64,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub rewarded_to: Option<Uuid>,
    pub rewarded_points: i64,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserPointsConsumption {
    pub fn from_points_consumed(
        consumption_type: UserPointsConsumptionType,
        user_id: &Uuid, usage: UserUsagePoints, 
        rewarded_to: Option<Uuid>, rewarded_points: i64) -> Self {
        Self {
            id: Uuid::new_v4(),
            
            user: user_id.clone(),
            consumption_type,
            from_claimed: usage.points_consumed_claimed,
            from_purchased: usage.points_consumed_purchased,
            from_misc: usage.points_consumed_misc,

            rewarded_to,
            rewarded_points,

            created_at: 0,
            updated_at: 0,
        }
    }
}