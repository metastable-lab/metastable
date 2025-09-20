use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::{Json, Uuid};

use crate::User;

use super::{
    BackgroundStories, BehaviorTraits, Character, CharacterFeature, 
    CharacterLanguage, CharacterStatus, Relationships, SkillsAndInterests
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_characters_history"]
#[allow_type_change]
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
    pub language: CharacterLanguage,
    pub features: Json<Vec<CharacterFeature>>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_first_message: Json<Option<FunctionCall>>,

    // v0
    pub prompts_example_dialogue: String,
    pub prompts_background_stories: Json<Vec<BackgroundStories>>,
    pub prompts_behavior_traits: Json<Vec<BehaviorTraits>>,

    // v1
    pub prompts_additional_example_dialogue: Json<Vec<String>>,
    pub prompts_relationships: Json<Vec<Relationships>>,
    pub prompts_skills_and_interests: Json<Vec<SkillsAndInterests>>,
    pub prompts_additional_info: Json<Vec<String>>,

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
            language: character.language,
            features: character.features,
            prompts_scenario: character.prompts_scenario,
            prompts_personality: character.prompts_personality,
            prompts_first_message: character.prompts_first_message,

            prompts_example_dialogue: character.prompts_example_dialogue,
            prompts_background_stories: character.prompts_background_stories,
            prompts_behavior_traits: character.prompts_behavior_traits,

            prompts_additional_example_dialogue: character.prompts_additional_example_dialogue,
            prompts_relationships: character.prompts_relationships,
            prompts_skills_and_interests: character.prompts_skills_and_interests,
            prompts_additional_info: character.prompts_additional_info,
            creator_notes: character.creator_notes,
            tags: character.tags,
            created_at: 0,
            updated_at: 0,
        }
    }
}