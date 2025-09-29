use anyhow::{anyhow, Result};
use async_openai::types::{CreateChatCompletionRequestArgs, FunctionCall};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::{Json, Uuid};

use metastable_common::ModuleClient;
use metastable_clients::{R2Client, ImageFolder};

use crate::{Agent, Message, MessageType, Prompt, ToolCall, llm_request::{ExtendedChatCompletionRequest, ReasoningConfig}};

// Clean image response types
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageObject {
    #[serde(rename = "type")]
    pub image_type: String,
    pub image_url: ImageUrl,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub images: Option<Vec<ImageObject>>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageChoice {
    pub message: ImageResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ImageResponse {
    pub choices: Vec<ImageChoice>,
    pub usage: Option<async_openai::types::CompletionUsage>,
}

// Virtual tool call result
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerateImageResult {
    pub image_urls: Vec<String>,
    pub description: String,
}

#[async_trait::async_trait]
pub trait ImageAgent: Agent {
    fn r2_client(&self) -> &R2Client;

    async fn generate_image(
        &self,
        caller: &Uuid,
        input: &Self::Input
    ) -> Result<(Message, Self::Tool, Option<Value>)> {
        tracing::debug!("[ImageAgent::generate_image] Generating image for: {}", Self::SYSTEM_CONFIG_NAME);

        // Build and validate messages
        let messages = self.build_input(input).await?;
        let messages = Prompt::sort(messages)?;
        let messages = Prompt::validate_messages(messages)?;
        let user_message = messages.last().expect("already validated").clone();

        // Create request
        let llm_messages = Prompt::pack(messages)?;
        let base_request = CreateChatCompletionRequestArgs::default()
            .model(Self::model())
            .messages(llm_messages)
            .temperature(Self::temperature())
            .max_tokens(Self::max_tokens() as u32)
            .build()?;

        let request = ExtendedChatCompletionRequest {
            base: base_request,
            reasoning: Self::reasoning_effort().map(|effort| ReasoningConfig {
                effort: effort.to_string(),
            }),
            modalities: Some(vec!["image".to_string(), "text".to_string()]),
        };

        // Make API call
        use async_openai::config::Config;
        let client = reqwest::Client::new();
        let config = self.llm_client().get_client().config();
        let request_body = serde_json::to_value(&request)?;

        let mut http_request = client
            .post(format!("{}/chat/completions", config.api_base()))
            .header("Content-Type", "application/json")
            .json(&request_body);

        for (key, value) in config.headers().iter() {
            http_request = http_request.header(key, value);
        }

        let response = http_request.send().await?;
        let response_text = response.text().await?;

        let response: ImageResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("Failed to parse image response: {}", e))?;

        // Process and upload images
        let choice = response.choices.first()
            .ok_or(anyhow!("No choices in image response"))?;

        let images = choice.message.images.as_ref()
            .ok_or(anyhow!("No images in response"))?;

        let mut uploaded_urls = Vec::new();
        for image in images {
            let base64_data = &image.image_url.url;
            let uploaded_url = self.r2_client().upload_base64(ImageFolder::Generated, base64_data).await?;
            uploaded_urls.push(uploaded_url);
        }

        let description = choice.message.content.clone().unwrap_or_default();

        // Create virtual tool call
        let result = GenerateImageResult {
            image_urls: uploaded_urls,
            description,
        };

        let virtual_tool_call = FunctionCall {
            name: "generate_image".to_string(),
            arguments: serde_json::to_string(&result)?,
        };

        // Build final message
        let message = Message {
            id: Uuid::new_v4(),
            owner: caller.clone(),
            system_config: self.system_config().id,
            session: None,

            user_message_content: user_message.content.clone(),
            user_message_content_type: user_message.content_type.clone(),
            input_toolcall: Json(None),

            assistant_message_content: choice.message.content.clone().unwrap_or_default(),
            assistant_message_content_type: MessageType::Image,
            assistant_message_tool_call: Json(Some(virtual_tool_call.clone())),

            model_name: Self::model().to_string(),
            usage: Json(response.usage.clone()),
            finish_reason: choice.finish_reason.clone(),
            refusal: None,

            summary: None,
            is_stale: false,
            is_memorizeable: false,
            is_in_memory: false,
            is_migrated: false,
            created_at: 0,
            updated_at: 0,
        };

        println!("message: {:?}", message);

        let tool = Self::Tool::try_from_tool_call(&virtual_tool_call)?;
        let (msg, misc_value) = self.handle_output(input, &message, &tool).await?;

        Ok((msg, tool, misc_value))
    }
}