use anyhow::Result;
use async_openai::types::{FunctionCall, FunctionObject};
use serde::{Deserialize, Serialize};
use serde_json::json;

use voda_runtime::ExecutableFunctionCall;
use crate::llm::LlmConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryToolcall {
    pub answer: String,
}

pub fn get_extract_memory_config(_user_id: String, conversation_data: String, memories: &Vec<String>) -> (LlmConfig, String) {
    let memories_text = if memories.is_empty() {
        "No memories available.".to_string()
    } else {
        memories.join("\n")
    };

    let system_prompt = format!(
        r#"Answer questions based strictly on the following memories. If no relevant memories exist, respond with "I don't know."

Memories:
{}

Rules:
- Only use information from memories.
- For food preferences, look for words like: likes, loves, favorite, enjoys, hates, dislikes.
- If asked to compare or infer (e.g., "which do I prefer?"), say "I can't compare."

Examples:
Memories: ["Likes pizza", "Hates broccoli"]
Q: What do I like? A: I like pizza.
Q: What do I hate? A: I hate broccoli.
Q: Do I like sushi? A: I don't know."#,
        memories_text
    );  

    let user_prompt = json!({
        "role": "user",
        "content": conversation_data
    }).to_string();

    let extract_memory_tool = FunctionObject {
        name: "extract_memory".to_string(),
        description: Some("Provide accurate and concise answers based on the provided memories.".to_string()),
        parameters: Some(json!({
            "type": "object",
            "properties": {
                "answer": {
                    "type": "string",
                    "description": "A clear and concise answer to the question based on the memories."
                }
            },
            "required": ["answer"],
            "additionalProperties": false,
        })),
        strict: Some(true),
    };

    let tools = vec![extract_memory_tool];

    let config = LlmConfig {
        model: "mistralai/ministral-8b".to_string(),
        temperature: 0.3,
        max_tokens: 5000,
        system_prompt,
        tools,
    };

    (config, user_prompt)
}

impl ExecutableFunctionCall for MemoryToolcall {
    fn name() -> &'static str {
        "extract_memory"
    }

    fn from_function_call(function_call: FunctionCall) -> Result<Self> {
        let args: serde_json::Value = serde_json::from_str(&function_call.arguments)?;
        let answer = args["answer"].as_str().unwrap_or("").to_string();
        Ok(Self { answer })
    }

    async fn execute(&self) -> Result<String> {
        Ok(self.answer.clone())
    }
}

