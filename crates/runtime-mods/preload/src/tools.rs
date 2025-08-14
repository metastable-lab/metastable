use metastable_runtime::LlmTool;
use metastable_runtime_roleplay::RoleplayMessageType;
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
        enum_lang = "zh"
    )]
    pub background_stories: Vec<BackgroundStories>,
    #[llm_tool(
        description = "行为特征条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh"
    )]
    pub behavior_traits: Vec<BehaviorTraits>,
    #[llm_tool(
        description = "人际关系条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh"
    )]
    pub relationships: Vec<Relationships>,
    #[llm_tool(
        description = "技能与兴趣条目。严格对象格式：{ type: 中文前缀, content: 值 }。",
        enum_lang = "zh"
    )]
    pub skills_and_interests: Vec<SkillsAndInterests>,
    #[llm_tool(description = "追加对话风格示例（多条）。")]
    pub additional_example_dialogue: Option<Vec<String>>,
    #[llm_tool(description = "任何无法归类但很重要的信息，以中文句子表达。")]
    pub additional_info: Option<Vec<String>>,
    #[llm_tool(description = "描述角色特点的标签，便于搜索和分类。")]
    pub tags: Vec<String>,
}

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "show_story_options",
    description = "向用户呈现故事选项以继续角色扮演。"
)]
pub struct ShowStoryOptions {
    #[llm_tool(description = "向用户呈现的用于继续故事的选项列表，内容也需要是中文。")]
    pub options: Vec<String>,
}

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "send_message",
    description = "用于向用户发送结构化消息的唯一工具。你必须使用此工具来发送所有回应，包括对话、动作、场景描述和选项。"
)]
pub struct SendMessage {
    #[llm_tool(description = "一个包含多个消息片段的数组，按顺序组合成完整的回复。")]
    pub messages: Vec<RoleplayMessageType>,
    #[llm_tool(description = "一个包含多个选项的数组，按顺序组合成完整的回复。")]
    pub options: Vec<String>,
}
