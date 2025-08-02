use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_runtime::{ExecutableFunctionCall, LLMRunResponse};

use crate::RoleplayMessageType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "content", rename_all = "camelCase")]
pub enum MessagePart {
    Action(String),
    Scenario(String),
    InnerThoughts(String),
    Chat(String),
    Text(String),
    Options(Vec<String>),
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
            match part {
                MessagePart::Action(s) => {
                    content_v1.push(RoleplayMessageType::Action(s.clone()));
                }
                MessagePart::Scenario(s) => {
                    content_v1.push(RoleplayMessageType::Scenario(s.clone()));
                }
                MessagePart::InnerThoughts(s) => {
                    content_v1.push(RoleplayMessageType::InnerThoughts(s.clone()));
                }
                MessagePart::Chat(s) => {
                    content_v1.push(RoleplayMessageType::Chat(s.clone()));
                }
                MessagePart::Text(s) => {
                    content_v1.push(RoleplayMessageType::Text(s.clone()));
                }
                MessagePart::Options(opts) => {
                    options.extend(opts.clone());
                }
            }
        }

        Ok(ComposedMessage {
            content_v1,
            options,
        })
    }
}
