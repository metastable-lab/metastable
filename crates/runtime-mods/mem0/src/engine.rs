use anyhow::Result;
use metastable_runtime::{Agent, MessageRole, MessageType, Prompt};

use crate::{EmbeddingMessage, Mem0Engine, Mem0Filter};
use crate::agents::{
    ExtractFactsAgent, ExtractFactsInput, 
    UpdateMemoryAgent, UpdateMemoryInput
};

#[cfg(feature = "graph")]
use crate::agents::{
    ExtractEntitiesAgent, ExtractEntitiesInput,
    ExtractRelationshipsAgent, ExtractRelationshipsInput,
    DeleteRelationshipsAgent, DeleteRelationshipsInput,
};
type AsyncTask = tokio::task::JoinHandle<Result<()>>;

impl Mem0Engine {
    async fn add_memory(&self, message: String, filter: &Mem0Filter) -> Result<()> {
        let fact_extract_agent = ExtractFactsAgent::new().await?;
        let memory_update_agent = UpdateMemoryAgent::new().await?;

        let facts_tool_input = ExtractFactsInput {
            filter: filter.clone(),
            new_message: message.clone(),
        };
        let (_, output, _) = fact_extract_agent.call(&filter.user_id, &facts_tool_input).await?;
        let embedding_messages = EmbeddingMessage::batch_create(
            &self, &output.facts, &filter
        ).await?;
        let update_memory_input = UpdateMemoryInput {
            filter: filter.clone(),
            existing_memories: embedding_messages.clone(),
        };
        let (_, _, summary) = memory_update_agent.call(&filter.user_id, &update_memory_input).await?;
        tracing::info!("Memory update summary: {:?}", summary);
        Ok(())
    }

    #[cfg(feature = "graph")]
    async fn add_graph(&self, message: String, filter: &Mem0Filter) -> Result<()> {
        let entity_extract_agent = ExtractEntitiesAgent::new().await?;
        let relationship_extract_agent = ExtractRelationshipsAgent::new().await?;
        let delete_relationship_agent = DeleteRelationshipsAgent::new().await?;

        let entity_tool_input = ExtractEntitiesInput {
            filter: filter.clone(),
            new_message: message.clone(),
            user_aka: filter.user_aka.clone(),
        };
        let (_, output, _) = entity_extract_agent.call(&filter.user_id, &entity_tool_input).await?;

        // insert operations
        let insert_cloned_filter = filter.clone();
        let insert_cloned_messages = message.clone();
        let insert_output = output.clone();
        let graph_insert_operations: AsyncTask = tokio::spawn(async move {
            let extract_relationship_tool_input = ExtractRelationshipsInput {
                filter: insert_cloned_filter.clone(),
                entities: insert_output.entities.clone(),
                new_message: insert_cloned_messages.clone(),
                user_aka: insert_cloned_filter.user_aka.clone(),
            };
            let (_, _, summary) = relationship_extract_agent.call(&insert_cloned_filter.user_id, &extract_relationship_tool_input).await?;
            tracing::info!("Relationship extraction summary: {:?}", summary);

            Ok(())
        });

        // delete operations
        let delete_cloned_filter = filter.clone();
        let delete_cloned_messages = message.clone();
        let delete_output = output.clone();
        let graph_delete_operations: AsyncTask = tokio::spawn(async move {
            let delete_relationship_tool_input = DeleteRelationshipsInput {
                filter: delete_cloned_filter.clone(),
                user_aka: delete_cloned_filter.user_aka.clone(),
                type_mapping: delete_output.entities.clone(),
                new_message: delete_cloned_messages.clone(),
            };
            let (_, _, summary) = delete_relationship_agent.call(&delete_cloned_filter.user_id, &delete_relationship_tool_input).await?;
            tracing::info!("Relationship deletion summary: {:?}", summary);

            Ok(())
        });

        let (graph_insert_results, graph_delete_results) = futures::future::join(
            graph_insert_operations, 
            graph_delete_operations
        ).await;
        let insert_result = graph_insert_results?;
        let delete_result = graph_delete_results?;
        if insert_result.is_err() || delete_result.is_err() {
            tracing::warn!("[Mem0Engine::add] Failed to insert or delete relationships. Insert result: {:?}, Delete result: {:?}", insert_result, delete_result);
        }
        Ok(())
    }

