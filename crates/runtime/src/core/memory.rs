use std::collections::HashMap;

use anyhow::Result;
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HistoryMessage {
    pub owner: CryptoHash,
    pub character_id: CryptoHash,

    pub role: MessageRole,
    pub content_type: MessageType,
    
    pub content: String
}

pub type HistoryMessagePair = (HistoryMessage, HistoryMessage);

#[allow(async_fn_in_trait)]
pub trait Memory<DB>: Sized {
    async fn save_memory(db: &DB, messages: Self) -> Result<()>;
    async fn load_memory_by_id(db: &DB, id: &CryptoHash) -> Result<Option<Self>>;
    
    // get all conversations of a character of a user
    async fn load_memory_by_character_and_owner(
        db: &DB, 
        character: &CryptoHash, owner: &CryptoHash,
        limit: Option<usize>
    ) -> Result<Vec<Self>>;

    // get a list of all characters a user interacted with, with the number of conversations
    async fn load_character_list_of_user(
        db: &DB, owner: &CryptoHash
    ) -> Result<HashMap<CryptoHash, usize>>;
}