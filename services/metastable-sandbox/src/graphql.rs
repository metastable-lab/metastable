use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Uuid;
use tracing::{debug, error};
use metastable_runtime::User;
pub use metastable_runtime::SystemConfig;
pub use metastable_runtime_roleplay::Character;

pub mod flexible_date_deserializer {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum IntOrString {
            Int(i64),
            String(String),
        }

        match IntOrString::deserialize(deserializer)? {
            IntOrString::Int(i) => Ok(Utc.timestamp_opt(i, 0).single().unwrap()), // Safe to unwrap as timestamp is valid
            IntOrString::String(s) => DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(serde::de::Error::custom),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Message {
    pub id: Uuid,
    pub content: String,
    pub role: String,
    #[serde(deserialize_with = "flexible_date_deserializer::deserialize")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Session {
    pub id: Uuid,
    pub character: Uuid,
    #[serde(deserialize_with = "flexible_date_deserializer::deserialize")]
    pub created_at: DateTime<Utc>,
    pub roleplay_messages: Vec<Message>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CharacterSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub prompts_first_message: String,
    pub prompts_personality: String,
    pub prompts_scenario: String,
    pub prompts_example_dialogue: String,
    pub creator: Option<Uuid>,
    #[serde(deserialize_with = "flexible_date_deserializer::deserialize")]
    pub created_at: DateTime<Utc>,
    #[serde(deserialize_with = "flexible_date_deserializer::deserialize")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GetMySessionsAndMessagesData {
    pub roleplay_sessions: Vec<Session>,
}

#[derive(Debug, Deserialize)]
pub struct GetAllCharactersData {
    pub roleplay_characters: Vec<CharacterSummary>,
}

#[derive(Debug, Deserialize)]
pub struct GetAllSystemConfigsData {
    #[serde(rename = "system_configs")]
    pub system_configs: Vec<SystemConfig>,
}

#[derive(Serialize)]
struct GetSessionByCharacterVars {
    character_id: Uuid,
}

#[derive(Deserialize, Debug)]
struct GraphQLResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQLErrorDetail>>,
}

#[derive(Deserialize, Debug)]
struct GraphQLErrorDetail {
    message: String,
}

pub struct GraphQlClient {
    http_client: reqwest::Client,
    base_url: String,
    user: User,
    secret_key: String,
}

impl GraphQlClient {
    pub fn new(
        base_url: String,
        user: User,
        secret_key: String,
        http_client: reqwest::Client,
    ) -> Self {
        Self {
            base_url,
            http_client,
            user,
            secret_key,
        }
    }

    async fn post_graphql<V, T>(&self, query: &str, variables: V) -> Result<T>
    where
        V: Serialize,
        T: for<'de> Deserialize<'de>,
    {
        let graphql_url = format!("{}/graphql", self.base_url);

        let body = json!({
            "query": query,
            "variables": variables,
        });

        debug!(
            "Sending GraphQL request to {}: body: {}",
            graphql_url,
            serde_json::to_string_pretty(&body).unwrap_or_default()
        );

        let token = self.user.generate_auth_token(&self.secret_key);

        let res = self
            .http_client
            .post(&graphql_url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;
        let status = res.status();
        let response_text = res.text().await?;

        debug!("GraphQL response status: {}", status);
        debug!("GraphQL response text: {}", response_text);

        if !status.is_success() {
            error!("GraphQL request failed with status: {}", status);
            return Err(anyhow!(
                "GraphQL request failed with status: {}. Body: {}",
                status,
                response_text
            ));
        }

        match serde_json::from_str::<GraphQLResponse<T>>(&response_text) {
            Ok(gql_response) => {
                if let Some(errors) = gql_response.errors {
                    let error_messages: Vec<String> =
                        errors.into_iter().map(|e| e.message).collect();
                    error!("GraphQL query returned errors: {:?}", error_messages);
                    return Err(anyhow!(
                        "GraphQL query failed: {}",
                        error_messages.join(", ")
                    ));
                }

                if let Some(data) = gql_response.data {
                    Ok(data)
                } else {
                    Err(anyhow!(
                        "GraphQL response did not contain data or errors"
                    ))
                }
            }
            Err(e) => {
                error!("Failed to parse GraphQL response: {}", e);
                Err(anyhow!(e).context("Failed to parse GraphQL response"))
            }
        }
    }

    pub async fn get_my_sessions_and_messages(&self) -> Result<GetMySessionsAndMessagesData> {
        let query = r#"
            query GetMySessionsAndMessages {
              roleplay_sessions(order_by: {created_at: desc}) {
                id
                character
                created_at
                roleplay_messages(order_by: {created_at: asc}) {
                  id
                  content
                  role
                  created_at
                }
              }
            }
        "#;

        self.post_graphql(query, serde_json::Value::Null).await
    }

    pub async fn get_session_by_character(
        &self,
        character_id: Uuid,
    ) -> Result<GetMySessionsAndMessagesData> {
        let query = r#"
            query GetSessionByCharacter($character_id: uuid!) {
              roleplay_sessions(where: {character: {_eq: $character_id}}, order_by: {created_at: desc}, limit: 1) {
                id
                character
                created_at
                roleplay_messages(order_by: {created_at: asc}) {
                  id
                  content
                  role
                  created_at
                }
              }
            }
        "#;

        let vars = GetSessionByCharacterVars { character_id };

        self.post_graphql(query, vars).await
    }

    pub async fn get_all_characters(&self) -> Result<GetAllCharactersData> {
        let query = r#"
            query GetAllCharacters {
              roleplay_characters {
                id
                name
                description
                prompts_first_message
                prompts_personality
                prompts_scenario
                prompts_example_dialogue
                creator
                created_at
                updated_at
              }
            }
        "#;

        self.post_graphql(query, serde_json::Value::Null).await
    }

    pub async fn get_all_system_configs(&self) -> Result<GetAllSystemConfigsData> {
        let query = r#"
            query GetAllSystemConfigs {
              system_configs {
                id
                name
                system_prompt
                system_prompt_version
                openai_base_url
                openai_model
                openai_temperature
                openai_max_tokens
                functions
                updated_at
                created_at
              }
            }
        "#;

        self.post_graphql(query, serde_json::Value::Null).await
    }
}
