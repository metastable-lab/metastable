use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::{SqlxObject, SqlxPopulateId};

use crate::user::BALANCE_CAP;

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, SqlxObject)]
#[table_name = "user_points"]
pub struct UserPoints {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub running_claimed_balance: i64,
    pub running_purchased_balance: i64,
    pub running_misc_balance: i64,

    pub balance_usage: i64,

    pub free_balance_claimed_at: i64,
    pub last_balance_deduction_at: i64,
}

impl SqlxPopulateId for UserPoints {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            anyhow::bail!("[UserPoints] id is not populated");
        } else {
            Ok(())
        }
    }
}

impl UserPoints {
    /* BALANCE ADDITION */
    // Rate limit: ONE Claim per day
    // running balance sum should be <= BALANCE_CAP
    // if over BALANCE_CAP, reduce free balance first
    // don't reduce paid balance
    pub fn try_claim_free_balance(&mut self, amount: i64) -> Result<()> {
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

    pub fn purchase_balance(&mut self, amount: i64) {
        self.running_purchased_balance += amount;
    }

    pub fn add_misc_balance(&mut self, amount: i64) {
        self.running_misc_balance += amount;
    }

    /* BALANCE SUBTRACTION */
    pub fn pay(&mut self, amount: i64) -> bool {
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

    pub fn get_available_balance(&self) -> i64 {
        self.running_purchased_balance + self.running_claimed_balance + self.running_misc_balance
    }
}
