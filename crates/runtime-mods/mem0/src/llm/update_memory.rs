use anyhow::Result;
use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::types::Uuid;
use voda_runtime::{ExecutableFunctionCall, LLMRunResponse};
use crate::{llm::{LlmTool, ToolInput}, pgvector::{BatchUpdateSummary, MemoryEvent, MemoryUpdateEntry, VectorQueryCriteria}, EmbeddingMessage, Mem0Engine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMemory {
    pub id: Uuid,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdateToolInput {
    pub user_id: Uuid, pub agent_id: Option<Uuid>,
    pub retrieved_facts: Vec<String>,
    pub old_memories: Vec<InputMemory>,
}

impl ToolInput for MemoryUpdateToolInput {
    fn user_id(&self) -> Uuid { self.user_id.clone() }
    fn agent_id(&self) -> Option<Uuid> { self.agent_id.clone() }

    fn build(&self) -> String {
        "Please update the memory based on the new facts.".to_string()
    }
}

impl MemoryUpdateToolInput {
    pub async fn search_vector_db_and_prepare_input(
        user_id: Uuid, agent_id: Option<Uuid>, 
        embedding_messages: Vec<EmbeddingMessage>,
        engine: &Mem0Engine,
    ) -> Result<Self> {
        tracing::debug!("[MemoryUpdateToolInput::prepare_input] Searching vector DB for existing memories");
        let queries = embedding_messages.iter().map(|embedding_message| {
            let criteria = VectorQueryCriteria::new(&embedding_message.embedding, user_id)
                .with_limit(5)
                .with_agent_id(agent_id);
            engine.vector_db_search_embeddings(criteria)
        }).collect::<Vec<_>>();
        let results = futures::future::join_all(queries).await;
        let mut existing_memories = Vec::new();
        for result in results {
            match result {
                Ok(res) => {
                    existing_memories.extend(res.into_iter().map(|(embedding_message, _)| {
                        InputMemory { id: embedding_message.id, content: embedding_message.content }
                    }));
                }
                Err(e) => {
                    tracing::warn!("[MemoryUpdateToolInput::prepare_input] Error in vector_db_search_embeddings: {:?}", e);
                }
            }
        }
        tracing::debug!("[MemoryUpdateToolInput::prepare_input] Found {} existing memories", existing_memories.len());

        Ok(Self {
            user_id, agent_id, 
            retrieved_facts: embedding_messages.iter().map(|embedding_message| embedding_message.content.clone()).collect(), 
            old_memories: existing_memories,
        })
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntrySimplified {
    pub id: Uuid,
    pub content: String,
    pub event: MemoryEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdateToolcall {
    pub memory: Vec<MemoryEntrySimplified>,
    pub input: Option<MemoryUpdateToolInput>,
}

#[async_trait::async_trait]
impl LlmTool for MemoryUpdateToolcall {
    type ToolInput = MemoryUpdateToolInput;

    fn tool_input(&self) -> Option<Self::ToolInput> { self.input.clone() }
    fn set_tool_input(&mut self, tool_input: Self::ToolInput) { self.input = Some(tool_input); }

    fn system_prompt(input: &Self::ToolInput) -> String {
        let retrieved_facts_text = serde_json::to_string_pretty(&input.retrieved_facts).unwrap_or_else(|_| "[]".to_string());
        let memories_text = serde_json::to_string_pretty(&input.old_memories).unwrap_or_else(|_| "[]".to_string());
        format!(
            r#"You are a smart memory manager which controls the memory of a system.
You can perform four operations: (1) add into the memory, (2) update the memory, (3) delete from the memory, and (4) no change.
The memory entries are identified by a unique UUID.

Based on the above four operations, the memory will change.

Compare newly retrieved facts with the existing memory. For each new fact, decide whether to:
- ADD: Add it to the memory as a new element.
- UPDATE: Update an existing memory element.
- DELETE: Delete an existing memory element.
- NONE: Make no change (if the fact is already present or irrelevant).

There are specific guidelines to select which operation to perform:

1. **Add**: If the retrieved facts contain new information not present in the memory, then you have to add it. For the `id` field of a new memory, you **MUST** use the nil UUID `00000000-0000-0000-0000-000000000000`.
- **Example**:
    - Old Memory:
        [
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174000",
                "content" : "User is a software engineer"
            }}
        ]
    - Retrieved facts: ["Name is John"]
    - New Memory:
        {{
            "memory" : [
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174000",
                    "content" : "User is a software engineer",
                    "event" : "NONE"
                }},
                {{
                    "id" : "00000000-0000-0000-0000-000000000000",
                    "content" : "Name is John",
                    "event" : "ADD"
                }}
            ]
        }}

2. **Update**: If the retrieved facts contain information that is already present in the memory but the information is totally different, then you have to update it. 
If the retrieved fact contains information that conveys the same thing as the elements present in the memory, then you have to keep the fact which has the most information. 
Example (a) -- if the memory contains "User likes to play cricket" and the retrieved fact is "Loves to play cricket with friends", then update the memory with the retrieved facts.
Example (b) -- if the memory contains "Likes cheese pizza" and the retrieved fact is "Loves cheese pizza", then you do not need to update it because they convey the same information.
If the direction is to update the memory, then you have to update it.
Please keep in mind while updating you have to keep the same ID.
Please note to return the UUIDs in the output from the input UUIDs only and do not generate any new UUIDs for existing memories.
- **Example**:
    - Old Memory:
        [
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174001",
                "content" : "I really like cheese pizza"
            }},
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174002",
                "content" : "User is a software engineer"
            }},
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174003",
                "content" : "User likes to play cricket"
            }}
        ]
    - Retrieved facts: ["Loves chicken pizza", "Loves to play cricket with friends"]
    - New Memory:
        {{
        "memory" : [
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174001",
                    "content" : "Loves cheese and chicken pizza",
                    "event" : "UPDATE"
                }},
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174002",
                    "content" : "User is a software engineer",
                    "event" : "NONE"
                }},
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174003",
                    "content" : "User likes to play cricket",
                    "event" : "UPDATE"
                }}
            ]
        }}


