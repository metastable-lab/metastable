use anyhow::{anyhow, Result};
use async_openai::types::CompletionUsage;
use mongodb::Database;
use serde::{Deserialize, Serialize};
use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::MongoDbObject;

pub const BALANCE_CAP: u64 = 500;

#[derive(Debug, Serialize, Deserialize, Clone, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum UserRole {
    Admin,
    #[default]
    User,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserProvider {
    #[default]
    Telegram,
    Google,
    X,
    Github,
    CryptoWallet,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub user_id: String,

    pub role: UserRole,
    pub provider: UserProvider,
    pub network_name: Option<String>,

    pub profile: UserProfile,
    pub points: UserPoints,
    pub usage: Vec<UserUsage>,

    pub last_active: u64,
    pub created_at: u64,
}

impl MongoDbObject for User {
    const COLLECTION_NAME: &'static str = "users";
    type Error = anyhow::Error;
    
    fn get_id(&self) -> CryptoHash { self.id.clone() }
    fn populate_id(&mut self) { self.id = self.profile.id.clone(); }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub id: CryptoHash,

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
    pub running_claimed_balance: u64,
    pub running_purchased_balance: u64,
    pub running_misc_balance: u64,

    pub balance_usage: u64,

    pub free_balance_claimed_at: u64,
    pub last_balance_deduction_at: u64,
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
    // running balance sum should be <= BALANCE_CAP
    // if over BALANCE_CAP, reduce free balance first
    // don't reduce paid balance
    pub fn try_claim_free_balance(&mut self, amount: u64) -> Result<()> {
        let current_timestamp = get_current_timestamp();
        // ONE Claim per day
        if current_timestamp - self.free_balance_claimed_at < 24 * 60 * 60 {
            return Err(anyhow!("Too frequent to claim free balance"));
        }

        self.running_claimed_balance += amount;
        self.free_balance_claimed_at = current_timestamp;

        // capping logic - no errors
            if amount + 
            self.running_claimed_balance + 
            self.running_purchased_balance + 
            self.running_misc_balance > BALANCE_CAP 
        {
            if self.running_purchased_balance + self.running_misc_balance < BALANCE_CAP {
                self.running_claimed_balance = BALANCE_CAP - self.running_purchased_balance - self.running_misc_balance;
            } else {
                // noop
            }
        }

        Ok(())
    }

    pub fn purchase_balance(&mut self, amount: u64) {
        self.running_purchased_balance += amount;
    }

    pub fn add_misc_balance(&mut self, amount: u64) {
        self.running_misc_balance += amount;
    }

    /* BALANCE SUBTRACTION */
    pub fn pay(&mut self, amount: u64) -> bool {
        let mut remaining = amount;
        let self_clone = self.clone();
        let current_timestamp = get_current_timestamp();

        // Try free_claimed_balance first
        if self.running_claimed_balance > 0 {
            let deduct = remaining.min(self.running_claimed_balance);
            self.running_claimed_balance -= deduct;
            self.last_balance_deduction_at = current_timestamp;
            remaining -= deduct;
        }

        // Try misc_balance next
        if remaining > 0 {
            let deduct = remaining.min(self.running_misc_balance);
            self.running_misc_balance -= deduct;
            remaining -= deduct;
        }

        // Finally try paid_avaliable_balance
        if remaining > 0 {
            if self.running_purchased_balance >= remaining {
                self.running_purchased_balance -= remaining;
                self.last_balance_deduction_at = current_timestamp;
                remaining = 0;
            }
        }

        // If we couldn't pay the full amount, revert all changes
        if remaining > 0 {
            *self = self_clone;
            false
        } else {
            self.balance_usage += amount;
            true
        }
    }

    pub fn get_available_balance(&self) -> u64 {
        self.running_purchased_balance + self.running_claimed_balance + self.running_misc_balance
    }
}

impl User {
    pub fn new(profile: UserProfile, user_id: String) -> Self {
        Self {
            id: profile.id.clone(),
            user_id,
            role: UserRole::default(),
            provider: UserProvider::default(),
            network_name: None,

            profile,
            points: UserPoints::default(),
            usage: Vec::new(),

            last_active: get_current_timestamp(),
            created_at: get_current_timestamp(),
        }
    }

    // High Level Wrapper
    pub async fn pay_and_update(db: &Database, user_id: &CryptoHash, amount: u64) -> Result<()> {
        let mut user = Self::select_one_by_index(db, user_id).await?
            .ok_or(anyhow!("User not found"))?;

        if user.role != UserRole::Admin {
            if !user.points.pay(amount) {
                return Err(anyhow!("Insufficient points"));
            }
            user.save_or_update(db).await?;
        }
        Ok(())
    }

    pub async fn claim_free_balance(db: &Database, user_id: &CryptoHash, amount: u64) -> Result<()> {
        let mut user = Self::select_one_by_index(db, user_id).await?
            .ok_or(anyhow!("User not found"))?;
        user.points.try_claim_free_balance(amount)?;
        user.save_or_update(db).await?;
        Ok(())
    }

    pub async fn record_purchase_balance(db: &Database, user_id: &CryptoHash, amount: u64) -> Result<()> {
        let mut user = Self::select_one_by_index(db, user_id).await?
            .ok_or(anyhow!("User not found"))?;
        user.points.purchase_balance(amount);
        user.save_or_update(db).await?;
        Ok(())
    }

    pub async fn record_misc_balance(db: &Database, user_id: &CryptoHash, amount: u64) -> Result<()> {
        let mut user = Self::select_one_by_index(db, user_id).await?
            .ok_or(anyhow!("User not found"))?;
        user.points.add_misc_balance(amount);
        user.save_or_update(db).await?;
        Ok(())
    }

    pub fn add_usage(&mut self, usage: CompletionUsage, model_name: String) {
        self.usage.push(UserUsage::new(model_name, usage));
    }
}