    pub async fn add(&self, messages: Vec<Prompt>, filter: &Mem0Filter) -> Result<()> {
        let messages = Prompt::pack_flat_messages(messages)?;

        let cloned_filter = filter.clone();
        let cloned_messages = messages.clone();
        let cloned_self = self.clone();
        let memories_operations: AsyncTask = tokio::spawn(async move {
            cloned_self.add_memory(cloned_messages, &cloned_filter).await
        });

        #[cfg(feature = "graph")]
        {
            let cloned_filter = filter.clone();
            let cloned_messages = messages.clone();
            let cloned_self = self.clone();
            let graph_operations: AsyncTask = tokio::spawn(async move {
                cloned_self.add_graph(cloned_messages, &cloned_filter).await
            });

            let (memories_result, graph_result) = futures::future::join(
                memories_operations, 
                graph_operations
            ).await;

            let memories_result = memories_result?;
            let graph_result = graph_result?;

            if memories_result.is_err() || graph_result.is_err() {
                tracing::warn!("[Mem0Engine::add] Failed to add memories or graph. Memories result: {:?}, Graph result: {:?}", memories_result, graph_result);
            }
        }

        #[cfg(not(feature = "graph"))]
        {
            let memories_result = memories_operations.await?;
            if memories_result.is_err() {
                tracing::warn!("[Mem0Engine::add] Failed to add memories. Memories result: {:?}", memories_result);
            }
        }

        Ok(())
    }

    pub async fn search(&self, message: Prompt, filter: &Mem0Filter) -> Result<Vec<Prompt>> {
        // Create embedding for the query message
        let query = EmbeddingMessage::batch_create(self, &[message.content], filter).await?;

        // Vector DB search (returns a future)
        let vector_db_search_fut = EmbeddingMessage::batch_search(self, filter, &query, 10);

        #[cfg(feature = "graph")]
        let graph_db_search_fut = self.graph_db.search(
            query.iter().map(|q| q.embedding.to_vec()).collect::<Vec<_>>(),
            filter
        );

        #[cfg(feature = "graph")]
        {
            // Run both searches concurrently
            let (vector_db_search_res, graph_db_search_res) = futures::future::join(
                vector_db_search_fut,
                graph_db_search_fut
            ).await;

            let vector_db_search = vector_db_search_res?
                .into_iter()
                .flatten()
                .map(|r| r.content.clone())
                .collect::<Vec<_>>();

            let graph_db_search = graph_db_search_res?
                .into_iter()
                .map(|r| format!("{} - {} - {}", r.source, r.relationship, r.destination))
                .collect::<Vec<_>>();

            // Combine results into Prompts
            let mut results = Vec::new();

            // If both have results, pair them; otherwise, show all from both
            let max_len = std::cmp::max(vector_db_search.len(), graph_db_search.len());
            for i in 0..max_len {
                let vector_db_result = vector_db_search.get(i).cloned().unwrap_or_default();
                let graph_db_result = graph_db_search.get(i).cloned().unwrap_or_default();
                results.push(Prompt {
                    role: MessageRole::User,
                    content_type: MessageType::Text,
                    content: format!("Vector DB Search: {}\nGraph DB Search: {}", vector_db_result, graph_db_result),
                    toolcall: None,
                    created_at: 1,
                });
            }
            Ok(results)
        }

        #[cfg(not(feature = "graph"))]
        {
            let vector_db_search = vector_db_search_fut.await?
                .into_iter()
                .flatten()
                .map(|r| r.content.clone())
                .collect::<Vec<_>>();

            let mut results = Vec::new();
            for vector_db_result in vector_db_search {
                results.push(Prompt {
                    role: MessageRole::User,
                    content_type: MessageType::Text,
                    content: format!("Vector DB Search: {}", vector_db_result),
                    toolcall: None,
                    created_at: 1,
                });
            }
            Ok(results)
        }
    }
}