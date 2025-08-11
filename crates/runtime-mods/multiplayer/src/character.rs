use anyhow::Result;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use strum_macros::{Display, EnumString};
use sqlx::types::Uuid;
use std::fmt;
use std::str::FromStr;

use metastable_runtime::User;

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

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default)]
pub enum CharacterFeature {
    #[default]
    Roleplay,
    CharacterCreation,

    BackgroundImage(String),
    AvatarImage(String),

    Voice(String),

    DynamicImage(Vec<String>),
    Others(String),
}

impl fmt::Display for CharacterFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CharacterFeature::Roleplay => write!(f, "Roleplay"),
            CharacterFeature::CharacterCreation => write!(f, "CharacterCreation"),
            CharacterFeature::BackgroundImage(s) => write!(f, "BackgroundImage({})", s),
            CharacterFeature::AvatarImage(s) => write!(f, "AvatarImage({})", s),
            CharacterFeature::Voice(s) => write!(f, "Voice({})", s),
            CharacterFeature::DynamicImage(v) => {
                // For DynamicImage, join vector elements with a comma
                write!(f, "DynamicImage([{}])", v.join(","))
            }
            CharacterFeature::Others(s) => write!(f, "Others({})", s),
        }
    }
}

impl FromStr for CharacterFeature {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Roleplay" => Ok(CharacterFeature::Roleplay),
            "CharacterCreation" => Ok(CharacterFeature::CharacterCreation),
            s if s.starts_with("BackgroundImage(") && s.ends_with(')') => {
                let inner = &s["BackgroundImage(".len()..s.len() - 1];
                Ok(CharacterFeature::BackgroundImage(inner.to_string()))
            }
            s if s.starts_with("AvatarImage(") && s.ends_with(')') => {
                let inner = &s["AvatarImage(".len()..s.len() - 1];
                Ok(CharacterFeature::AvatarImage(inner.to_string()))
            }
            s if s.starts_with("Voice(") && s.ends_with(')') => {
                let inner = &s["Voice(".len()..s.len() - 1];
                Ok(CharacterFeature::Voice(inner.to_string()))
            }
            s if s.starts_with("DynamicImage([") && s.ends_with("])") => {
                let inner = &s["DynamicImage([".len()..s.len() - 2];
                if inner.is_empty() {
                    Ok(CharacterFeature::DynamicImage(vec![]))
                } else {
                    let vec = inner.split(',').map(|s| s.trim().to_string()).collect();
                    Ok(CharacterFeature::DynamicImage(vec))
                }
            }
            s if s.starts_with("Others(") && s.ends_with(')') => {
                let inner = &s["Others(".len()..s.len() - 1];
                Ok(CharacterFeature::Others(inner.to_string()))
            }
            _ => anyhow::bail!("Invalid CharacterFeature string: {}", s),
        }
    }
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

    pub creator_notes: Option<String>,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}
