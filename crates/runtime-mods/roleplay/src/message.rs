use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use metastable_common::get_time_in_utc8;
use metastable_database::SqlxObject;
use metastable_runtime::{Message, MessageRole, MessageType, SystemConfig, User};
use metastable_runtime_mem0::Mem0Messages;

use super::{
    BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests, Character, 
    RoleplayMessageType, RoleplaySession
};

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "roleplay_messages"]
pub struct RoleplayMessage {
    pub id: Uuid,

    #[foreign_key(referenced_table = "roleplay_sessions", related_rust_type = "RoleplaySession")]
    pub session_id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub owner: Uuid,

    pub role: MessageRole,
    pub content_type: MessageType,

    pub content: String,
    pub content_v1: Vec<RoleplayMessageType>,
    pub options: Vec<String>,

    pub is_saved_in_memory: bool,
    pub is_removed: bool,

    pub created_at: i64,
    pub updated_at: i64,
}

impl Message for RoleplayMessage {
    fn id(&self) -> &Uuid { &self.id }

    fn role(&self) -> &MessageRole { &self.role }
    fn owner(&self) -> &Uuid { &self.owner }
    
    fn content_type(&self) -> &MessageType { &self.content_type }
    fn text_content(&self) -> Option<String> {
        if self.content.is_empty() {
            Some(RoleplayMessageType::batch_to_text(&self.content_v1))
        } else {
            Some(self.content.clone())
        }
    }
    fn binary_content(&self) -> Option<Vec<u8>> { None }
    fn url_content(&self) -> Option<String> { None }

    fn created_at(&self) -> i64 { self.created_at }
}

impl RoleplayMessage {
    fn replace_placeholders(
        text: &str, character_name: &str, user_name: &str,
    ) -> String {
        text.replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
    }
    
    fn replace_placeholders_system_prompt(
        character_name: &str, user_name: &str,
        system_prompt: &str,
        character_personality: &str,
        character_example_dialogue: &str,
        character_additional_example_dialogue: &Vec<String>,
        character_scenario: &str,
        character_background_stories: &Vec<BackgroundStories>,
        character_behavior_traits: &Vec<BehaviorTraits>,
        character_relationships: &Vec<Relationships>,
        character_skills_and_interests: &Vec<SkillsAndInterests>,
        character_additional_info: &Vec<String>,
        request_time: &str,
    ) -> String {
        let character_personality = Self::replace_placeholders(character_personality, character_name, user_name);
        let character_example_dialogue = Self::replace_placeholders(character_example_dialogue, character_name, user_name);
        let character_additional_example_dialogue = character_additional_example_dialogue
            .iter()
            .map(|v| Self::replace_placeholders(v, character_name, user_name))
            .collect::<Vec<_>>()
            .join("\n- ");
        let character_scenario = Self::replace_placeholders(character_scenario, character_name, user_name);
        let character_background_stories = character_background_stories
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");
        let character_behavior_traits = character_behavior_traits
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");
        let character_relationships = character_relationships
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");
        let character_skills_and_interests = character_skills_and_interests
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join("\n- ");
        let character_additional_info = character_additional_info
            .iter()
            .map(|v| Self::replace_placeholders(v, character_name, user_name))
            .collect::<Vec<_>>()
            .join("\n- ");
    
        let system_prompt = system_prompt
            .replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
            .replace("{{char_personality}}", &character_personality)
            .replace("{{char_example_dialogue}}", &character_example_dialogue)
            .replace("{{char_additional_example_dialogue}}", &character_additional_example_dialogue)
            .replace("{{char_scenario}}", &character_scenario)
            .replace("{{char_background_stories}}", &character_background_stories)
            .replace("{{char_behavior_traits}}", &character_behavior_traits)
            .replace("{{char_relationships}}", &character_relationships)
            .replace("{{char_skills_and_interests}}", &character_skills_and_interests)
            .replace("{{char_additional_info}}", &character_additional_info)
            .replace("{{request_time}}", request_time);
    
        system_prompt
    }

    pub fn system(
        session: &RoleplaySession, system_config: &SystemConfig, character: &Character, user: &User,
    ) -> Self {
        let request_time = get_time_in_utc8();
        let system_prompt = Self::replace_placeholders_system_prompt(
            &character.name,
            &user.user_aka,
            &system_config.system_prompt,
            &character.prompts_personality,
            &character.prompts_example_dialogue,
            &character.prompts_additional_example_dialogue,
            &character.prompts_scenario,
            &character.prompts_background_stories,
            &character.prompts_behavior_traits,
            &character.prompts_relationships,
            &character.prompts_skills_and_interests,
            &character.prompts_additional_info,
            &request_time,
        );

        Self {
            id: Uuid::default(),
            owner: user.id.clone(),
            role: MessageRole::System,
            content_type: MessageType::Text,
            
            content: system_prompt,
            content_v1: vec![],
            options: vec![],
            is_saved_in_memory: false,
            is_removed: false,

            session_id: session.id.clone(),

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn first_message(
        session: &RoleplaySession, character: &Character, user: &User
    ) -> Self {
        let first_message = Self::replace_placeholders(
            &character.prompts_first_message, 
            &character.name, 
            &user.user_aka
        );

        Self {
            id: Uuid::default(),
            owner: user.id.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: first_message,
            content_v1: vec![],
            options: vec![],
            is_saved_in_memory: false,
            is_removed: false,

            session_id: session.id.clone(),

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn user_message(
        message: &str, session_id: &Uuid, user_id: &Uuid
    ) -> Self {
        Self {
            id: Uuid::default(),
            owner: user_id.clone(),
            role: MessageRole::User,
            content_type: MessageType::Text,
            
            content: message.to_string(),
            content_v1: vec![],
            options: vec![],
            is_saved_in_memory: false,
            is_removed: false,
            session_id: session_id.clone(),

            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn from_mem0_messages(
        session_id: &Uuid,
        memory_message: &Mem0Messages,
        relationship_message: &Mem0Messages,
    ) -> Self {
        let content = format!(r#"
记忆片段：
{}
关系片段：
{}
"#, memory_message.content, relationship_message.content);
        
        Self {
            id: Uuid::default(),
            owner: memory_message.user_id,
            role: MessageRole::User,
            content_type: MessageType::Text,
            options: vec![],
            content,
            content_v1: vec![],
            is_saved_in_memory: false,
            is_removed: false,
            session_id: session_id.clone(),
            created_at: memory_message.created_at,
            updated_at: memory_message.updated_at,
        }
    }
}
