use sqlx::types::Uuid;
use metastable_runtime::{Message, MessageRole, MessageType};

#[derive(Debug, Clone, PartialEq)]
pub struct Mem0Messages {
    pub id: Uuid,

    pub user_id: Uuid,
    pub character_id: Option<Uuid>,
    pub session_id: Option<Uuid>,

    pub user_aka: String,

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
    fn content(&self) -> Option<String> { Some(self.content.clone()) }
}
