mod badge;
mod url;
mod referral;
mod follow;
mod log;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use metastable_database::TextCodecEnum;
use sqlx::types::{Json, Uuid};
use serde_json::json;
use metastable_common::{encrypt, decrypt, get_current_timestamp};
use metastable_database::SqlxObject;

pub use url::UserUrl;
pub use referral::UserReferral;
pub use badge::UserBadge;
pub use follow::UserFollow;
pub use log::UserPointsLog;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserUsagePoints {
    pub points_consumed_claimed: i64,
    pub points_consumed_purchased: i64,
    pub points_consumed_misc: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default, TextCodecEnum)]
#[text_codec(format = "paren", storage_lang = "en")]
pub enum UserRole {
    Admin,
    #[default]
    User,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "users"]
pub struct User {
    pub id: Uuid,
    #[unique]
    #[indexed]
    pub user_id: String,
    pub user_aka: String,

    pub role: UserRole,
    pub provider: String,
    pub banned: bool,

    pub generated_referral_count: i64,

    // access to the llm system
    pub llm_access_level: i64,

    // points related
    pub running_claimed_balance: i64,
    pub running_purchased_balance: i64,
    pub running_misc_balance: i64,

    pub balance_usage: i64,

    pub free_balance_claimed_at: i64,
    pub last_balance_deduction_at: i64,

    // profile related
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
    
    pub mask: Vec<String>,

    pub extra: Option<Json<Value>>, // array of user profiles to be injected into prompts

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedRequest {
    pub user_id: String,
    pub timestamp: i64,
    pub origin: String,
}

impl User {
    pub fn generate_auth_token(&self, salt: &str) -> String {
        let payload = json!({
            "user_id": self.user_id,
            "timestamp": metastable_common::get_current_timestamp(),
            "origin": "runtime"
        });
        let payload_str = payload.to_string();
        encrypt(&payload_str, salt)
            .expect("[User::generate_auth_token] failed to encrypt auth token")
    }

    pub fn verify_auth_token(token: &str, salt: &str) -> Result<String> {
        let decrypted = decrypt(token, salt)?;
        let authenticated_request: AuthenticatedRequest = serde_json::from_str(&decrypted)?;
        if authenticated_request.timestamp < get_current_timestamp() - 60 * 60 * 24 * 30 {
            return Err(anyhow::anyhow!("[User::verify_auth_token] authenticate expired"));
        }
        Ok(authenticated_request.user_id)
    }
}

impl User {
    /* BALANCE ADDITION */
    // Rate limit: ONE Claim per 24 hours
    pub fn try_claim_free_balance(&mut self, amount: i64) -> Result<UserPointsLog> {
        let current_timestamp = get_current_timestamp();
        // ONE Claim per day
        if current_timestamp - self.free_balance_claimed_at < 24 * 60 * 60 {
            return Err(anyhow!("[User::try_claim_free_balance] Too frequent to claim free balance"));
        }

        self.running_claimed_balance += amount;
        self.free_balance_claimed_at = current_timestamp;

        // add log

        Ok(UserPointsLog::from_daily_checkin(&self.id, amount))
    }

    pub fn invitation_reward(&mut self, others_id: &Uuid, amount: i64, others_amount: i64) -> UserPointsLog {
        self.running_misc_balance += amount;
        UserPointsLog::from_invitation(&self.id, others_id, amount, others_amount)
    }

    pub fn system_reward(&mut self, amount: i64) -> UserPointsLog {
        self.running_misc_balance += amount;
        UserPointsLog::from_system_reward(&self.id, amount)
    }

    pub fn creator_reward(&mut self, amount: i64) -> UserPointsLog {
        self.running_misc_balance += amount;
        UserPointsLog::from_creator_reward(&self.id, amount)
    }

    /* BALANCE SUBTRACTION */
    pub fn pay_for_character_creation(&mut self, amount: i64, message: Uuid) -> Result<UserPointsLog> {
        let usage = self.pay(amount)?;
        Ok(UserPointsLog::from_character_creation(&self.id, usage, message))
    }

    pub fn pay_for_chat_message_regenerate(&mut self, amount: i64, message: Uuid) -> Result<UserPointsLog> {
        let usage = self.pay(amount)?;
        Ok(UserPointsLog::from_chat_message_regenerate(&self.id, usage, message))
    }

    pub fn pay_for_chat_message(&mut self, amount: i64, message: Uuid, character_creator: Uuid, reward_amount: i64) -> Result<UserPointsLog> {
        let usage = self.pay(amount)?;
        Ok(UserPointsLog::from_chat_message(&self.id, usage, message, character_creator, reward_amount))
    }

    fn pay(&mut self, amount: i64) -> Result<UserUsagePoints> {
        let mut remaining = amount;
        let self_clone = self.clone();
        let current_timestamp = get_current_timestamp();

        let mut paid_claimed_balance = 0;
        let mut paid_misc_balance = 0;
        let mut paid_purchased_balance = 0;

        // Try free_claimed_balance first
        if self.running_claimed_balance > 0 {
            let deduct = remaining.min(self.running_claimed_balance);
            self.running_claimed_balance -= deduct;
            self.last_balance_deduction_at = current_timestamp;
            remaining -= deduct;
            paid_claimed_balance += deduct;
        }

        // Try misc_balance next
        if remaining > 0 {
            let deduct = remaining.min(self.running_misc_balance);
            self.running_misc_balance -= deduct;
            remaining -= deduct;
            paid_misc_balance += deduct;
        }

        // Finally try paid_avaliable_balance
        if remaining > 0 {
            if self.running_purchased_balance >= remaining {
                self.running_purchased_balance -= remaining;
                self.last_balance_deduction_at = current_timestamp;
                remaining = 0;
                paid_purchased_balance += remaining;
            }
        }

        // If we couldn't pay the full amount, revert all changes
        if remaining > 0 {
            *self = self_clone;
            Err(anyhow!("[User::pay] failed to pay balance"))
        } else {
            self.balance_usage += amount;
            Ok(UserUsagePoints {
                points_consumed_claimed: paid_claimed_balance,
                points_consumed_misc: paid_misc_balance,
                points_consumed_purchased: paid_purchased_balance,
            })
        }
    }

    pub fn try_pay(&self, amount: i64) -> Result<i64> {
        if self.running_purchased_balance + self.running_claimed_balance + self.running_misc_balance < amount {
            Err(anyhow!("[User::try_pay] Insufficient balance"))
        } else {
            Ok(amount)
        }
    }
}
