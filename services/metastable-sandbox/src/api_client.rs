use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use metastable_runtime::User;

pub struct ApiClient {
    client: Client,
    base_url: String,
    user: User,
    secret_key: String,
}

impl ApiClient {
    pub fn new(base_url: String, user: User, secret_key: String, client: Client) -> Self {
        Self {
            client,
            base_url,
            user,
            secret_key,
        }
    }

    pub async fn create_session(
        &self,
        character_id: String,
        system_config_id: String,
    ) -> Result<()> {
        let token = self.user.generate_auth_token(&self.secret_key);
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
            .header("Authorization", format!("Bearer {}", token))
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
        let token = self.user.generate_auth_token(&self.secret_key);
        let response = self
            .client
            .post(format!(
                "{}/runtime/roleplay/chat/{}",
                self.base_url, session_id
            ))
            .json(&json!({ "message": message }))
            .header("Authorization", format!("Bearer {}", token))
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
        let token = self.user.generate_auth_token(&self.secret_key);
        let response = self
            .client
            .post(format!(
                "{}/runtime/roleplay/rollback/{}",
                self.base_url, session_id
            ))
            .json(&json!({ "message": message }))
            .header("Authorization", format!("Bearer {}", token))
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

    pub async fn buy_referral(&self, count: u32) -> Result<()> {
        let token = self.user.generate_auth_token(&self.secret_key);
        let response = self
            .client
            .post(format!("{}/user/referral/buy", self.base_url))
            .json(&json!({ "count": count }))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error body".to_string());
            return Err(anyhow::anyhow!(
                "Failed to buy referral. Status: {}. Body: {}",
                status,
                text
            ));
        }
        Ok(())
    }
}
