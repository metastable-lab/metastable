use anyhow::{anyhow, Result};
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolType, FunctionCall
};
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;

use crate::{Message, MessageRole, MessageType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub role: MessageRole,
    pub content_type: MessageType,

    pub content: String,
    pub toolcall: Option<FunctionCall>,

    pub created_at: i64,
}

impl Prompt {
    pub fn validate_messages(messages: Vec<Self>) -> Result<Vec<Self>> {
        // 1. first message should be system message, and it should be the only system messages 
        if messages.len() == 0 {
            return Err(anyhow!("[Prompt::validate_messages_sequence] No messages to build input"));
        }
        let first_message = messages.first()
            .ok_or(anyhow!("[Prompt::validate_messages_sequence] No first message to build input"))?;
        if first_message.role != MessageRole::System {
            return Err(anyhow!("[Prompt::validate_messages_sequence] First message should be system message"));
        }
        // 2. there should be AT LEAST one messages other than the system messages
        if messages.len() == 1 {
            return Err(anyhow!("[Prompt::validate_messages_sequence] There should be AT LEAST one messages other than the system messages"));
        }
        // 3. the last message MUST be a user message
        let last_message = messages.last()
            .ok_or(anyhow!("[Prompt::validate_messages_sequence] No last message to build input"))?; // unexpected
        if last_message.role != MessageRole::User {
            return Err(anyhow!("[Prompt::validate_messages_sequence] Last message should be user message"));
        }
        Ok(messages)
    }

    pub fn validate_and_sort(mut messages: Vec<Self>) -> Result<Vec<Self>> {
        Self::validate_messages(messages.clone())?;
        // Order by created_at ascending, and for equal created_at, User first, then Assistant, then others
        messages.sort_by(|a, b| {
            match a.created_at.cmp(&b.created_at) {
                std::cmp::Ordering::Equal => {
                    // User < Assistant < others
                    let role_rank = |role: &MessageRole| match role {
                        MessageRole::User => 0,
                        MessageRole::Assistant => 1,
                        _ => 2,
                    };
                    role_rank(&a.role).cmp(&role_rank(&b.role))
                }
                ord => ord,
            }
        });
        Ok(messages)
    }

    pub fn pack_flat_messages(messages: Vec<Self>) -> Result<String> {
        let messages = Self::validate_and_sort(messages)?;
        Ok(messages.iter().map(|m| {
            format!("{}: {}", m.role, m.content)
        }).collect::<Vec<String>>().join("\n"))
    }

    pub fn pack(messages: Vec<Self>) -> Result<Vec<ChatCompletionRequestMessage>> {
        let messages = Self::validate_and_sort(messages)?;
        messages.iter().map(|m| {
            let maybe_toolcall = m.toolcall.as_ref().map(|toolcall| vec![ChatCompletionMessageToolCall {
                id: Uuid::new_v4().to_string(),
                r#type: ChatCompletionToolType::Function,
                function: toolcall.clone(),
            }]);
            let content = m.content.clone();

            Ok(match m.role {
                MessageRole::System => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(content)
                        .build()
                        .map_err(|e| anyhow!("[Prompt::pack] Failed to pack message: {}", e))?
                ),
                MessageRole::User => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(content)
                        .build()
                        .map_err(|e| anyhow!("[Prompt::pack] Failed to pack message: {}", e))?
                ),
                MessageRole::Assistant => {
                    let content = if let Some(toolcalls) = maybe_toolcall {
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(content)
                            .tool_calls(toolcalls)
                            .build()
                            .map_err(|e| anyhow!("[Prompt::pack] Failed to pack message: {}", e))?
                    } else {
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(content)
                            .build()
                            .map_err(|e| anyhow!("[Prompt::pack] Failed to pack message: {}", e))?
                    };
                    ChatCompletionRequestMessage::Assistant(content)
                },
                MessageRole::ToolCall => ChatCompletionRequestMessage::Tool(
                    ChatCompletionRequestToolMessageArgs::default()
                        .content(content)
                        .build()
                        .map_err(|e| anyhow!("[Prompt::pack] Failed to pack message: {}", e))?
                ),
            })
        }).collect()
    }

    pub fn new_system(prompt: &str) -> Self {
        Self {
            role: MessageRole::System,
            content_type: MessageType::Text,
            content: prompt.to_string(),
            toolcall: None,
            created_at: 0,
        }
    }

    pub fn new_user(prompt: &str) -> Self {
        Self {
            role: MessageRole::User,
            content_type: MessageType::Text,
            content: prompt.to_string(),
            toolcall: None,
            created_at: 0,
        }
    }

    pub fn from_message(message: &Message) -> [Self; 2] {
        [
            Self {
                role: MessageRole::User,
                content_type: message.user_message_content_type.clone(),
                content: message.user_message_content.clone(),
                toolcall: None,
                created_at: message.created_at,
            },
            Self {
                role: MessageRole::Assistant,
                content_type: message.assistant_message_content_type.clone(),
                content: message.assistant_message_content.clone(),
                toolcall: message.assistant_message_tool_call.0.clone(),
                created_at: message.created_at,
            }
        ]
    }

    pub fn empty() -> Self {
        Self {
            role: MessageRole::User,
            content_type: MessageType::Text,
            content: "".to_string(),
            toolcall: None,
            created_at: 0,
        }
    }
}