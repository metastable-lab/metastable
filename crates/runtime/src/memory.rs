use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, 
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, 
    ChatCompletionRequestUserMessageArgs
};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use voda_common::CryptoHash;
use crate::LLMRunResponse;

#[derive(Debug, Serialize, Deserialize, Clone, Default, Display, EnumString, PartialEq, Eq)]
pub enum MessageRole {
    System,

    #[default]
    User,

    Assistant,
    ToolCall,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, Display, EnumString, PartialEq, Eq)]
pub enum MessageType {
    #[default]
    Text,

    Image(String),
    Audio(String),
}

pub trait Message: Clone + Send + Sync + 'static {
    fn id(&self) -> &CryptoHash;

    fn role(&self) -> &MessageRole;
    fn owner(&self) -> &CryptoHash;

    fn content_type(&self) -> &MessageType;
    fn text_content(&self) -> Option<String>;
    fn binary_content(&self) -> Option<Vec<u8>>;
    fn url_content(&self) -> Option<String>;

    fn created_at(&self) -> i64;

    fn from_llm_response(response: LLMRunResponse, session_id: &CryptoHash, user_id: &CryptoHash) -> Self;
    fn pack(message: &[Self]) -> Result<Vec<ChatCompletionRequestMessage>> {
        message
            .iter()
            .map(|m| {
                Ok(match m.role() {
                    MessageRole::System => ChatCompletionRequestMessage::System(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(m.text_content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::User => ChatCompletionRequestMessage::User(
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(m.text_content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::Assistant => ChatCompletionRequestMessage::Assistant(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(m.text_content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                    MessageRole::ToolCall => ChatCompletionRequestMessage::Tool(
                        ChatCompletionRequestToolMessageArgs::default()
                            .content(m.text_content().unwrap_or_default())
                            .build()
                            .map_err(|e| anyhow!("Failed to pack message: {}", e))?
                    ),
                })
            })
            .collect()
    }
}

#[async_trait::async_trait]
pub trait Memory: Clone + Send + Sync + 'static {
    type MessageType: Message;

    async fn initialize(&self) -> Result<()>;

    async fn add_messages(&self, messages: &[Self::MessageType]) -> Result<()>;
    async fn get_one(&self, message_id: &CryptoHash) -> Result<Option<Self::MessageType>>;
    async fn get_all(
        &self, user_id: &CryptoHash, limit: u64, offset: u64
    ) -> Result<Vec<Self::MessageType>>;

    async fn search(&self, message: &Self::MessageType, limit: u64, offset: u64) -> Result<
        Vec<Self::MessageType>
    >;

    async fn update(&self, messages: &[Self::MessageType]) -> Result<()>;
    async fn delete(&self, message_ids: &[CryptoHash]) -> Result<()>;

    async fn reset(&self, user_id: &CryptoHash) -> Result<()>;
}
