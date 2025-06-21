use anyhow::Result;
use serde::{Deserialize, Serialize};
use voda_database::SqlxObject;
use strum_macros::{Display, EnumString};
use sqlx::types::Uuid;

use voda_runtime::User;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)]
pub enum CharacterStatus {
    #[default]
    Draft,
    Reviewing,
    Rejected(String),

    Published,
    Archived(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)]
pub enum CharacterGender {
    #[default]
    Male,
    Female,
    Multiple,
    Others(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)]
pub enum CharacterLanguage {
    #[default]
    English,
    Chinese,
    Japanese,
    Korean,
    Others(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)]
pub enum CharacterFeature {
    #[default]
    DefaultRoleplay,
    Roleplay,
    CharacterCreation,

    BackgroundImage(String),
    AvatarImage(String),

    Voice(String),

    DynamicImage(Vec<String>),
    Others(String),
}

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
    pub language: CharacterLanguage,
    pub features: Vec<CharacterFeature>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,
    pub prompts_background_stories: Vec<String>,
    pub prompts_behavior_traits: Vec<String>,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}
