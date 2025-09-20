use anyhow::Result;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use metastable_runtime::{Message, User, ToolCall};
use metastable_runtime::{CharacterFeature, BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests};
use async_openai::types::FunctionCall;
use metastable_runtime_roleplay::agents::{SendMessage, RoleplayMessageType};
use sqlx::types::Json;

pub use metastable_runtime::{
    Character as NewCharacter,
    CharacterLanguage, CharacterStatus, CharacterOrientation,
};
use metastable_runtime::ChatSession;

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_characters"]
pub struct Character {
    pub id: Uuid,

    pub name: String,
    pub description: String,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub creator: Uuid,

    #[foreign_key(referenced_table = "messages", related_rust_type = "Message")]
    pub creation_message: Option<Uuid>,

    #[foreign_key(referenced_table = "chat_sessions", related_rust_type = "ChatSession")]
    pub creation_session: Option<Uuid>,

    pub version: i64,

    pub status: CharacterStatus,
    pub orientation: CharacterOrientation,
    pub language: CharacterLanguage,
    pub features: Vec<String>,

    pub prompts_scenario: String,
    pub prompts_personality: String,
    pub prompts_first_message: String,

    // v0
    pub prompts_example_dialogue: String,
    pub prompts_background_stories: Vec<String>,
    pub prompts_behavior_traits: Vec<String>,

    // v1
    pub prompts_additional_example_dialogue: Vec<String>,
    pub prompts_relationships: Vec<String>,
    pub prompts_skills_and_interests: Vec<String>,
    pub prompts_additional_info: Vec<String>,

    pub creator_notes: Option<String>,

    pub tags: Vec<String>,

    pub created_at: i64,
    pub updated_at: i64
}

