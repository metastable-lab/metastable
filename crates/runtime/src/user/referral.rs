use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use voda_common::{blake3_hash, get_current_timestamp};
use voda_database::SqlxObject;

use crate::{user::User, UserRole};

pub const REFERRAL_CODE_PRICE: i64 = 10;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_referrals"]
pub struct UserReferral {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,
    
    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub used_by: Option<Uuid>,
    pub used_at: Option<i64>,

    pub code_seed: Uuid,
    pub code: String,

    pub created_at: i64,
    pub updated_at: i64,
}

impl User {
    pub fn buy_referral_code(&mut self, count: i64) -> Result<Vec<UserReferral>> {
        if self.role != UserRole::Admin {
            if self.get_available_balance() < REFERRAL_CODE_PRICE * count {
                return Err(anyhow!("[User::buy_referral_code] Insufficient balance"));
            }
    
            self.pay(REFERRAL_CODE_PRICE * count);
        }

        self.generated_referral_count += count;
        let mut referrals = Vec::new();
        for _ in 0..count {
            let code_seed = Uuid::new_v4();
            let code = blake3_hash(code_seed.as_bytes())
                .to_hex_string()
                .chars()
                .take(16)
                .collect::<String>();

            referrals.push(UserReferral {
                id: Uuid::new_v4(),
                user_id: self.id,
                used_by: None,
                used_at: None,
                code_seed,
                code,
                created_at: get_current_timestamp(),
                updated_at: get_current_timestamp(),
            });
        }
        Ok(referrals)
    }
}