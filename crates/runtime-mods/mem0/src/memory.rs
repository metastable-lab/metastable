use anyhow::{anyhow, Result};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_runtime::{ExecutableFunctionCall, Memory, Message, MessageRole, MessageType, SystemConfig};

use crate::{EmbeddingMessage, Mem0Filter};
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
        let filter = Mem0Filter {
            user_id: messages[0].user_id.clone(),
            character_id: messages[0].character_id.clone(),
            session_id: messages[0].session_id.clone(),
        };
        let user_aka = messages[0].user_aka.clone();
        let flattened_message = Mem0Messages::pack_flat_messages(messages)?;
        
        // 1. TASK 1 - VectorDB Operations
        let self_clone_vector = self.clone();
        let flattened_message_clone = flattened_message.clone();
        let filter_clone = filter.clone();
        let vector_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting vector DB operations");
            let facts_tool_input = ExtractFactsToolInput {
                filter: filter_clone.clone(),
                new_message: flattened_message_clone.clone(),
            };
            let (facts_tool_call, facts_llm_response) = FactsToolcall::call(&self_clone_vector, facts_tool_input).await?;
            // 1.1 extract facts & build embeddings
            let embedding_messages = facts_tool_call.execute(&facts_llm_response, &self_clone_vector).await?;

            // 1.2 search db for existing memories
            let update_memory_input = MemoryUpdateToolInput::search_vector_db_and_prepare_input(&filter_clone, embedding_messages, &self_clone_vector).await?;

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
        let filter_clone = filter.clone();
        let user_aka_clone = user_aka.clone();
        let graph_db_operations: AsyncTask = tokio::spawn(async move {
            tracing::debug!("[Mem0Engine::add_messages] Starting graph DB operations");
            // 2.1 extract entities
            let entities_tool_input = ExtractEntityToolInput {
                filter: filter.clone(),
                new_message: flattened_message_clone.clone(),
                user_aka: user_aka_clone.clone(),
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
                    filter: filter_clone.clone(),
                    entities: type_mapping_clone.clone(),
                    new_information: flattened_message_clone.clone(),
                    user_aka: user_aka_clone.clone(),
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
            let filter_clone = filter.clone();
            let graph_db_delete_operations: AsyncTask = tokio::spawn(async move {
                tracing::debug!("[Mem0Engine::add_messages] Starting graph DB delete operations");
                // 2.3 search for exisiting nodes
                let type_mapping_keys = type_mapping_clone.clone().iter().map(|entity| entity.entity_name.clone()).collect::<Vec<_>>();
                tracing::debug!("[Mem0Engine::add_messages] Searching for existing nodes with keys: {:?}", type_mapping_keys);
                let nodes = self_clone_delete.graph_db_search(type_mapping_keys, &filter_clone).await?;
                let relationships = nodes.iter().map(|node| node.into()).collect::<Vec<_>>();
                tracing::debug!("[Mem0Engine::add_messages] Found {} existing nodes", nodes.len());

                // 2.4 Delete existing relationships if we have to
                let delete_relationship_tool_input = DeleteGraphMemoryToolInput {
                    filter: filter_clone.clone(),
                    type_mapping: type_mapping_clone.clone(),
                    existing_memories: relationships,
                    new_message: flattened_message_clone.clone(),
                    user_aka: user_aka.clone(),
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
            let insert_result = graph_db_insert_results?;
            let delete_result = graph_db_delete_results?;
            if insert_result.is_err() || delete_result.is_err() {
                tracing::warn!("[Mem0Engine::add_messages] Failed to insert or delete relationships. Insert result: {:?}, Delete result: {:?}", insert_result, delete_result);
            }
            Ok(())
        });

        let (vector_db_results, graph_db_results) = futures::future::join(
            vector_db_operations, 
            graph_db_operations
        ).await;
        let vector_db_results = vector_db_results?;
        let graph_db_results = graph_db_results?;
        if vector_db_results.is_err() || graph_db_results.is_err() {
            tracing::warn!("[Mem0Engine::add_messages] Failed to add messages. Vector DB result: {:?}, Graph DB result: {:?}", vector_db_results, graph_db_results);
        }
        Ok(())
    }

    async fn search(&self, message: &Mem0Messages, limit: u64) -> Result<
        (Vec<Mem0Messages>, SystemConfig)
    > {
        let filter = Mem0Filter {
            user_id: message.user_id.clone(),
            character_id: message.character_id.clone(),
            session_id: message.session_id.clone(),
        };
        let content = message.content.clone();
        let user_aka = message.user_aka.clone();
        tracing::debug!("[Mem0Engine::search] Searching for message content: {}", content);

        // NOTE: the vector_search always returns a corresponding embedding message for each fact
        // so we can safely get the first element
        let embedding_query = EmbeddingMessage::batch_create(self, &[content.clone()], &filter).await?;
        let the_embedding_query = embedding_query[0].clone();
        tracing::debug!("[Mem0Engine::search] Generated embedding for search");

        let vector_search_query = EmbeddingMessage::batch_search(self, &filter, &embedding_query, limit as i64);
        let graph_search_query = self.graph_db_search(vec![content], &filter);

        let (vector_search_results, graph_search_results) = futures::future::join(vector_search_query, graph_search_query).await;
        let vector_search_results = vector_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in vector_db_search_embeddings: {:?}", e))?;
        let graph_search_results = graph_search_results.map_err(|e| anyhow!("[Mem0Engine::search] Error in graph_db_search: {:?}", e))?;
        tracing::debug!("[Mem0Engine::search] Vector search found {} results", vector_search_results.len());
        tracing::debug!("[Mem0Engine::search] Graph search found {} results", graph_search_results.len());

        let mut memories = Vec::new();

        // NOTE: the vector_search always returns a corresponding embedding message for each fact
        // so we can safely get the first element
        let existing_memories = vector_search_results[0]
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>(); 

        tracing::debug!("[Mem0Engine::search] Vector search memories: {:?}", existing_memories);
        memories.push(Mem0Messages {
            id: the_embedding_query.id,
            user_id: the_embedding_query.user_id,
            character_id: the_embedding_query.character_id,
            session_id: the_embedding_query.session_id,
            user_aka: user_aka.clone(),
            content_type: MessageType::Text,
            role: MessageRole::User,
            content: existing_memories.join("\n"),
            created_at: the_embedding_query.created_at,
            updated_at: the_embedding_query.updated_at,
        });

        let relations = graph_search_results.iter()
            .map(|relation_info| format!("{} {} {}", relation_info.source, relation_info.relationship, relation_info.destination))
            .collect::<Vec<_>>();
        tracing::debug!("[Mem0Engine::search] Graph search relations: {:?}", relations);

        memories.push(Mem0Messages {
            id: Uuid::new_v4(),
            user_id: filter.user_id,
            character_id: filter.character_id,
            session_id: filter.session_id,
            user_aka: user_aka.clone(),
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