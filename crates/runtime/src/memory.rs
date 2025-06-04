use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, 
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, 
    ChatCompletionRequestUserMessageArgs
};
use serde::{Deserialize, Serialize};
use voda_common::CryptoHash;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    #[default]
    User,
    Assistant,
    ToolCall,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    #[default]
    Text,
    Image,
    Audio,
}

pub trait Message: Clone + Send + Sync + 'static {
    fn role(&self) -> &MessageRole;
    fn owner(&self) -> &CryptoHash;

    fn content_type(&self) -> &MessageType;
    fn text_content(&self) -> Option<String>;
    fn binary_content(&self) -> Option<Vec<u8>>;
    fn url_content(&self) -> Option<String>;

    fn created_at(&self) -> u64;

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

    async fn add_message(&self, message: &Self::MessageType) -> Result<()>;

    async fn get_one(&self, memory_id: &CryptoHash) -> Result<Option<Self::MessageType>>;
    async fn get_all(
        &self, user_id: &CryptoHash,
        limit: u64, offset: u64
    ) -> Result<Vec<Self::MessageType>>;

    async fn search(&self, message: &Self::MessageType, limit: u64, offset: u64) -> Result<
        Vec<Self::MessageType>
    >;

    async fn update(&self, memory_id: &CryptoHash, message: &Self::MessageType) -> Result<()>;

    async fn delete(&self, memory_id: &CryptoHash) -> Result<()>;
    async fn reset(&self, user_id: &CryptoHash) -> Result<()>;
}
