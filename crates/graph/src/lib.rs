mod env;
mod relation;
mod add;
mod del;
mod search;
mod llm;

use std::sync::Arc;

use anyhow::Result;
use async_openai::{config::OpenAIConfig, Client};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolChoiceOption, CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs
};
use neo4rs::{ConfigBuilder, Graph};
use voda_common::EnvVars;
use voda_runtime::{define_function_types, ExecutableFunctionCall};

use crate::{env::GraphEnv, llm::LlmConfig};
pub type Embedding = Vec<f32>;

const EMBEDDING_DIMS: i32 = 1024;

define_function_types!(
    EntitiesToolcall(crate::llm::EntitiesToolcall, "extract_entities"),
    RelationshipsToolcall(crate::llm::RelationshipsToolcall, "establish_relations")
);

pub struct GraphDatabase {
    db: Arc<Graph>,
    embeder: Client<OpenAIConfig>,
    llm: Client<OpenAIConfig>,
}

impl GraphDatabase {
    pub async fn new() -> Self {
        let env = GraphEnv::load();
        let config = ConfigBuilder::default()
            .uri(env.get_env_var("GRAPH_URI"))
            .user(env.get_env_var("GRAPH_USER"))
            .password(env.get_env_var("GRAPH_PASSWORD"))
            .db("memgraph")
            .build()
            .unwrap();

        let db = Graph::connect(config).await.unwrap();

        let embeder_config = OpenAIConfig::new()
            .with_api_base(env.get_env_var("EMBEDDING_BASE_URL"))
            .with_api_key(env.get_env_var("EMBEDDING_API_KEY"));

        let llm_config = OpenAIConfig::new()
            .with_api_base(env.get_env_var("OPENAI_BASE_URL"))
            .with_api_key(env.get_env_var("OPENAI_API_KEY"));

        let embeder = Client::build(
            reqwest::Client::new(),
            embeder_config,
            Default::default()
        );
        let llm = Client::build(    
            reqwest::Client::new(),
            llm_config,
            Default::default()
        );

        Self { db: Arc::new(db), embeder, llm }
    }

    pub async fn init(&self) -> Result<()> {
        let ddl1 = format!(
            "CREATE VECTOR INDEX memzero ON :Entity(embedding) WITH CONFIG {{'dimension': {}, 'capacity': 1000, 'metric': 'cos'}};",
            EMBEDDING_DIMS
        );
        self.db.run(neo4rs::query(&ddl1)).await?;

        let ddl2 = "CREATE INDEX ON :Entity(user_id);";
        self.db.run(neo4rs::query(ddl2)).await?;

        let ddl3 = "CREATE INDEX ON :Entity;";
        self.db.run(neo4rs::query(ddl3)).await?;

        Ok(())
    }

    pub async fn embed(&self, text: Vec<String>) -> Result<Vec<Embedding>> {
        let env = GraphEnv::load();
        let response = self.embeder.embeddings().create(
            CreateEmbeddingRequestArgs::default()
                .model(&env.get_env_var("EMBEDDING_EMBEDDING_MODEL"))
                .input(text)
                .build()?
        ).await?;
        let embeddings = response.data
            .into_iter()
            .map(|item| item.embedding)
            .collect();

        Ok(embeddings)
    }

