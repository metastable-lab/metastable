use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_runtime::{ExecutableFunctionCall, LLMRunResponse};
use metastable_runtime_roleplay::{Character, CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeCharacterToolCall {
    pub name: String,
    pub description: String,
    pub gender: CharacterGender,
    pub language: CharacterLanguage,
    pub prompts_personality: String,
    pub prompts_scenario: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,
    pub prompts_background_stories: Vec<String>,
    pub prompts_behavior_traits: Vec<String>,
    pub tags: Vec<String>,
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for SummarizeCharacterToolCall {
    type CTX = ();
    type RETURN = Character;

    fn name() -> &'static str { "summarize_character" }

    async fn execute(&self, 
        llm_response: &LLMRunResponse, 
        _execution_context: &Self::CTX
    ) -> Result<Character> {
        let character = Character {
            id: Uuid::new_v4(),
            name: self.name.clone(),
            description: self.description.clone(),
            gender: self.gender.clone(),
            language: self.language.clone(),
            features: vec![CharacterFeature::Roleplay],
            prompts_scenario: self.prompts_scenario.clone(),
            prompts_personality: self.prompts_personality.clone(),
            prompts_example_dialogue: self.prompts_example_dialogue.clone(),
            prompts_first_message: self.prompts_first_message.clone(),
            prompts_background_stories: self.prompts_background_stories.clone(),
            prompts_behavior_traits: self.prompts_behavior_traits.clone(),
            tags: self.tags.clone(),
            creator: llm_response.caller.clone(),
            version: 1,
            status: CharacterStatus::Draft,
            creator_notes: None,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };
        Ok(character)
    }
}