impl Character {
    pub fn into_new_character(&self) -> NewCharacter {
        NewCharacter {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),

            creator: self.creator,

            creation_message: self.creation_message,
            creation_session: self.creation_session,

            version: self.version,
            status: self.status.clone(),
            orientation: self.orientation.clone(),
            language: self.language.clone(),
            features: self.migrate_features(),
            prompts_scenario: self.prompts_scenario.clone(),
            prompts_personality: self.prompts_personality.clone(),
            prompts_first_message: self.migrate_first_message(),
            prompts_example_dialogue: self.prompts_example_dialogue.clone(),
            prompts_background_stories: self.migrate_background_stories(),
            prompts_behavior_traits: self.migrate_behavior_traits(),
            prompts_additional_example_dialogue: Json(self.prompts_additional_example_dialogue.clone()),
            prompts_relationships: self.migrate_relationships(),
            prompts_skills_and_interests: self.migrate_skills_and_interests(),
            prompts_additional_info: Json(self.prompts_additional_info.clone()),
            creator_notes: self.creator_notes.clone(),
            tags: self.tags.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            ..Default::default()
        }
    }

    pub fn migrate_features(&self) -> Json<Vec<CharacterFeature>> {
        let mut new_features = Vec::new();
        for feature_str in &self.features {
            if let Some(start_paren) = feature_str.find('(') {
                if feature_str.ends_with(')') {
                    let feature_type = &feature_str[..start_paren];
                    let content = &feature_str[start_paren + 1..feature_str.len() - 1];
                    let feature = match feature_type {
                        "AvatarImage" => CharacterFeature::AvatarImage(content.to_string()),
                        "BackgroundImage" => CharacterFeature::BackgroundImage(content.to_string()),
                        "Voice" => CharacterFeature::Voice(content.to_string()),
                        _ => CharacterFeature::Others(feature_str.to_string()),
                    };
                    new_features.push(feature);
                } else {
                    new_features.push(CharacterFeature::Others(feature_str.to_string()));
                }
            } else {
                let feature = match feature_str.as_str() {
                    "Roleplay" => CharacterFeature::Roleplay,
                    "CharacterCreation" => CharacterFeature::CharacterCreation,
                    _ => CharacterFeature::Others(feature_str.to_string()),
                };
                new_features.push(feature);
            }
        }
        new_features.dedup();
        Json(new_features)
    }

    pub fn migrate_background_stories(&self) -> Json<Vec<BackgroundStories>> {
        Json(parse_prefixed_string(
            &self.prompts_background_stories,
            &[
                ("职业", BackgroundStories::Professions),
                ("童年经历", BackgroundStories::ChildhoodExperience),
                ("成长环境", BackgroundStories::GrowthEnvironment),
                ("重大经历", BackgroundStories::SignificantEvents),
                ("价值观", BackgroundStories::ValuesAndBeliefs),
                ("过去的遗憾或创伤，无法释怀的事", BackgroundStories::Regrets),
                ("梦想，渴望的事情，追求的事情", BackgroundStories::Dreams),
            ],
            BackgroundStories::Others,
        ))
    }

    pub fn migrate_behavior_traits(&self) -> Json<Vec<BehaviorTraits>> {
        Json(parse_prefixed_string(
            &self.prompts_behavior_traits,
            &[
                ("行为举止", BehaviorTraits::PhysicalBehavior),
                ("外貌特征", BehaviorTraits::PhysicalAppearance),
                ("穿搭风格", BehaviorTraits::ClothingStyle),
                ("情绪表达方式", BehaviorTraits::EmotionalExpression),
                ("个人沟通习惯", BehaviorTraits::GenralCommunicationStyle),
                ("与用户的沟通习惯", BehaviorTraits::CommunicationStyleWithUser),
                ("个人行为特征", BehaviorTraits::GeneralBehaviorTraits),
                ("与用户的沟通特征", BehaviorTraits::BehaviorTraitsWithUser),
            ],
            BehaviorTraits::Others,
        ))
    }

    pub fn migrate_relationships(&self) -> Json<Vec<Relationships>> {
        Json(parse_prefixed_string(
            &self.prompts_relationships,
            &[
                ("亲密伴侣", Relationships::IntimatePartner),
                ("家庭", Relationships::Family),
                ("朋友", Relationships::Friends),
                ("敌人", Relationships::Enemies),
                ("社交圈", Relationships::SocialCircle),
            ],
            Relationships::Others,
        ))
    }

    pub fn migrate_skills_and_interests(&self) -> Json<Vec<SkillsAndInterests>> {
        Json(parse_prefixed_string(
            &self.prompts_skills_and_interests,
            &[
                ("职业技能", SkillsAndInterests::ProfessionalSkills),
                ("生活技能", SkillsAndInterests::LifeSkills),
                ("兴趣爱好", SkillsAndInterests::HobbiesAndInterests),
                ("弱点，不擅长的领域", SkillsAndInterests::Weaknesses),
                ("优点，擅长的事情", SkillsAndInterests::Virtues),
                ("内心矛盾冲突", SkillsAndInterests::InnerConflicts),
                ("性癖", SkillsAndInterests::Kinks),
            ],
            SkillsAndInterests::Others,
        ))
    }

    pub fn migrate_first_message(&self) -> Json<Option<FunctionCall>> {
        if let Ok(function_call) = serde_json::from_str::<FunctionCall>(&self.prompts_first_message) {
            if function_call.name == "send_message" {
                return Json(Some(function_call));
            }
        }

        let parsed_messages = RoleplayMessageType::from_legacy_message(&self.prompts_first_message);
        let send_message = SendMessage {
            messages: parsed_messages,
            ..Default::default()
        };
        let tool_call = send_message.into_tool_call().ok();
        Json(tool_call)
    }
}

fn parse_prefixed_string<T>(
    legacy_values: &[String],
    prefix_map: &[(&str, fn(String) -> T)],
    catch_all: fn(String) -> T,
) -> Vec<T> {
    let mut new_values = Vec::new();
    for s in legacy_values {
        let mut current_s = s.as_str().trim();
        while let Some(stripped) = current_s.strip_prefix("Others:").or(current_s.strip_prefix("Others：")) {
            current_s = stripped.trim();
        }

        let mut matched = false;
        for (prefix, constructor) in prefix_map {
            if let Some(content) = current_s.strip_prefix(prefix) {
                let content = content.strip_prefix("：").unwrap_or(content).trim();
                new_values.push(constructor(content.to_string()));
                matched = true;
                break;
            }
        }
        if !matched {
            new_values.push(catch_all(current_s.to_string()));
        }
    }
    new_values
}