3. **Delete**: If the retrieved facts contain information that contradicts the information present in the memory, then you have to delete it. Or if the direction is to delete the memory, then you have to delete it.
Please note to return the UUIDs in the output from the input UUIDs only and do not generate any new UUIDs.
- **Example**:
    - Old Memory:
        [
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174004",
                "content" : "Name is John"
            }},
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174005",
                "content" : "Loves cheese pizza"
            }}
        ]
    - Retrieved facts: ["Dislikes cheese pizza"]
    - New Memory:
        {{
        "memory" : [
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174004",
                    "content" : "Name is John",
                    "event" : "NONE"
                }},
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174005",
                    "content" : "Loves cheese pizza",
                    "event" : "DELETE"
                }}
        ]
        }}

4. **No Change**: If the retrieved facts contain information that is already present in the memory, then you do not need to make any changes.
- **Example**:
    - Old Memory:
        [
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174006",
                "content" : "Name is John"
            }},
            {{
                "id" : "123e4567-e89b-12d3-a456-426614174007",
                "content" : "Loves cheese pizza"
            }}
        ]
    - Retrieved facts: ["Name is John"]
    - New Memory:
        {{
        "memory" : [
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174006",
                    "content" : "Name is John",
                    "event" : "NONE"
                }},
                {{
                    "id" : "123e4567-e89b-12d3-a456-426614174007",
                    "content" : "Loves cheese pizza",
                    "event" : "NONE"
                }}
            ]
        }}

Below is the current content of my memory which I have collected till now. You have to update it in the following format only:
```
{memories}
```

The new retrieved facts are mentioned in the triple backticks. You have to analyze the new retrieved facts and determine whether these facts should be added, updated, or deleted in the memory.
```
{facts}
```

You must call the `update_memory` tool to perform the memory updates. The arguments you provide to the tool must follow the JSON structure shown below and adhere to all the instructions.
{{
    "memory" : [
        {{
            "id" : "<ID of the memory>",                # Use existing ID for updates/deletes, or new ID for additions
            "content" : "<Content of the memory>",         # Content of the memory
            "event" : "<Operation to be performed>",    # Must be "ADD", "UPDATE", "DELETE", or "NONE"
        }},
        ...
    ]
}}

Follow the instruction mentioned below when constructing the arguments for the tool call:
- Do not return anything from the custom few shot prompts provided above.
- If the current memory is empty, then you have to add the new retrieved facts to the memory.
- The `memory` key should be the same if no changes are made.
- If there is an addition, you must use the nil UUID `00000000-0000-0000-0000-000000000000` as the new ID.
- If there is a deletion, the memory key-value pair should be removed from the memory.
- If there is an update, the ID key should remain the same and only the value needs to be updated.

Your response must only be the tool call.
"#,
            memories = memories_text,
            facts = retrieved_facts_text
        )
    }

    fn tools() -> Vec<FunctionObject> {
        vec![FunctionObject {
            name: "update_memory".to_string(),
            description: Some("Updates the memory with new facts, including adding, modifying, or deleting entries.".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {
                    "memory": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": {
                                    "type": "string",
                                    "format": "uuid",
                                    "description": "ID of the memory in UUID format. Use existing ID for updates/deletes, or the nil UUID for additions."
                                },
                                "content": {
                                    "type": "string",
                                    "description": "Content of the memory."
                                },
                                "event": {
                                    "type": "string",
                                    "enum": ["ADD", "UPDATE", "DELETE", "NONE"],
                                    "description": "Operation to be performed."
                                },
                            },
                            "required": ["id", "content", "event"]
                        }
                    }
                },
                "required": ["memory"],
                "additionalProperties": false
            })),
            strict: Some(true),
        }]
    }
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for MemoryUpdateToolcall {
    type CTX = Mem0Engine;
    type RETURN = BatchUpdateSummary;

    fn name() -> &'static str { "update_memory" }

    async fn execute(&self, llm_response: &LLMRunResponse, execution_context: &Self::CTX) -> Result<Self::RETURN> {
        execution_context.add_usage_report(llm_response).await?;

        let input = self.tool_input()
            .ok_or(anyhow::anyhow!("[MemoryUpdateToolcall::execute] No input found"))?;

        if self.memory.is_empty() {
            return Ok(BatchUpdateSummary {
                added: 0,
                updated: 0,
                deleted: 0,
            });
        }

        let memory_update_entries = self.memory.clone().into_iter()
            .map(|entry| MemoryUpdateEntry {
                id: entry.id,
                user_id: input.user_id,
                agent_id: input.agent_id.clone().unwrap_or_default(),
                event: entry.event,
                content: entry.content,
            }).collect();

        let summary = execution_context.vector_db_batch_update(memory_update_entries).await?;
        Ok(summary)
    }
}
