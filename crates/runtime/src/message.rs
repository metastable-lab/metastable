use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, 
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, 
    ChatCompletionRequestUserMessageArgs
};
use serde::{Deserialize, Serialize};
use metastable_database::TextCodecEnum;

use sqlx::types::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone, Default, TextCodecEnum, PartialEq, Eq)]
#[text_codec(format = "paren", storage_lang = "en")]
pub enum MessageRole {
    System,

    #[default]
    User,

    Assistant,
    ToolCall,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, TextCodecEnum, PartialEq, Eq)]
#[text_codec(format = "paren", storage_lang = "en")]
pub enum MessageType {
    #[default]
    Text,

    Image(String),
    Audio(String),
}

pub trait Message: Clone + Send + Sync + 'static {
    fn id(&self) -> &Uuid;

    fn role(&self) -> &MessageRole;
    fn owner(&self) -> &Uuid;

    fn content_type(&self) -> &MessageType;
    fn content(&self) -> Option<String>;

    fn pack(message: &[Self]) -> Result<Vec<ChatCompletionRequestMessage>> {
        message
            .iter()
            .map(|m| {
                Ok(match m.role() {
                    MessageRole::System => ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(m.content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::User => ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(m.content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::Assistant => ChatCompletionRequestMessage::Assistant(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(m.content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::ToolCall => ChatCompletionRequestMessage::Tool(
                        ChatCompletionRequestToolMessageArgs::default()
                            .content(m.content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                })
            })
            .collect()
    }

    fn pack_flat_messages(messages: &[Self]) -> Result<String> {
        let mut flat_messages = Vec::new();
        for message in messages {
            match message.role() {
                MessageRole::System => {
                    flat_messages.push(format!("system: {}", message.content().unwrap_or_default()));
                }
                MessageRole::User => {
                    flat_messages.push(format!("user: {}", message.content().unwrap_or_default()));
                }
                MessageRole::Assistant => {
                    flat_messages.push(format!("assistant: {}", message.content().unwrap_or_default()));
                }
                MessageRole::ToolCall => {
                    flat_messages.push(format!("tool_call: {}", message.content().unwrap_or_default()));
                }
            }
        }
        Ok(flat_messages.join("\n"))
    }
}
