use anyhow::{anyhow, Result};
use sqlx::types::Uuid;
use voda_common::get_current_timestamp;
use voda_runtime::{Memory, Message, MessageType, SystemConfig};

use crate::pgvector::VectorQueryCriteria;
use crate::{EmbeddingMessage, GraphEntities};
use crate::{message::Mem0Messages, Mem0Engine};
use crate::llm::{
    get_delete_graph_memory_config, get_extract_entity_config, get_extract_facts_config, get_extract_relationship_config, get_update_memory_config, 
    DeleteGraphMemoryToolcall, EntitiesToolcall, FactsToolcall, InputMemory, MemoryUpdateToolcall, RelationshipsToolcall
};

#[async_trait::async_trait]
impl Memory for Mem0Engine {
    type MessageType = Mem0Messages;

    async fn initialize(&self) -> Result<()> { 
        self.init().await
    }

    async fn add_messages(&self, messages: &[Mem0Messages]) -> Result<()> {
        let user_id = messages[0].user_id.clone();
        let agent_id = messages[0].agent_id.clone();

        let flattened_message = Mem0Messages::pack_flat_messages(messages)?;
        let (extract_facts_config, user_prompt) = get_extract_facts_config(&user_id, &flattened_message);

        let response = self.llm(&extract_facts_config, user_prompt).await?;
        let facts = response.maybe_results.first().ok_or(anyhow!("[Mem0Engine::add_messages] No result from LLM"))?;
        let facts = serde_json::from_str::<FactsToolcall>(&facts)
            .map_err(|e| anyhow!("[Mem0Engine::add_messages] Error in parsing facts: {:?}", e))?
            .facts;
        tracing::info!("[Mem0Engine::add_messages] Extracted facts: {:?}", facts.len());
        if facts.is_empty() {
            tracing::info!("[Mem0Engine::add_messages] No facts extracted, skipping");
            return Ok(());
        }

        let embeddings = self.embed(facts.clone()).await?;
        let embedding_messages = embeddings.iter().zip(facts.clone())
            .map(|(embedding, fact)| EmbeddingMessage {
                id: Uuid::new_v4(),
                user_id: user_id.clone(),
                agent_id: agent_id.clone(),
                embedding: embedding.clone().into(),
                content: fact,
                created_at: get_current_timestamp(),
                updated_at: get_current_timestamp(),
            }).collect::<Vec<_>>();

        // 1. search db for existing memories
        let queries = embedding_messages.iter().map(|embedding_message| {
            let criteria = VectorQueryCriteria::new(&embedding_message.embedding, user_id.clone())
                .with_limit(5)
                .with_agent_id(agent_id);
            self.vector_db_search_embeddings(criteria)
        }).collect::<Vec<_>>();

        let results = futures::future::join_all(queries).await;
        let mut existing_memories = Vec::new();
        for result in results {
            match result {
                Ok(res) => {
                    existing_memories.extend(res.into_iter().map(|(embedding_message, _)| {
                        InputMemory {
                            id: embedding_message.id,
                            content: embedding_message.content,
                        }
                    }));
                }
                Err(e) => {
                    tracing::warn!("[Mem0Engine::add_messages] Error in vector_db_search_embeddings: {:?}", e);
                }
            }
        }
        let (config, user_prompt) = get_update_memory_config(facts, &existing_memories);
        let response = self.llm(&config, user_prompt).await?;
        let maybe_result = response.maybe_results.first().ok_or(anyhow!("[Mem0Engine::add_messages] No result from LLM"))?;
        let simplified_update_entries = serde_json::from_str::<MemoryUpdateToolcall>(&maybe_result)
            .map_err(|e| anyhow!("[Mem0Engine::add_messages] Error in parsing update memory toolcall: {:?}", e))?;
        let update_entries = simplified_update_entries.into_memory_update_entry(user_id.clone(), agent_id.unwrap_or_default());
        self.vector_db_batch_update(update_entries).await?;

        // 2. add to graph
        // 2.1 get the type mapping
        let (type_mapping_config, user_message) = get_extract_entity_config(user_id.clone().to_string(), flattened_message.clone());
        let type_mapping = self.llm(&type_mapping_config, user_message).await?;
        let type_mapping = type_mapping.maybe_results.first().ok_or(anyhow!("[Mem0Engine::add_messages] No result from LLM"))?;
        let type_mapping = serde_json::from_str::<EntitiesToolcall>(&type_mapping)
            .map_err(|e| anyhow!("[Mem0Engine::add_messages] Error in parsing type mapping: {:?}", e))?;

        // 2.2 get relationships on the new information
        let (relationship_config, user_message) = get_extract_relationship_config(user_id.clone().to_string(), &type_mapping.entities, flattened_message.clone());
        let relationship = self.llm(&relationship_config, user_message).await?;
        let relationship = relationship.maybe_results.first().ok_or(anyhow!("[Mem0Engine::add_messages] No result from LLM"))?;
        let relationship = serde_json::from_str::<RelationshipsToolcall>(&relationship)
            .map_err(|e| anyhow!("[Mem0Engine::add_messages] Error in parsing relationship: {:?}", e))?;
        
        // 2.3 search for exisiting nodes
        let type_mapping_keys = type_mapping.entities.iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>();
        let nodes = self.graph_db_search(type_mapping_keys, user_id.clone(), agent_id.clone()).await?;

        // 2.4 Delete existing relationships
        let (delete_relationship_config, user_message) = get_delete_graph_memory_config(user_id.clone().to_string(), nodes, flattened_message.clone());
        let delete_relationship = self.llm(&delete_relationship_config, user_message).await?;
        let delete_relationship = delete_relationship.maybe_results.first().ok_or(anyhow!("[Mem0Engine::add_messages] No result from LLM"))?;
        let delete_relationship = serde_json::from_str::<DeleteGraphMemoryToolcall>(&delete_relationship)
            .map_err(|e| anyhow!("[Mem0Engine::add_messages] Error in parsing delete relationship: {:?}", e))?;

        // 2.5 delete existing relationships
        let delete_relationship = delete_relationship.relationships;
        let delete_entities = GraphEntities::new(
            delete_relationship,
            type_mapping.entities.clone(),
            user_id.clone(),
            agent_id.clone(),
        );
        let delete_relationship = self.graph_db_delete(&delete_entities).await?;

        // 2.6 add new relationships
        let add_relationship = relationship.relationships;
        let add_entities = GraphEntities::new(
            add_relationship,
            type_mapping.entities,
            user_id.clone(),
            agent_id.clone(),
        );
        let add_relationship = self.graph_db_add(&add_entities).await?;

        tracing::info!("[Mem0Engine::add_messages] Added {} relationships and deleted {} relationships", add_relationship, delete_relationship);
        Ok(())
    }

