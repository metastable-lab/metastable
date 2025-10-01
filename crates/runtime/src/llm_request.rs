use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReasoningConfig {
    pub effort: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtendedChatCompletionRequest {
    #[serde(flatten)]
    pub base: async_openai::types::CreateChatCompletionRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<String>>,
}

pub async fn make_extended_request(
    extended_request: &ExtendedChatCompletionRequest,
    client_config: &async_openai::config::OpenAIConfig,
) -> Result<async_openai::types::CreateChatCompletionResponse> {
    use async_openai::config::Config;

    let client = reqwest::Client::new();
    let request_body = serde_json::to_value(extended_request)?;

    let mut request = client
        .post(format!("{}/chat/completions", client_config.api_base()))
        .header("Content-Type", "application/json")
        .json(&request_body);

    // Add headers from config (includes authorization)
    for (key, value) in client_config.headers().iter() {
        request = request.header(key, value);
    }

    let response = request.send().await?;

    let response_text = response.text().await?;
    let response: async_openai::types::CreateChatCompletionResponse =
        serde_json::from_str(&response_text)?;

    Ok(response)
}