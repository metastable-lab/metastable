use metastable_llm_macros::LlmTool;
use metastable_runtime_roleplay::{
    BackgroundStories, BehaviorTraits, CharacterGender, CharacterLanguage, Relationships,
    SkillsAndInterests,
};

use serde::{Deserialize, Serialize};

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "summarize_character",
    description = "根据与用户的对话，总结并创建一个完整的角色档案。"
)]
pub struct SummarizeCharacter {
    #[llm_tool(description = "角色的名字")]
    pub name: String,
    #[llm_tool(description = "对角色的一段简短描述，包括其核心身份、外貌特点等。")]
    pub description: String,
    #[llm_tool(description = "角色的性别")]
    pub gender: CharacterGender,
    #[llm_tool(description = "角色的主要使用语言")]
    pub language: CharacterLanguage,
    #[llm_tool(description = "描述角色的性格特点。例如：热情、冷漠、幽默、严肃等。")]
    pub prompts_personality: String,
    #[llm_tool(description = "角色所处的典型场景或背景故事。这会影响角色扮演的开场。")]
    pub prompts_scenario: String,
    #[llm_tool(description = "一段示例对话，展示角色的说话风格和语气。")]
    pub prompts_example_dialogue: String,
    #[llm_tool(description = "角色在对话开始时会说的第一句话。")]
    pub prompts_first_message: String,
    #[llm_tool(
        description = "背景故事条目。严格对象格式：{ type:  中文前缀, content: 值 }。type 只能取以下之一。",
        enum_lang = "zh",
        custom_parser = true
    )]
    pub background_stories: Vec<BackgroundStories>,
    #[llm_tool(
        description = "行为特征条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh",
        custom_parser = true
    )]
    pub behavior_traits: Vec<BehaviorTraits>,
    #[llm_tool(
        description = "人际关系条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh",
        custom_parser = true
    )]
    pub relationships: Vec<Relationships>,
    #[llm_tool(
        description = "技能与兴趣条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh",
        custom_parser = true
    )]
    pub skills_and_interests: Vec<SkillsAndInterests>,
    #[llm_tool(description = "追加对话风格示例（多条）。")]
    pub additional_example_dialogue: Option<Vec<String>>,
    #[llm_tool(description = "任何无法归类但很重要的信息，以中文句子表达。")]
    pub additional_info: Option<Vec<String>>,
    #[llm_tool(description = "描述角色特点的标签，便于搜索和分类。")]
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::types::{
        ChatCompletionMessageToolCall, FunctionObject,
    };
    use serde_json::json;

    #[tokio::test]
    async fn test_summarize_character_function_object() {
        let generated_function = SummarizeCharacter::to_function_object();

        let expected_json = json!({
          "name": "summarize_character",
          "description": "根据与用户的对话，总结并创建一个完整的角色档案。",
          "parameters": {
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
                    "background_stories": {
                        "type": "array",
                        "description": "背景故事条目。严格对象格式：{ type:  中文前缀, content: 值 }。type 只能取以下之一。",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": [
                                    "职业", "童年经历", "成长环境", "重大经历", "价值观", "过去的遗憾或创伤，无法释怀的事", "梦想，渴望的事情，追求的事情", "Others"
                                ]},
                                "content": { "type": "string" }
                            },
                            "required": ["type", "content"]
                        }
                    },
                    "behavior_traits": {
                        "type": "array",
                        "description": "行为特征条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": [
                                    "行为举止", "外貌特征", "穿搭风格", "情绪表达方式", "个人沟通习惯", "与用户的沟通习惯", "个人行为特征", "与用户的沟通特征", "Others"
                                ]},
                                "content": { "type": "string" }
                            },
                            "required": ["type", "content"]
                        }
                    },
                    "relationships": {
                        "type": "array",
                        "description": "人际关系条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": [
                                    "亲密伴侣", "家庭", "朋友", "敌人", "社交圈", "Others"
                                ]},
                                "content": { "type": "string" }
                            },
                            "required": ["type", "content"]
                        }
                    },
                    "skills_and_interests": {
                        "type": "array",
                        "description": "技能与兴趣条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string", "enum": [
                                    "职业技能", "生活技能", "兴趣爱好", "弱点，不擅长的领域", "优点，擅长的事情", "内心矛盾冲突", "性癖", "Others"
                                ]},
                                "content": { "type": "string" }
                            },
                            "required": ["type", "content"]
                        }
                    },
                    "additional_example_dialogue": { "type": "array", "items": { "type": "string" }, "description": "追加对话风格示例（多条）。" },
                    "additional_info": { "type": "array", "items": { "type": "string" }, "description": "任何无法归类但很重要的信息，以中文句子表达。" },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": "描述角色特点的标签，便于搜索和分类。" }
                },
                "required": [
                    "name", "description", "gender", "language",
                    "prompts_personality", "prompts_scenario", "prompts_example_dialogue", "prompts_first_message",
                    "background_stories", "behavior_traits", "relationships", "skills_and_interests", "tags"
                ]
          },
          "strict": true
        });

        let expected_function: FunctionObject = serde_json::from_value(expected_json).unwrap();

        let generated_function_json = serde_json::to_value(&generated_function).unwrap();
        let expected_function_json = serde_json::to_value(&expected_function).unwrap();

        println!("generated_function_json: {}", generated_function_json.to_string());
        println!("expected_function_json: {}", expected_function_json.to_string());

        assert_eq!(generated_function_json, expected_function_json);
    }

    #[tokio::test]
    async fn test_parse_tool_call() {
        let tool_call_json = json!({
            "id": "call_123",
            "type": "function",
            "function": {
                "name": "summarize_character",
                "arguments": "{\n  \"name\": \"艾拉\",\n  \"description\": \"一位充满活力的年轻探险家，总是渴望发现新奇事物。\",\n  \"gender\": \"Female\",\n  \"language\": \"Chinese\",\n  \"prompts_personality\": \"热情、好奇、勇敢\",\n  \"prompts_scenario\": \"在一个古老的森林里寻找传说中的遗迹。\",\n  \"prompts_example_dialogue\": \"哇，你看那边！那是什么？我们快去看看！\",\n  \"prompts_first_message\": \"你好，我是艾拉，你愿意和我一起去冒险吗？\",\n  \"background_stories\": [\n    { \"type\": \"职业\", \"content\": \"探险家\" },\n    { \"type\": \"成长环境\", \"content\": \"在一个充满冒险故事的家庭长大\" }\n  ],\n  \"behavior_traits\": [\n    { \"type\": \"行为举止\", \"content\": \"总是充满活力，喜欢跑跑跳跳\" }\n  ],\n  \"relationships\": [\n    { \"type\": \"朋友\", \"content\": \"与各种各样的生物都能成为朋友\" }\n  ],\n  \"skills_and_interests\": [\n    { \"type\": \"兴趣爱好\", \"content\": \"收集各种奇特的石头和植物\" }\n  ],\n  \"additional_example_dialogue\": [],\n  \"additional_info\": [],\n  \"tags\": [\"探险\", \"年轻\", \"女性\"] \n}"
            }
        });

        let tool_call: ChatCompletionMessageToolCall =
            serde_json::from_value(tool_call_json).unwrap();

        let summary = SummarizeCharacter::try_from_tool_call(&tool_call).unwrap();

        assert_eq!(summary.name, "艾拉");
        assert_eq!(summary.gender, CharacterGender::Female);
        assert_eq!(summary.language, CharacterLanguage::Chinese);
        assert_eq!(
            summary.background_stories,
            vec![
                BackgroundStories::Professions("探险家".to_string()),
                BackgroundStories::GrowthEnvironment("在一个充满冒险故事的家庭长大".to_string())
            ]
        );
    }
}
