use anyhow::Result;
use serde::{Deserialize, Serialize};
use voda_common::CryptoHash;
use voda_database::{SqlxObject, SqlxPopulateId};
use strum_macros::{Display, EnumString};

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
    Roleplay,
    BackgroundImage(String),
    AvatarImage(String),
    Voice(String),
    DynamicImage,
    Others(String),
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "characters"]
pub struct Character {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub name: String,
    pub description: String,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub creator: CryptoHash,
    pub reviewed_by: Option<CryptoHash>,

    pub version: i64,

    pub status: CharacterStatus,
    pub gender: CharacterGender,
    pub language: CharacterLanguage,
    pub features: Vec<CharacterFeature>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: i64,
}

impl SqlxPopulateId for Character {
    fn sql_populate_id(&mut self) -> Result<()> {
        if self.id == CryptoHash::default() {
            self.id = CryptoHash::random();
        }
        Ok(())
    }
}
