use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::{Json, Uuid};
use metastable_database::{SqlxObject, TextEnum};

use crate::User;

#[derive(Debug, Clone, TextEnum, Default)]
pub enum UserPaymentStatus {
    #[default]
    Pending,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_payments"]
pub struct UserPayment {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    #[indexed]
    pub user_id: Uuid,

    #[indexed]
    pub checkout_session_id: String,
    pub url: String,

    pub amount_total: i64,
    pub currency: String,

    pub items: Json<serde_json::Value>,

    pub vip_level: i32,

    pub status: UserPaymentStatus,
    pub created_at: i64,
    pub updated_at: i64,
}
