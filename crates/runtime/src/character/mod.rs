mod audit;
mod character_detail;
mod character_history;
mod character_sub;
mod character_mask;
mod character_post;
mod post_comments;

use anyhow::Result;
use async_openai::types::FunctionCall;
use metastable_common::get_time_in_utc8;
use serde::{Deserialize, Serialize};
use metastable_database::SqlxObject;
use sqlx::types::Json;
use sqlx::types::Uuid;

use crate::{Message, MessageRole, MessageType, Prompt, User};

pub use character_detail::{
    BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests,
    CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus, CharacterOrientation,
};

pub use audit::AuditLog;
pub use character_history::CharacterHistory;
pub use character_sub::CharacterSub;
pub use character_mask::CharacterMask;
pub use character_post::CharacterPost;
pub use post_comments::CharacterPostComments;

use crate::ChatSession;

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "roleplay_characters"]
#[allow_type_change]
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
        let p = self.prompts_first_message.0.clone()
            .unwrap_or(FunctionCall { name: "send_message".to_string(), arguments: "{}".to_string() });

        let p_arguments = p.arguments
            .replace("{{char}}", &self.name)
            .replace("{{user}}", user_name);
        
        let p_toolcall = FunctionCall { name: p.name, arguments: p_arguments };

        Prompt {
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: "".to_string(),
            toolcall: Some(p_toolcall),
            created_at: 1,
        }
    }
}
