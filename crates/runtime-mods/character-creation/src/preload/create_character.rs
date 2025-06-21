use anyhow::Result;
use async_openai::types::FunctionCall;
use serde::{Deserialize, Serialize};
use voda_runtime::ExecutableFunctionCall;
use voda_runtime_roleplay::{CharacterGender, CharacterLanguage};

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
        Ok(serde_json::from_str(&function_call.arguments)?)
    }

    async fn execute(&self) -> Result<String> {
        Ok("Character created successfully".to_string())
    }
}