use anyhow::Result;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use metastable_runtime::User;
use super::character_detail::{
    BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests,
    CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus, CharacterOrientation,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_characters"]
pub struct Character {
    pub id: Uuid,

    pub name: String,
    pub description: String,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub creator: Uuid,

    pub version: i64,

    pub status: CharacterStatus,
    pub gender: CharacterGender,
    pub orientation: CharacterOrientation,
    pub language: CharacterLanguage,
    pub features: Vec<CharacterFeature>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_first_message: String,

    // v0
    pub prompts_example_dialogue: String,
    pub prompts_background_stories: Vec<BackgroundStories>,
    pub prompts_behavior_traits: Vec<BehaviorTraits>,

    // v1
    pub prompts_additional_example_dialogue: Vec<String>,
    pub prompts_relationships: Vec<Relationships>,
    pub prompts_skills_and_interests: Vec<SkillsAndInterests>,
    pub prompts_additional_info: Vec<String>,

    pub creator_notes: Option<String>,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}

