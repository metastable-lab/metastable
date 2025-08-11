use metastable_common::get_current_timestamp;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use metastable_runtime::User;
use crate::{Character};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_character_sub"]
pub struct CharacterSub {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user: Uuid,

    pub sub_type: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}

impl CharacterSub {
    pub fn new(user: Uuid, character: Uuid, sub_type: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user,
            character,
            sub_type,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        }
    }
}