use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_database::{SqlxObject, TextEnum};

use crate::{Message, User, UserUsagePoints};

#[derive(Debug, Clone, Default, TextEnum)]
pub enum UserPointsLogAddReason {
    #[default]
    NA,

    DailyCheckin,
    Inviation,
    SystemReward,
    CreatorReward,
    Purchase,
    DirectPurchase,
}

#[derive(Debug, Clone, Default, TextEnum)]
pub enum UserPointsLogDeductReason {
    #[default]
    NA,

    ChatMessage,
    ChatRegeneration,
    CharacterCreation,
    VoiceGeneration,
}

#[derive(Debug, Clone, Default, TextEnum)]
pub enum UserPointsLogRewardReason {
    #[default]
    NA,
    CreatorReward,
    Inviation,
}

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject)]
#[table_name = "user_points_logs"]
pub struct UserPointsLog {
    pub id: Uuid,
    
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user: Uuid,

    #[foreign_key(referenced_table = "messages", related_rust_type = "Message")]
    pub message: Option<Uuid>,

    pub add_reason: UserPointsLogAddReason,
    pub deduct_reason: UserPointsLogDeductReason,
    pub reward_reason: UserPointsLogRewardReason,

    pub deducted_from_claimed: i64,
    pub deducted_from_purchased: i64,
    pub deducted_from_misc: i64,

    pub added_to_claimed: i64,
    pub added_to_purchased: i64,
    pub added_to_misc: i64,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub reward_to: Option<Uuid>,
    pub reward_amount: i64,


    pub disputed: bool,
    pub disputed_at: Option<i64>,
    pub resolved: bool,
    pub resolved_at: Option<i64>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserPointsLog {
    pub fn from_invitation(
        self_id: &Uuid, others_id: &Uuid, 
        self_amount: i64, others_amount: i64
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user: self_id.clone(),
            
            message: None,

            add_reason: UserPointsLogAddReason::Inviation,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::Inviation,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,

            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: self_amount,

            reward_to: Some(others_id.clone()),
            reward_amount: others_amount,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_daily_checkin(
        user_id: &Uuid, amount: i64
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user: user_id.clone(),
            message: None,

            add_reason: UserPointsLogAddReason::DailyCheckin,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::NA,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,

            added_to_claimed: amount,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_character_creation(
        user_id: &Uuid, usage: UserUsagePoints, message: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            
            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::NA,
            deduct_reason: UserPointsLogDeductReason::CharacterCreation,
            reward_reason: UserPointsLogRewardReason::NA,

            message: Some(message),
            deducted_from_claimed: usage.points_consumed_claimed,
            deducted_from_purchased: usage.points_consumed_purchased,
            deducted_from_misc: usage.points_consumed_misc,

            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_chat_message_regenerate(
        user_id: &Uuid, usage: UserUsagePoints, message: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            
            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::NA,
            deduct_reason: UserPointsLogDeductReason::ChatRegeneration,
            reward_reason: UserPointsLogRewardReason::NA,

            message: Some(message),
            deducted_from_claimed: usage.points_consumed_claimed,
            deducted_from_purchased: usage.points_consumed_purchased,
            deducted_from_misc: usage.points_consumed_misc,

            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_chat_message(
        user_id: &Uuid, usage: UserUsagePoints, message: Uuid,
        character_creator: Uuid, reward_amount: i64,
    ) -> Self {
        let (reward_reason, reward_to, reward_amount) = if usage.points_consumed_purchased > 0 {
            (UserPointsLogRewardReason::CreatorReward, Some(character_creator), reward_amount)
        } else {
            (UserPointsLogRewardReason::NA, None, 0)
        };

        Self {
            id: Uuid::new_v4(),
            
            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::NA,
            deduct_reason: UserPointsLogDeductReason::ChatMessage,
            reward_reason,

            message: Some(message),
            deducted_from_claimed: usage.points_consumed_claimed,
            deducted_from_purchased: usage.points_consumed_purchased,
            deducted_from_misc: usage.points_consumed_misc,

            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to,
            reward_amount,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_system_reward(
        user_id: &Uuid, amount: i64
    ) -> Self {
        Self {
            id: Uuid::new_v4(),

            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::SystemReward,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::NA,

            message: None,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,
            
            added_to_claimed: amount,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_creator_reward(
        user_id: &Uuid, amount: i64
    ) -> Self {
        Self {
            id: Uuid::new_v4(),

            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::CreatorReward,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::NA,
            
            message: None,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,
            
            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: amount,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_purchase(user_id: &Uuid, amount: i64) -> Self {       
        Self {
            id: Uuid::new_v4(),

            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::Purchase,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::NA,
            
            message: None,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,
            
            added_to_claimed: 0,
            added_to_purchased: amount,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_direct_purchase(user_id: &Uuid, amount: i64) -> Self {
        Self {
            id: Uuid::new_v4(),

            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::DirectPurchase,
            deduct_reason: UserPointsLogDeductReason::NA,
            reward_reason: UserPointsLogRewardReason::NA,
            
            message: None,

            deducted_from_claimed: 0,
            deducted_from_purchased: 0,
            deducted_from_misc: 0,
            
            added_to_claimed: 0,
            added_to_purchased: amount,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_voice_generation(
        user_id: &Uuid, usage: UserUsagePoints, message: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user: user_id.clone(),

            add_reason: UserPointsLogAddReason::NA,
            deduct_reason: UserPointsLogDeductReason::VoiceGeneration,
            reward_reason: UserPointsLogRewardReason::NA,

            message: Some(message),
            deducted_from_claimed: usage.points_consumed_claimed,
            deducted_from_purchased: usage.points_consumed_purchased,
            deducted_from_misc: usage.points_consumed_misc,

            added_to_claimed: 0,
            added_to_purchased: 0,
            added_to_misc: 0,

            reward_to: None,
            reward_amount: 0,

            disputed: false,
            disputed_at: None,
            resolved: false,
            resolved_at: None,

            created_at: 0,
            updated_at: 0,
        }
    }

}