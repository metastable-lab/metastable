use async_openai::types::FunctionObject;
use serde_json::json;
use sqlx::types::{Json, Uuid};
use voda_common::get_current_timestamp;
use voda_runtime::SystemConfig;

pub fn get_system_configs_for_char_creation() -> SystemConfig {
    let functions = vec![
        FunctionObject {
            name: "summarize_character".to_string(),
            description: Some("根据与用户的对话，总结并创建一个完整的角色档案。".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "角色的名字" },
                    "description": { "type": "string", "description": "对角色的一段简短描述，包括其核心身份、外貌特点等。" },
                    "gender": { "type": "string", "enum": ["Male", "Female", "Multiple", "Others"], "description": "角色的性别" },
                    "language": { "type": "string", "enum": ["English", "Chinese", "Japanese", "Korean", "Others"], "description": "角色的主要使用语言" },
                    "prompts_personality": { "type": "string", "description": "描述角色的性格特点。例如：热情、冷漠、幽默、严肃等。" },
                    "prompts_scenario": { "type": "string", "description": "角色所处的典型场景或背景故事。这会影响角色扮演的开场。" },
                    "prompts_example_dialogue": { "type": "string", "description": "一段示例对话，展示角色的说话风格和语气。" },
                    "prompts_first_message": { "type": "string", "description": "角色在对话开始时会说的第一句话。" },
                    "prompts_background_stories": { "type": "array", "items": { "type": "string" }, "description": "角色的背景故事，可以是多个故事片段。" },
                    "prompts_behavior_traits": { "type": "array", "items": { "type": "string" }, "description": "角色的行为特点或习惯。例如：喜欢喝茶、紧张时会挠头等。" },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": "描述角色特点的标签，便于搜索和分类。" }
                },
                "required": ["name", "description", "gender", "language", "prompts_personality", "prompts_scenario", "prompts_example_dialogue", "prompts_first_message"]
            }).into()),
            strict: Some(true),
        }
    ];

    SystemConfig {
        id: Uuid::new_v4(),
        name: "character_creation_v0".to_string(),
        system_prompt: r#"你是一个专门用于分析对话并创建角色档案的AI助手。
你的任务是接收一段完整的对话记录，这段对话是关于构思一个新角色的。
你需要仔细阅读整段对话，从中提取并整合所有关于角色的关键信息。

**核心任务**:
1.  **全面分析**: 阅读并理解提供的完整对话内容。
2.  **信息提取**: 从对话中识别并提取角色的所有相关信息，包括但不限于：姓名、描述、性别、语言、性格、场景/背景、示例对话、第一句话、背景故事、行为特点和标签。
3.  **调用工具**: 在分析和提取完所有信息后，你 **必须** 调用 `summarize_character` 函数，并将提取出的所有信息作为参数，以生成最终的角色档案。你不需要与用户进行任何额外的对话或确认。这是一个一次性任务。"#.to_string(),
        system_prompt_version: 3,
        openai_base_url: "https://openrouter.ai/api/v1".to_string(),
        openai_model: "deepseek/deepseek-r1-0528:free".to_string(),
        openai_temperature: 0.7,
        openai_max_tokens: 2500,
        functions: Json(functions),
        updated_at: get_current_timestamp(),
        created_at: get_current_timestamp(),
    }
}