    async fn search(&self, message: &Mem0Messages, limit: u64) -> Result<
        (Vec<Mem0Messages>, SystemConfig)
    > {
        let user_id = message.user_id;
        let agent_id = message.agent_id;
        let content = message.content.clone();

        let embeddings = self.embed(vec![content.clone()]).await?;
        let embedding_message = EmbeddingMessage {
            id: Uuid::new_v4(),
            user_id: user_id.clone(),
            agent_id: agent_id.clone(),
            embedding: embeddings[0].clone().into(),
            content: content.clone(),
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };

        let criteria = VectorQueryCriteria::new(&embedding_message.embedding, user_id.clone())
            .with_limit(limit as usize)
            .with_agent_id(agent_id);
        let vector_search_query = self.vector_db_search_embeddings(criteria);
        let graph_search_query = self.graph_db_search(vec![content], user_id.clone(), agent_id.clone());

        let (vector_search_results, graph_search_results) = futures::future::join(vector_search_query, graph_search_query).await;
        let vector_search_results = vector_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in vector_db_search_embeddings: {:?}", e))?;
        let graph_search_results = graph_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in graph_db_search: {:?}", e))?;

        let mut memories = Vec::new();

        let embedding_messages = vector_search_results.iter().map(|(embedding_message, _)| embedding_message.content.clone()).collect::<Vec<_>>(); 
        memories.push(Mem0Messages {
            id: embedding_message.id,
            user_id: embedding_message.user_id,
            agent_id: embedding_message.agent_id,
            content_type: MessageType::Text,
            content: embedding_messages.join("\n"),
            created_at: embedding_message.created_at,
            updated_at: embedding_message.updated_at,
        });

        let relations = graph_search_results.iter()
            .map(|relation_info| format!("{} {} {}", relation_info.source, relation_info.relationship, relation_info.destination))
            .collect::<Vec<_>>();

        memories.push(Mem0Messages {
            id: Uuid::new_v4(),
            user_id: user_id.clone(),
            agent_id: agent_id.clone(),
            content_type: MessageType::Text,
            content: relations.join("\n"),
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        });

        Ok((memories, SystemConfig::default()))
    }

    async fn update(&self, messages: &[Mem0Messages]) -> Result<()> {
        self.add_messages(messages).await
    }

    async fn delete(&self, _message_ids: &[Uuid]) -> Result<()> {
        unimplemented!()
    }

    async fn reset(&self, _user_id: &Uuid) -> Result<()> {
        unimplemented!()
    }
}