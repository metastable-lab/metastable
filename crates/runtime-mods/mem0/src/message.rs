use sqlx::types::Uuid;
use voda_runtime::{Message, MessageRole, MessageType};

#[derive(Debug, Clone, PartialEq)]
pub struct Mem0Messages {
    pub id: Uuid,

    pub user_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub content_type: MessageType,
    pub role: MessageRole,

    pub content: String,

    pub created_at: i64,
    pub updated_at: i64,
}

impl Message for Mem0Messages {
    fn id(&self) -> &Uuid { &self.id }

    fn role(&self) -> &MessageRole { &self.role }
    fn owner(&self) -> &Uuid { &self.user_id }
    
    fn content_type(&self) -> &MessageType { &self.content_type }
    fn text_content(&self) -> Option<String> { Some(self.content.clone()) }
    fn binary_content(&self) -> Option<Vec<u8>> { None }
    fn url_content(&self) -> Option<String> { None }

    fn created_at(&self) -> i64 { self.created_at }
}