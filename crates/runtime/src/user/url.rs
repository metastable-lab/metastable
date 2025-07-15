use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_database::SqlxObject;

use crate::User;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_urls"]
pub struct UserUrl {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub created_by: Uuid,
    #[foreign_key_many(referenced_table = "users", related_rust_type = "User")]
    pub used_by: Vec<Uuid>,

    pub url_type: String,
    pub path: String,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserUrl {
    pub fn new(created_by: Uuid, path: String, url_type: String) -> Self {
        let mut url = Self::default();
        url.id = Uuid::new_v4();
        url.created_at = get_current_timestamp();
        url.created_by = created_by;
        url.path = path;
        url.url_type = url_type;
        url
    }
}