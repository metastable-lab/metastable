use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use crate::{User, CharacterPost};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_character_post_comments"]
pub struct CharacterPostComments {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_character_posts", related_rust_type = "CharacterPost")]
    pub post: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user_id: Uuid,

    pub content: String,
    pub reaction: String,

    pub created_at: i64,
    pub updated_at: i64
}
