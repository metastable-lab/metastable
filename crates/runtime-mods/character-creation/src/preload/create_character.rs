use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use voda_common::get_current_timestamp;
use voda_runtime::ExecutableFunctionCall;
use voda_runtime_roleplay::{Character, CharacterFeature, CharacterGender, CharacterLanguage, CharacterStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeCharacterFunctionCall {
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

impl ExecutableFunctionCall for SummarizeCharacterFunctionCall {
    fn name() -> &'static str {
        "summarize_character"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        println!("function_call: {:?}", function_call);
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        let character = Character {
            id: Uuid::new_v4(),
            name: self.name.clone(),
            description: self.description.clone(),
            gender: self.gender.clone(),
            language: self.language.clone(),
            features: vec![CharacterFeature::DefaultRoleplay],
            prompts_scenario: self.prompts_scenario.clone(),
            prompts_personality: self.prompts_personality.clone(),
            prompts_example_dialogue: self.prompts_example_dialogue.clone(),
            prompts_first_message: self.prompts_first_message.clone(),
            prompts_background_stories: self.prompts_background_stories.clone(),
            prompts_behavior_traits: self.prompts_behavior_traits.clone(),
            tags: self.tags.clone(),
            creator: Uuid::new_v4(),
            version: 1,
            status: CharacterStatus::Draft,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };

        Ok(serde_json::to_string(&character)?)
    }
}