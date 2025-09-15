use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use crate::{User, Character};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_character_mask"]
pub struct CharacterMask {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub mask: String,

    pub created_at: i64,
    pub updated_at: i64
}
