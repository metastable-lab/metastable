use serde::{Deserialize, Serialize};
use metastable_database::{TextCodecEnum, TextPromptCodec};

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

impl RoleplayMessageType {
    pub fn to_text(&self) -> String { self.to_lang("zh") }

    pub fn batch_to_text(msg: &[Self]) -> String {
        let mut text = String::new();
        for m in msg {
            text.push_str(&m.to_text());
            text.push('\n');
        }
        text
    }

    pub fn from_text_batch(text: &str) -> anyhow::Result<Vec<Self>> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.parse())
            .collect()
    }
}
