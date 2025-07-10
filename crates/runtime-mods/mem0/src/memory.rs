use anyhow::{anyhow, Result};
use sqlx::types::Uuid;
use voda_common::get_current_timestamp;
use voda_runtime::{ExecutableFunctionCall, Memory, Message, MessageRole, MessageType, SystemConfig};

use crate::pgvector::VectorQueryCriteria;
use crate::EmbeddingMessage;
use crate::{message::Mem0Messages, Mem0Engine};
use crate::llm::{
    LlmTool,
    DeleteGraphMemoryToolInput, DeleteGraphMemoryToolcall, 
    EntitiesToolcall, ExtractEntityToolInput, 
    ExtractFactsToolInput, FactsToolcall,
    MemoryUpdateToolInput, MemoryUpdateToolcall, 
    ExtractRelationshipToolInput,RelationshipsToolcall
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
        
        // 1. TASK 1 - VectorDB Operations
        let self_clone_vector = self.clone();
        let flattened_message_clone = flattened_message.clone();
        let vector_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting vector DB operations");
            let facts_tool_input = ExtractFactsToolInput {
                user_id: user_id.clone(),
                agent_id: agent_id.clone(),
                new_message: flattened_message_clone.clone(),
            };
            let (facts_tool_call, facts_llm_response) = FactsToolcall::call(&self_clone_vector, facts_tool_input).await?;
            // 1.1 extract facts & build embeddings
            let embedding_messages = facts_tool_call.execute(&facts_llm_response, &self_clone_vector).await?;

            // 1.2 search db for existing memories
            let update_memory_input = MemoryUpdateToolInput::search_vector_db_and_prepare_input(user_id.clone(), agent_id.clone(), embedding_messages, &self_clone_vector).await?;
            
            // 1.3 get memories to update and push to db
            let (update_memory_tool_call, update_memory_llm_response) = MemoryUpdateToolcall::call(&self_clone_vector, update_memory_input).await?;
            let update_entries = update_memory_tool_call.execute(&update_memory_llm_response, &self_clone_vector).await?;
            tracing::debug!(
                "[Mem0Engine::add_messages] Added {} new memories and updated {} existing memories, deleted {} memories", 
                update_entries.added, update_entries.updated, update_entries.deleted
            );
            Ok(())
        });

        // 2. TASK 2 - GraphDB Operations
        let self_clone_graph = self.clone();
        let flattened_message_clone = flattened_message.clone();
        let graph_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting graph DB operations");
            // 2.1 extract entities
            let entities_tool_input = ExtractEntityToolInput {
                user_id: user_id.clone(),
                agent_id: agent_id.clone(),
                new_message: flattened_message_clone.clone(),
            };
            let (entities_tool_call, entities_llm_response) = EntitiesToolcall::call(&self_clone_graph, entities_tool_input).await?;
            let type_mapping = entities_tool_call.execute(&entities_llm_response, &self_clone_graph).await?;

            // 2.1 Insert Operations
            let self_clone_insert = self_clone_graph.clone();
            let type_mapping_clone = type_mapping.clone();
            let flattened_message_clone = flattened_message.clone();
            let graph_db_insert_operations: AsyncTask = tokio::spawn(async move {
                tracing::debug!("[Mem0Engine::add_messages] Starting graph DB insert operations");
                // 2.2 get relationships on the new information
                let relationship_tool_input = ExtractRelationshipToolInput {
                    user_id: user_id.clone(), agent_id: agent_id.clone(),
                    entities: type_mapping_clone.clone(),
                    new_information: flattened_message_clone.clone(),
                };
                let (relationship_tool_call, relationship_llm_response) = RelationshipsToolcall::call(&self_clone_insert, relationship_tool_input).await?;
                // insert new relationships into GraphDB
                let add_size = relationship_tool_call.execute(&relationship_llm_response, &self_clone_insert).await?;
                tracing::info!("[Mem0Engine::add_messages] Added {} relationships", add_size);
                Ok(())
            });

            // 2.2 Delete Operations
            let self_clone_delete = self_clone_graph.clone();
            let type_mapping_clone = type_mapping.clone();
            let flattened_message_clone = flattened_message.clone();
            let graph_db_delete_operations: AsyncTask = tokio::spawn(async move {
                tracing::debug!("[Mem0Engine::add_messages] Starting graph DB delete operations");
                // 2.3 search for exisiting nodes
                let type_mapping_keys = type_mapping_clone.clone().iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>();
                tracing::debug!("[Mem0Engine::add_messages] Searching for existing nodes with keys: {:?}", type_mapping_keys);
                let nodes = self_clone_delete.graph_db_search(type_mapping_keys, user_id.clone(), agent_id.clone()).await?;
                let relationships = nodes.iter().map(|node| node.into()).collect::<Vec<_>>();
                tracing::debug!("[Mem0Engine::add_messages] Found {} existing nodes", nodes.len());

                // 2.4 Delete existing relationships if we have to
                let delete_relationship_tool_input = DeleteGraphMemoryToolInput {
                    user_id: user_id.clone(), agent_id: agent_id.clone(),
                    type_mapping: type_mapping_clone.clone(),
                    existing_memories: relationships,
                    new_message: flattened_message_clone.clone(),
                };
                let (delete_relationship_tool_call, delete_relationship_llm_response) = DeleteGraphMemoryToolcall::call(&self_clone_delete, delete_relationship_tool_input).await?;
                let delete_size = delete_relationship_tool_call.execute(&delete_relationship_llm_response, &self_clone_delete).await?;
                tracing::info!("[Mem0Engine::add_messages] Deleted {} relationships", delete_size);

                Ok(())
            });

            let (graph_db_insert_results, graph_db_delete_results) = futures::future::join(
                graph_db_insert_operations, 
                graph_db_delete_operations
            ).await;
            graph_db_insert_results??;
            graph_db_delete_results??;
            Ok(())
        });

        let (vector_db_results, graph_db_results) = futures::future::join(
            vector_db_operations, 
            graph_db_operations
        ).await;
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