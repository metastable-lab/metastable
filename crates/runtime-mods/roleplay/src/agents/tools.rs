use metastable_runtime::LlmTool;
use metastable_database::TextCodecEnum;
use serde::{Deserialize, Serialize};

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "show_story_options",
    description = "向用户呈现故事选项以继续角色扮演。"
)]
pub struct ShowStoryOptions {
    #[llm_tool(description = "向用户呈现的用于继续故事的选项列表，内容也需要是中文。")]
    pub options: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, TextCodecEnum)]
#[text_codec(format = "colon", storage_lang = "zh", colon_char = "：")]
pub enum RoleplayMessageType {
    #[prefix(lang = "zh", content = "动作")]
    Action(String),
    #[prefix(lang = "zh", content = "场景")]
    Scenario(String),
    #[prefix(lang = "zh", content = "内心独白")]
    InnerThoughts(String),
    #[prefix(lang = "zh", content = "对话")]
    Chat(String),

    #[catch_all(no_prefix = true)]
    Text(String),
}

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "send_message",
    description = "用于向用户发送结构化消息的唯一工具。你必须使用此工具来发送所有回应，包括对话、动作、场景描述和选项。"
)]
pub struct SendMessage {
    #[llm_tool(description = "一个包含多个消息片段的数组，按顺序组合成完整的回复。", is_enum = true)]
    pub messages: Vec<RoleplayMessageType>,
    #[llm_tool(description = "一个包含多个选项的数组，按顺序组合成完整的回复。")]
    pub options: Vec<String>,
    #[llm_tool(description = "一个简短的总结，用于描述本次对话的要点。例如：“用户告诉我他想要去水族馆，我赞同了之后和她一起去了。")]
    pub summary: String,
}
