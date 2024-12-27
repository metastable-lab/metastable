use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};
use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::MongoDbObject;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub user_id: String,

    pub profile: UserProfile,
    pub points: UserPoints,
    pub usage: Vec<UserUsage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub id: CryptoHash,

    pub created_at: u64,
    pub updated_at: u64,

    pub user_personality: Vec<String>,

    pub username: String,
    pub first_name: String, 
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserPoints {
    pub paid_avaliable_balance: u64,
    pub paid_pending_balance: u64, // balance pending confirmation

    pub free_claimed_balance: u64, // balance from FREE rewards or campaigns
    pub redeemed_balance: HashMap<CryptoHash, u64>, // balance redeemed for a specific campaign

    // NOTE: to better keep track of balance source, 
    // we broke down the avaliable balance into three parts:
    // paid_avaliable_balance + free_claimed_balance + SUM(redeemed_balance)

    pub paid_balance_updated_at: u64,
    pub free_claimed_balance_updated_at: u64,
    pub redeemed_balance_updated_at: u64,

    pub total_burnt_balance: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserUsage {
    pub created_at: u64,
    pub model_name: String,
    pub usage: CompletionUsage,
}

impl UserUsage {
    pub fn new(model_name: String, usage: CompletionUsage) -> Self {
        Self {
            created_at: get_current_timestamp(),
            model_name,
            usage,
        }
    }
}

impl UserPoints {
    /* BALANCE ADDITION */
    // Rate limit: ONE Claim per day
    pub fn try_claim_free_balance(&mut self, amount: u64) -> Result<()> {
        let current_timestamp = get_current_timestamp();
        // ONE Claim per day
        if current_timestamp - self.free_claimed_balance_updated_at < 24 * 60 * 60 {
            return Err(anyhow!("Too frequent to claim free balance"));
        }

        self.free_claimed_balance += amount;
        self.free_claimed_balance_updated_at = current_timestamp;
        Ok(())
    }

    // No Rate Limit
    pub fn redeem_balance(&mut self, campaign_id: &CryptoHash, amount: u64) {
        if !self.redeemed_balance.contains_key(campaign_id) {
            self.redeemed_balance.insert(campaign_id.clone(), amount);
        } else {
            *self.redeemed_balance.get_mut(campaign_id).unwrap() += amount;
        }

        self.redeemed_balance_updated_at = get_current_timestamp();
    }

    pub fn record_paid_balance_update(&mut self, amount: u64) {
        self.paid_pending_balance += amount;
        self.paid_balance_updated_at = get_current_timestamp();
    }

    pub fn record_paid_balance_confirmation(&mut self, amount: u64) {
        self.paid_pending_balance -= amount;
        self.paid_avaliable_balance += amount;
        self.paid_balance_updated_at = get_current_timestamp();
    }

    /* BALANCE SUBTRACTION */
    pub fn pay(&mut self, amount: u64) -> bool {
        let mut remaining = amount;
        let self_clone = self.clone();
        let current_timestamp = get_current_timestamp();

        // Try free_claimed_balance first
        if self.free_claimed_balance > 0 {
            let deduct = remaining.min(self.free_claimed_balance);
            self.free_claimed_balance -= deduct;
            self.free_claimed_balance_updated_at = current_timestamp;
            remaining -= deduct;
        }

        // Try redeemed_balance next
        if remaining > 0 {
            let mut redeemed_modified = false;
            for balance in self.redeemed_balance.values_mut() {
                if remaining == 0 { break; }
                let deduct = remaining.min(*balance);
                *balance -= deduct;
                remaining -= deduct;
                redeemed_modified = true;
            }
            if redeemed_modified {
                self.redeemed_balance_updated_at = current_timestamp;
            }
        }

        // Finally try paid_avaliable_balance
        if remaining > 0 {
            if self.paid_avaliable_balance >= remaining {
                self.paid_avaliable_balance -= remaining;
                self.paid_balance_updated_at = current_timestamp;
                remaining = 0;
            }
        }

        // If we couldn't pay the full amount, revert all changes
        if remaining > 0 {
            *self = self_clone;
            false
        } else {
            self.total_burnt_balance += amount;
            true
        }
    }

    pub fn get_available_balance(&self) -> u64 {
        self.paid_avaliable_balance + self.free_claimed_balance + self.redeemed_balance.values().sum::<u64>()
    }
}

impl User {
    pub fn new(profile: UserProfile, user_id: String) -> Self {
        Self {
            id: profile.id.clone(),
            user_id,
            profile,
            points: UserPoints::default(),
            usage: Vec::new(),
        }
    }
}

impl MongoDbObject for User {
    const COLLECTION_NAME: &'static str = "users";
    type Error = anyhow::Error;
    
    fn get_id(&self) -> CryptoHash { self.id.clone() }
    fn populate_id(&mut self) { self.id = self.profile.id.clone(); }
}