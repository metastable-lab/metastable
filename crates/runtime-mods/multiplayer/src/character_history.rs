use anyhow::Result;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use metastable_runtime::User;

use crate::{CharacterStatus, CharacterGender, CharacterLanguage, CharacterFeature, Character};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_characters_history"]
pub struct CharacterHistory {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub character: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub creator: Uuid,

    pub name: String,
    pub description: String,
    
    pub version: i64,

    pub status: CharacterStatus,
    pub gender: CharacterGender,
    pub language: CharacterLanguage,
    pub features: Vec<CharacterFeature>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,
    pub prompts_background_stories: Vec<String>,
    pub prompts_behavior_traits: Vec<String>,

    pub creator_notes: Option<String>,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}

impl CharacterHistory {
    pub fn new(character: Character) -> Self {
        Self {
            id: Uuid::new_v4(),
            character: character.id,
            creator: character.creator,
            name: character.name,
            description: character.description,
            version: character.version,
            status: character.status,
            gender: character.gender,
            language: character.language,
            features: character.features,
            prompts_scenario: character.prompts_scenario,
            prompts_personality: character.prompts_personality,
            prompts_example_dialogue: character.prompts_example_dialogue,
            prompts_first_message: character.prompts_first_message,
            prompts_background_stories: character.prompts_background_stories,
            prompts_behavior_traits: character.prompts_behavior_traits,
            creator_notes: character.creator_notes,
            tags: character.tags,
            created_at: 0,
            updated_at: 0,
        }
    }
}