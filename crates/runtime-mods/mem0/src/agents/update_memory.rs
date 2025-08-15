use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_common::get_current_timestamp;
use metastable_runtime::{Agent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig};
use metastable_clients::{LlmClient, Mem0Filter, PostgresClient};
use serde_json::{json, Value};
use sqlx::types::Uuid;

use crate::{init_mem0, EmbeddingMessage, Mem0Engine};
use crate::pgvector::{MemoryEvent, MemoryUpdateEntry};

init_mem0!();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntrySimplified {
    pub id: Uuid,
    pub content: String,
    pub event: MemoryEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
pub struct UpdateMemoryOutput {
    pub memory: Vec<MemoryEntrySimplified>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryInput {
    pub filter: Mem0Filter,
    pub existing_memories: Vec<EmbeddingMessage>,
}

#[derive(Clone)]
pub struct UpdateMemoryAgent {
    mem0_engine: Arc<Mem0Engine>,
    system_config: SystemConfig,
}

impl UpdateMemoryAgent {
    pub async fn new() -> Result<Self> {
        let mem0_engine = get_mem0_engine().await;
        let system_config = Self::preload(&mem0_engine.data_db).await?;

        Ok(Self { 
            mem0_engine: Arc::new(mem0_engine.clone()), 
            system_config 
        })
    }
}

#[async_trait::async_trait]
impl Agent for UpdateMemoryAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "update_memory_v0";
    type Tool = UpdateMemoryOutput;
    type Input = UpdateMemoryInput;

    fn llm_client(&self) -> &LlmClient { &self.mem0_engine.llm }
    fn db_client(&self) -> &PostgresClient { &self.mem0_engine.data_db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let retrived_facts = input.existing_memories.iter()
            .map(|embedding| embedding.content.clone()).collect::<Vec<String>>();

        let old_memories = EmbeddingMessage::batch_search(
            &self.mem0_engine, &input.filter, &input.existing_memories, 5
        ).await?
            .iter().flatten().map(|old_m| {
                json!({
                    "id": old_m.id,
                    "content": old_m.content,
                })
            }).collect::<Vec<_>>();

        let retrieved_facts_text = serde_json::to_string_pretty(&retrived_facts).unwrap_or_else(|_| "[]".to_string());
        let old_memories_text = serde_json::to_string_pretty(&old_memories).unwrap_or_else(|_| "[]".to_string());

        let system_prompt = Self::system_prompt()
            .replace("{{memories}}", old_memories_text)
            .replace("{{facts}}", retrieved_facts_text);

        Ok(vec![
            Prompt::new_system(system_prompt),
            Prompt {
                role: MessageRole::User,
                content_type: MessageType::Text,
                content: "Please update the memory based on the new facts.".to_string(),
                toolcall: None,
                created_at: get_current_timestamp(),
            }
        ])
    }

    async fn handle_output(&self, input: &Self::Input, message: &Message, tool: &Self::Tool) -> Result<Option<Value>> {
        let memory_updates = tool.memory.iter().map(|entry| {
            MemoryUpdateEntry {
                id: entry.id,
                filter: input.filter.clone(),
                event: entry.event,
                content: entry.content,
            }
        }).collect::<Vec<_>>();

        let summary = self.mem0_engine.vector_db_batch_update(memory_updates).await?;
        Ok(Some(serde_json::to_value(summary)?))
    }

    fn system_prompt() ->  &'static str {
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
{{memories}}
```

The new retrieved facts are mentioned in the triple backticks. You have to analyze the new retrieved facts and determine whether these facts should be added, updated, or deleted in the memory.
```
{{facts}}
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
"#
    }
}