use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::{Json, Uuid};

use crate::cards::{card::Card, DrawProbability};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "card_pool"]
pub struct CardPool {
    pub id: Uuid,
    pub name: String,

    #[foreign_key_many(referenced_table = "card", related_rust_type = "Card")]
    pub card_ids : Vec<Uuid>,
    pub pool_settings: Json<DrawProbability>,
    
    pub description: String,
    pub image_url: String,

    pub start_time: i64,
    pub end_time: i64,

    pub created_at: i64,
    pub updated_at: i64,
}
