use anyhow::Result;
use reqwest::Client;
use serde_json::json;

pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String, client: Client) -> Self {
        Self { client, base_url }
    }

    pub async fn create_session(
        &self,
        character_id: String,
        system_config_id: String,
    ) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/runtime/roleplay/create_session",
                self.base_url
            ))
            .json(&json!({
                "character_id": character_id,
                "system_config_id": system_config_id,
            }))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error body".to_string());
            return Err(anyhow::anyhow!(
                "Failed to create session. Status: {}. Body: {}",
                status,
                text
            ));
        }
        Ok(())
    }

    pub async fn chat(&self, session_id: String, message: String) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/runtime/roleplay/chat/{}",
                self.base_url, session_id
            ))
            .json(&json!({ "message": message }))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error body".to_string());
            return Err(anyhow::anyhow!(
                "Failed to chat. Status: {}. Body: {}",
                status,
                text
            ));
        }
        Ok(())
    }

    pub async fn rollback(&self, session_id: String, message: String) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/runtime/roleplay/rollback/{}",
                self.base_url, session_id
            ))
            .json(&json!({ "message": message }))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error body".to_string());
            return Err(anyhow::anyhow!(
                "Failed to rollback. Status: {}. Body: {}",
                status,
                text
            ));
        }
        Ok(())
    }
}