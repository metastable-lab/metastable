use anyhow::{anyhow, Result};
use sqlx::types::Uuid;
use voda_common::get_current_timestamp;
use voda_runtime::{Memory, Message, MessageRole, MessageType, SystemConfig};

use crate::pgvector::VectorQueryCriteria;
use crate::{EmbeddingMessage, GraphEntities};
use crate::{message::Mem0Messages, Mem0Engine};
use crate::llm::{
    get_delete_graph_memory_config, get_extract_entity_config, get_extract_facts_config, get_extract_relationship_config, get_update_memory_config, 
    DeleteGraphMemoryToolcall, EntitiesToolcall, FactsToolcall, InputMemory, MemoryUpdateToolcall, RelationshipsToolcall
};

type AsyncTask = tokio::task::JoinHandle<Result<()>>;

#[async_trait::async_trait]
impl Memory for Mem0Engine {
    type MessageType = Mem0Messages;

    async fn initialize(&mut self) -> Result<()> { 
        self.init().await
    }

    async fn add_messages(&self, messages: &[Mem0Messages]) -> Result<()> {
        let user_id = messages[0].user_id.clone();
        let agent_id = messages[0].agent_id.clone();

        let flattened_message = Mem0Messages::pack_flat_messages(messages)?;
        
        let self_clone_vector = self.clone();
        let flattened_message_vector = flattened_message.clone();
        let user_id_vector = user_id.clone();
        let agent_id_vector = agent_id.clone();
        // 1. TASK 1 - VectorDB Operations
        let vector_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting vector DB operations");
            let (extract_facts_config, user_prompt) = get_extract_facts_config(&user_id_vector, &flattened_message_vector);
            let facts = extract_facts_config.call::<FactsToolcall>(&self_clone_vector, user_prompt).await?.facts;
            tracing::info!("[Mem0Engine::add_messages] Extracted {} facts", facts.len());
            if facts.is_empty() {
                tracing::info!("[Mem0Engine::add_messages] No facts extracted, skipping");
                return Ok(());
            }

            let embeddings = self_clone_vector.embed(facts.clone()).await?;
            tracing::debug!("[Mem0Engine::add_messages] Generated {} embeddings", embeddings.len());
            let embedding_messages = embeddings.iter().zip(facts.clone())
                .map(|(embedding, fact)| EmbeddingMessage {
                    id: Uuid::new_v4(),
                    user_id: user_id_vector.clone(),
                    agent_id: agent_id_vector.clone(),
                    embedding: embedding.clone().into(),
                    content: fact,
                    created_at: get_current_timestamp(),
                    updated_at: get_current_timestamp(),
                }).collect::<Vec<_>>();

            // 1. search db for existing memories
            tracing::debug!("[Mem0Engine::add_messages] Searching vector DB for existing memories");
            let queries = embedding_messages.iter().map(|embedding_message| {
                let criteria = VectorQueryCriteria::new(&embedding_message.embedding, user_id_vector.clone())
                    .with_limit(5)
                    .with_agent_id(agent_id_vector);
                self_clone_vector.vector_db_search_embeddings(criteria)
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
            tracing::debug!("[Mem0Engine::add_messages] Found {} existing memories", existing_memories.len());
            let (config, user_prompt) = get_update_memory_config(facts, &existing_memories);
            let simplified_update_entries = config.call::<MemoryUpdateToolcall>(&self_clone_vector, user_prompt).await?;
            let update_entries = simplified_update_entries.into_memory_update_entry(user_id_vector.clone(), agent_id_vector.unwrap_or_default());
            tracing::debug!("[Mem0Engine::add_messages] Updating {} vector DB entries", update_entries.len());
            self_clone_vector.vector_db_batch_update(update_entries).await?;

            Ok(())
        });

        // 2. TASK 2 - GraphDB Operations
        let self_clone_graph = self.clone();
        let graph_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting graph DB operations");

            let (type_mapping_config, user_message) = get_extract_entity_config(user_id.clone().to_string(), flattened_message.clone());
            let type_mapping = type_mapping_config.call::<EntitiesToolcall>(&self_clone_graph, user_message).await?;

            // 2.1 Insert Operations
            let self_clone_insert = self_clone_graph.clone();
            let flattened_message_clone = flattened_message.clone();
            let type_mapping_entities_clone = type_mapping.entities.clone();
            let user_id_clone = user_id.clone();
            let agent_id_clone = agent_id.clone();
            let graph_db_insert_operations: AsyncTask = tokio::spawn(async move {
                tracing::debug!("[Mem0Engine::add_messages] Starting graph DB insert operations");
                 // 2.2 get relationships on the new information
                let (relationship_config, user_message) = get_extract_relationship_config(user_id_clone.to_string(), &type_mapping_entities_clone, flattened_message_clone);
                let relationship = relationship_config.call::<RelationshipsToolcall>(&self_clone_insert, user_message).await?;
                tracing::debug!("[Mem0Engine::add_messages] Extracted relationships: {:?}", relationship.relationships);

                // 2.6 add new relationships
                let add_relationship = relationship.relationships;
                let add_entities = GraphEntities::new(
                    add_relationship,
                    type_mapping_entities_clone,
                    user_id_clone,
                    agent_id_clone,
                );
                tracing::debug!("[Mem0Engine::add_messages] Adding relationships to graph DB");
                let add_size = self_clone_insert.graph_db_add(&add_entities).await?;
                tracing::info!("[Mem0Engine::add_messages] Added {} relationships", add_size);
                Ok(())
            });

            // Delete operations
            let self_clone_delete = self_clone_graph.clone();
            let graph_db_delete_operations: AsyncTask = tokio::spawn(async move {
                tracing::debug!("[Mem0Engine::add_messages] Starting graph DB delete operations");
                // 2.3 search for exisiting nodes
                let type_mapping_keys = type_mapping.entities.iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>();
                tracing::debug!("[Mem0Engine::add_messages] Searching for existing nodes with keys: {:?}", type_mapping_keys);
                let nodes = self_clone_delete.graph_db_search(type_mapping_keys, user_id.clone(), agent_id.clone()).await?;
                tracing::debug!("[Mem0Engine::add_messages] Found {} existing nodes", nodes.len());

                // 2.4 Delete existing relationships
                let (delete_relationship_config, user_message) = get_delete_graph_memory_config(user_id.clone().to_string(), nodes, flattened_message);
                let delete_relationship = delete_relationship_config.call::<DeleteGraphMemoryToolcall>(&self_clone_delete, user_message).await?;
                tracing::debug!("[Mem0Engine::add_messages] Relationships to delete: {:?}", delete_relationship.relationships);

                // 2.5 delete existing relationships
                let delete_relationship = delete_relationship.relationships;
                let delete_entities = GraphEntities::new(
                    delete_relationship,
                    type_mapping.entities,
                    user_id,
                    agent_id,
                );
                tracing::debug!("[Mem0Engine::add_messages] Deleting relationships from graph DB");
                let delete_size = self_clone_delete.graph_db_delete(&delete_entities).await?;
                tracing::info!("[Mem0Engine::add_messages] Deleted {} relationships", delete_size);
                Ok(())
            });

            let (graph_db_insert_results, graph_db_delete_results) = futures::future::join(graph_db_insert_operations, graph_db_delete_operations).await;
            graph_db_insert_results??;
            graph_db_delete_results??;
            Ok(())
        });

        let (vector_db_results, graph_db_results) = futures::future::join(vector_db_operations, graph_db_operations).await;
        vector_db_results??;
        graph_db_results??;
        Ok(())
    }

    async fn search(&self, message: &Mem0Messages, limit: u64) -> Result<
        (Vec<Mem0Messages>, SystemConfig)
    > {
        let user_id = message.user_id;
        let agent_id = message.agent_id;
        let content = message.content.clone();
        tracing::debug!("[Mem0Engine::search] Searching for message content: {}", content);

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
        tracing::debug!("[Mem0Engine::search] Generated embedding for search");

        let criteria = VectorQueryCriteria::new(&embedding_message.embedding, user_id.clone())
            .with_limit(limit as usize)
            .with_agent_id(agent_id);
        let vector_search_query = self.vector_db_search_embeddings(criteria);
        let graph_search_query = self.graph_db_search(vec![content], user_id.clone(), agent_id.clone());

        let (vector_search_results, graph_search_results) = futures::future::join(vector_search_query, graph_search_query).await;
        let vector_search_results = vector_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in vector_db_search_embeddings: {:?}", e))?;
        let graph_search_results = graph_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in graph_db_search: {:?}", e))?;
        tracing::debug!("[Mem0Engine::search] Vector search found {} results", vector_search_results.len());
        tracing::debug!("[Mem0Engine::search] Graph search found {} results", graph_search_results.len());

        let mut memories = Vec::new();

        let embedding_messages = vector_search_results.iter().map(|(embedding_message, _)| embedding_message.content.clone()).collect::<Vec<_>>(); 
        tracing::debug!("[Mem0Engine::search] Vector search memories: {:?}", embedding_messages);
        memories.push(Mem0Messages {
            id: embedding_message.id,
            user_id: embedding_message.user_id,
            agent_id: embedding_message.agent_id,
            content_type: MessageType::Text,
            role: MessageRole::User,
            content: embedding_messages.join("\n"),
            created_at: embedding_message.created_at,
            updated_at: embedding_message.updated_at,
        });

        let relations = graph_search_results.iter()
            .map(|relation_info| format!("{} {} {}", relation_info.source, relation_info.relationship, relation_info.destination))
            .collect::<Vec<_>>();
        tracing::debug!("[Mem0Engine::search] Graph search relations: {:?}", relations);

        memories.push(Mem0Messages {
            id: Uuid::new_v4(),
            user_id: user_id.clone(),
            agent_id: agent_id.clone(),
            content_type: MessageType::Text,
            role: MessageRole::User,
            content: relations.join("\n"),
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        });
        
        tracing::debug!("[Mem0Engine::search] Returning {} memories", memories.len());
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