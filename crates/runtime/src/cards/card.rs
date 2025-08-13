use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use metastable_database::{SqlxObject};
use crate::User;

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "card"]
pub struct Card {
    pub id: Uuid,
    pub name: String,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub author: Uuid,

    pub price: i64,
    pub rating: i64, // 0-2, 0 = SS, 1 = S, 2 = A
    pub description: String,

    // character related
    pub image_url: String,
    pub injected_prompt: Option<String>,
    
    // styling related
    pub card_styling: String,
    pub card_type: String,
    pub card_inner_html: String,

    pub created_at: i64,
    pub updated_at: i64,
}
