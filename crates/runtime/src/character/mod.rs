mod audit;
mod character_detail;
mod character_history;
mod character_sub;

use anyhow::Result;
use metastable_common::get_time_in_utc8;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Uuid;

use crate::{Message, MessageRole, MessageType, Prompt, User};

pub use character_detail::{
    BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests,
    CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus, CharacterOrientation,
};

pub use audit::AuditLog;
pub use character_history::CharacterHistory;
pub use character_sub::CharacterSub;

use crate::ChatSession;

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

impl Character {
    pub fn build_system_prompt(&self, prompt: &str, user_name: &str) -> Prompt {
        let prompts_background_stories = self.prompts_background_stories
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let prompts_behavior_traits = self.prompts_behavior_traits
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let prompts_relationships = self.prompts_relationships
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let prompts_skills_and_interests = self.prompts_skills_and_interests
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let prompts_additional_info = self.prompts_additional_info
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let prompts_additional_example_dialogue = self.prompts_additional_example_dialogue
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");

        let request_time = get_time_in_utc8();
        let p = prompt
            .replace("{{char}}", &self.name)
            .replace("{{user}}", user_name)
            .replace("{{request_time}}", &request_time)
            .replace("{{char_personality}}", &self.prompts_personality)
            .replace("{{char_example_dialogue}}", &self.prompts_example_dialogue)
            .replace("{{char_additional_example_dialogue}}", &prompts_additional_example_dialogue)
            .replace("{{char_scenario}}", &self.prompts_scenario)
            .replace("{{char_background_stories}}", &prompts_background_stories)
            .replace("{{char_behavior_traits}}", &prompts_behavior_traits)
            .replace("{{char_relationships}}", &prompts_relationships)
            .replace("{{char_skills_and_interests}}", &prompts_skills_and_interests)
            .replace("{{char_additional_info}}", &prompts_additional_info);

        Prompt::new_system(&p)
    }

    pub fn build_first_message(&self, user_name: &str) -> Prompt {
        let p = self.prompts_first_message
            .replace("{{user}}", user_name)
            .replace("{{char}}", &self.name);

        Prompt {
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: p,
            toolcall: None,
            created_at: 1,
        }
    }
}