    pub async fn llm(&self, config: &LlmConfig, user_message: &str) -> Result<String> {
        let messages = [
            ChatCompletionRequestMessage::System(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(config.system_prompt.clone())
                    .build()?
            ),
            ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_message.to_string())
                    .build()?
            ),
        ];

        let tools = config.tools.iter()
            .map(|function| ChatCompletionToolArgs::default()
                .function(function.clone())
                .build()
                .expect("Message should build")
            )
            .collect::<Vec<_>>();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&config.model)
            .messages(messages)
            .tools(tools)
            .temperature(config.temperature)
            .max_tokens(config.max_tokens as u32)
            .tool_choice(ChatCompletionToolChoiceOption::Auto)
            .build()?;

        let response = self.llm.chat().create(request).await?;
        let content = response.choices.first().unwrap().message.content.clone().unwrap_or_default();

        response.choices.first().unwrap()
            .message.tool_calls.clone().unwrap_or_default()
            .iter()
            .for_each(|t| {
                let tc = RuntimeFunctionType::from_function_call(t.function.clone()).unwrap();
                println!("tc: {:#?}", tc);
            });

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::{EntityTag, Relationship};
    use uuid::Uuid;

    impl GraphDatabase {
        async fn cleanup(&self) -> Result<()> {
            self.db.run(neo4rs::query("MATCH (n) DETACH DELETE n")).await?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_graph_operations() {
        // 1. Setup
        let db = GraphDatabase::new().await;
        db.cleanup().await.unwrap(); // Clean before test
        db.init().await.unwrap();

        // 2. Data
        let user_id = Uuid::new_v4().to_string();
        let relationships = vec![
            Relationship {
                source: "Elon Musk".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "SpaceX".to_string(),
                user_id: user_id.clone(),
            },
            Relationship {
                source: "Elon Musk".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "Tesla".to_string(),
                user_id: user_id.clone(),
            },
        ];
        let entity_tags = vec![
            EntityTag {
                entity_name: "Elon Musk".to_string(),
                entity_tag: "Person".to_string(),
            },
            EntityTag {
                entity_name: "SpaceX".to_string(),
                entity_tag: "Company".to_string(),
            },
            EntityTag {
                entity_name: "Tesla".to_string(),
                entity_tag: "Company".to_string(),
            },
        ];

        // 3. Add
        db.add(&relationships, &entity_tags).await.unwrap();
        
        // Allow time for indexing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 4. Search
        let search_results = db.search(vec!["Elon Musk".to_string()], user_id.clone(), None).await.unwrap();
        assert_eq!(search_results.len(), 2);

        // 5. Delete
        let rel_to_delete = vec![
            Relationship {
                source: "Elon Musk".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "Tesla".to_string(),
                user_id: user_id.clone(),
            },
        ];
        db.delete(&rel_to_delete).await.unwrap();

        // 6. Search again
        let search_results_after_delete = db.search(vec!["Elon Musk".to_string()], user_id.clone(), None).await.unwrap();
        assert_eq!(search_results_after_delete.len(), 1);
        
        let relation_info: serde_json::Value = serde_json::from_str(&search_results_after_delete[0]).unwrap();
        assert_eq!(relation_info["destination"], "SpaceX");

        // 7. Cleanup
        db.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn test_fuzzy_and_comprehensive_search() {
        // 1. Setup
        let db = GraphDatabase::new().await;
        db.cleanup().await.unwrap(); // Clean before test
        db.init().await.unwrap();

        // 2. Data
        let user_id = Uuid::new_v4().to_string();
        let relationships = vec![
            Relationship {
                source: "Elon Musk".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "SpaceX".to_string(),
                user_id: user_id.clone(),
            },
            Relationship {
                source: "Elon Musk".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "Tesla".to_string(),
                user_id: user_id.clone(),
            },
            Relationship {
                source: "Jeff Bezos".to_string(),
                relationship: "FOUNDED".to_string(),
                destination: "Amazon".to_string(),
                user_id: user_id.clone(),
            },
        ];
        let entity_tags = vec![
            EntityTag {
                entity_name: "Elon Musk".to_string(),
                entity_tag: "Person".to_string(),
            },
            EntityTag {
                entity_name: "SpaceX".to_string(),
                entity_tag: "Company".to_string(),
            },
            EntityTag {
                entity_name: "Tesla".to_string(),
                entity_tag: "Company".to_string(),
            },
            EntityTag {
                entity_name: "Jeff Bezos".to_string(),
                entity_tag: "Person".to_string(),
            },
            EntityTag {
                entity_name: "Amazon".to_string(),
                entity_tag: "Company".to_string(),
            },
        ];

        // 3. Add data
        db.add(&relationships, &entity_tags).await.unwrap();
        println!("Successfully added initial relationships for fuzzy search test.");
        
        // Allow time for indexing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // 4. Fuzzy Search for a partial name
        let fuzzy_search_results_elon = db.search(vec!["Elon".to_string()], user_id.clone(), None).await.unwrap();
        println!("Fuzzy search results for 'Elon': {:#?}", fuzzy_search_results_elon);
        assert_eq!(fuzzy_search_results_elon.len(), 2);

        // 5. Fuzzy Search for a descriptive name
        let fuzzy_search_results_company = db.search(vec!["Space Exploration Technologies".to_string()], user_id.clone(), None).await.unwrap();
        println!("Fuzzy search results for 'Space Exploration Technologies': {:#?}", fuzzy_search_results_company);
        assert_eq!(fuzzy_search_results_company.len(), 1);
        let relation_info_company: serde_json::Value = serde_json::from_str(&fuzzy_search_results_company[0]).unwrap();
        assert_eq!(relation_info_company["source"], "Elon Musk");
        assert_eq!(relation_info_company["destination"], "SpaceX");

        // 6. Search for multiple entities, one fuzzy, one exact
        let multi_search_results = db.search(vec!["Bezos".to_string(), "Tesla".to_string()], user_id.clone(), None).await.unwrap();
        println!("Multi-entity search for 'Bezos' and 'Tesla': {:#?}", multi_search_results);
        assert_eq!(multi_search_results.len(), 2);

        // 7. Search for something that shouldn't match
        let no_match_results = db.search(vec!["nonexistent company".to_string()], user_id.clone(), None).await.unwrap();
        println!("Search results for 'nonexistent company': {:#?}", no_match_results);
        assert!(no_match_results.is_empty());

        // 8. Cleanup
        db.cleanup().await.unwrap();
    }
}