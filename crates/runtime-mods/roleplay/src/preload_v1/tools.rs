use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use metastable_runtime::{ExecutableFunctionCall, LLMRunResponse};

use crate::RoleplayMessageType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessagePartType {
    Action,
    Scenario,
    InnerThoughts,
    Chat,
    Text,
    Options,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePart {
    pub r#type: MessagePartType,
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageToolCall {
    pub messages: Vec<MessagePart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposedMessage {
    pub content_v1: Vec<RoleplayMessageType>,
    pub options: Vec<String>,
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for SendMessageToolCall {
    type CTX = ();
    type RETURN = ComposedMessage;

    fn name() -> &'static str {
        "send_message"
    }

    async fn execute(
        &self,
        _llm_response: &LLMRunResponse,
        _execution_context: &Self::CTX,
    ) -> Result<Self::RETURN> {
        let mut content_v1 = vec![];
        let mut options = vec![];

        for part in &self.messages {
            match part.r#type {
                MessagePartType::Action => {
                    if let Some(s) = part.content.as_str() {
                        content_v1.push(RoleplayMessageType::Action(s.to_string()));
                    }
                }
                MessagePartType::Scenario => {
                    if let Some(s) = part.content.as_str() {
                        content_v1.push(RoleplayMessageType::Scenario(s.to_string()));
                    }
                }
                MessagePartType::InnerThoughts => {
                    if let Some(s) = part.content.as_str() {
                        content_v1.push(RoleplayMessageType::InnerThoughts(s.to_string()));
                    }
                }
                MessagePartType::Chat => {
                    if let Some(s) = part.content.as_str() {
                        content_v1.push(RoleplayMessageType::Chat(s.to_string()));
                    }
                }
                MessagePartType::Text => {
                    if let Some(s) = part.content.as_str() {
                        content_v1.push(RoleplayMessageType::Text(s.to_string()));
                    }
                }
                MessagePartType::Options => {
                    if let Some(arr) = part.content.as_array() {
                        for opt_val in arr {
                            if let Some(opt_str) = opt_val.as_str() {
                                options.push(opt_str.to_string());
                            }
                        }
                    }
                }
            }
        }
        Ok(ComposedMessage {
            content_v1,
            options,
        })
    }
}
