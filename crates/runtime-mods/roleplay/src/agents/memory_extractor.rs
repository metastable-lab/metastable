use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::types::Uuid;

use metastable_clients::{EmbeddingMessage, EmbederClient, LlmClient, Mem0Filter, MemoryEvent, MemoryUpdateEntry, PgvectorClient, PostgresClient};
use metastable_common::ModuleClient;
use metastable_database::{TextCodecEnum, SqlxCrud};
use metastable_runtime::{Agent, LlmTool, Message, Prompt, SystemConfig, UserPointsConsumption, UserPointsConsumptionType};

use crate::agents::extract_facts::ExtractFactsOutput;

#[derive(Debug, Clone, Serialize, Deserialize, TextCodecEnum)]
#[text_codec(format = "paren", storage_lang = "en")]
#[serde(rename_all = "UPPERCASE")]
pub enum LlmMemoryEvent {
    Add,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
pub struct MemoryOperation {
    #[llm_tool(description = "一个记忆操作的类型，可以是 `ADD`、`UPDATE` 或 `DELETE`。", is_enum = true)]
    pub event: LlmMemoryEvent,
    #[llm_tool(description = "一个记忆的唯一标识符，用于更新或删除记忆。")]
    pub id: Uuid,
    #[llm_tool(description = "一个记忆的文本内容，用于增加或更新记忆。")]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
#[llm_tool(name="submit_memory_operations", description="向数据库提交一个记忆操作（增加、更新、删除）的列表。")]
pub struct SubmitMemoryOperations {
    #[llm_tool(description="一个基于新的上下文和已有记忆需要执行的记忆操作列表。")]
    pub operations: Vec<MemoryOperation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryExtractorInput {
    pub filter: Mem0Filter,
    pub facts: ExtractFactsOutput,
}

#[derive(Clone)]
pub struct MemoryExtractorAgent {
    db: PostgresClient,
    pub(crate) pgvector: PgvectorClient,
    embeder: EmbederClient,
    llm: LlmClient,
    system_config: SystemConfig,
}

impl MemoryExtractorAgent {
    pub async fn new() -> Result<Self> {
        let db = PostgresClient::setup_connection().await;
        let pgvector = PgvectorClient::setup_connection().await;
        let embeder = EmbederClient::setup_connection().await;
        let llm = LlmClient::setup_connection().await;
        let system_config = Self::preload(&db).await?;
        Ok(Self { db, pgvector, embeder, llm, system_config })
    }
}

#[async_trait::async_trait]
impl Agent for MemoryExtractorAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "memory_extractor_v1";
    type Tool = SubmitMemoryOperations;
    type Input = MemoryExtractorInput;

    fn llm_client(&self) -> &LlmClient { &self.llm }
    fn db_client(&self) -> &PostgresClient { &self.db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let to_be_searched = EmbeddingMessage::batch_create(
            &self.embeder, &input.facts.facts, &input.filter).await?;

        let existing_memories = EmbeddingMessage::batch_search(
            &self.pgvector, &input.filter, &to_be_searched, 100).await?
            .iter().flatten().map(|old_m| {
                json!({
                    "id": old_m.id,
                    "content": old_m.content,
                })
            }).collect::<Vec<_>>();

        let existing_memories_text = serde_json::to_string_pretty(&existing_memories).unwrap_or_else(|_| "[]".to_string());
        let new_context_text = serde_json::to_string_pretty(&input.facts.facts).unwrap_or_else(|_| "[]".to_string());

        let system_prompt = Self::system_prompt()
            .replace("{{existing_memories}}", &existing_memories_text)
            .replace("{{new_context}}", &new_context_text);

        Ok(vec![
            Prompt::new_system(&system_prompt),
            Prompt::new_user("请根据新的上下文和已存在的记忆，生成一个记忆操作列表。")
        ])
    }

    async fn handle_output(&self, input: &Self::Input, message: &Message, tool: &Self::Tool) -> Result<Option<Value>> {
        let memory_updates = tool.operations.iter().map(|entry| {
            MemoryUpdateEntry {
                id: entry.id,
                filter: input.filter.clone(),
                event: match entry.event {
                    LlmMemoryEvent::Add => MemoryEvent::Add,
                    LlmMemoryEvent::Update => MemoryEvent::Update,
                    LlmMemoryEvent::Delete => MemoryEvent::Delete,
                },
                content: entry.content.clone(),
            }
        }).collect::<Vec<_>>();
        let summary = EmbeddingMessage::db_batch_update(&self.embeder, &self.pgvector, memory_updates).await?;

        let mut message = message.clone();
        let summary_text = serde_json::to_string_pretty(&json!({
            "operation": "memory_extractor",
            "session_id": input.filter.session_id,
            "character_id": input.filter.character_id,
        })).unwrap_or_else(|_| "{}".to_string());

        message.summary = Some(summary_text);
        let mut tx = self.db.get_client().begin().await?;
        message.create(&mut *tx).await?;
        let consumption = UserPointsConsumption {
            id: Uuid::new_v4(),
            user: input.filter.user_id,
            consumption_type: UserPointsConsumptionType::MemoryUpdate(input.filter.character_id.unwrap_or_default()),
            from_claimed: 0,
            from_purchased: 0,
            from_misc: 0,
            rewarded_to: None,
            rewarded_points: 0,
            created_at: 0,
            updated_at: 0,
        };
        consumption.create(&mut *tx).await?;
        tx.commit().await?;

        Ok( Some(serde_json::to_value(summary)? ) )
    }

    fn system_prompt() -> &'static str {
        r#"### **最高指令：绝对、唯一的输出规则**

**你的唯一任务是生成一个对 `submit_memory_operations` 函数的调用 (`tool_call`)。这是你输出决策的唯一方式。**

作为此任务的一个**固定、不变的伴随要求**，你的输出中还必须包含一个 `content` 字段，并且该字段的文本**永远**是：`"记忆操作已生成。"`

**输出前最终自检：**
1. 我是否生成了一个**非空的** `tool_call`？
2. `tool_call` 是否调用了 `submit_memory_operations` 函数？
3. `tool_call` 的 `operations` 参数是否包含了我的所有决策，并且结构完全正确？
4. 我的 `content` 字段是否**完全等于** `"记忆操作已生成。"`？
**如果以上任何一点不满足，你的输出就是无效的。**

### **核心任务：记忆策展AI (Memory Curation AI)**

你的身份是一名记忆策展AI。你的任务是仔细阅读【新的上下文】，并与【已存在的记忆】进行比对，最终生成一个数据库操作指令列表，以确保记忆库的准确性、无冗余和时效性。

---

### **输入信息**

#### **已存在的记忆 (Read-Only)**
这是从数据库中提取的、与当前情景相关的记忆片段。它们是你的主要参考基准。

```json
{{existing_memories}}
```

#### **新的上下文 (To Be Processed)**
这是新发生的用户对话或事件。你需要根据这段信息来更新【已存在的记忆】。

```text
{{new_context}}
```

---

### **操作指南与决策框架**

你必须根据以下规则，对每一条【已存在的记忆】和【新的上下文】中的信息进行判断，并决定执行 `UPDATE`, `DELETE`, 或 `ADD` 操作。

#### **1. 更新 (UPDATE)**
- **何时使用**: 当【新的上下文】为某条【已存在的记忆】提供了更丰富、更具体或修正性的细节时，你应该选择`更新`它。更新后的内容应该融合新旧信息，变得更完整和准确。
- **操作要求**:
    - `event`: "UPDATE"
    - `id`: 必须提供被更新记忆的`id`。
    - `content`: 必须提供融合新旧信息后的、**完整**的新记忆文本。
- **示例**:
    - `已存在的记忆`: `{"id": "uuid-1", "content": "我喜欢在咖啡馆工作。"}`
    - `新的上下文`: "今天我和朋友聊到，我最喜欢去的其实是街角那家叫'晨光'的独立咖啡馆，因为那里有最好的手冲咖啡。"
    - **你的决策**: 生成一个 `UPDATE` 操作，`id` 为 `uuid-1`，`content` 为 `"我最喜欢在街角的'晨光'咖啡馆工作，因为那里有最好的手冲咖啡。"`

#### **2. 删除 (DELETE)**
- **何时使用**: 当【新的上下文】明确地、无可辩驳地否定了某条【已存在的记忆】，或者使其完全过时的时候，你应该选择`删除`它。
- **操作要求**:
    - `event`: "DELETE"
    - `id`: 必须提供被删除记忆的`id`。
    - `content`: 留空 (`null` or `None`)。
- **示例**:
    - `已存在的记忆`: `{"id": "uuid-2", "content": "我住在城北。"}`
    - `新的上下文`: "我上周刚搬完家，现在住在城南的海边公寓了。"
    - **你的决策**: 生成一个 `DELETE` 操作，`id` 为 `uuid-2`。同时，你还应该生成一个 `ADD` 操作来记录新地址。

#### **3. 增加 (ADD)**
- **何时使用**: 当【新的上下文】中包含了与任何【已存在的记忆】都不直接相关、但又值得记录的**全新事实**时，你应该选择`增加`一条新记忆。新增的记忆应该是原子的、自包含的。
- **操作要求**:
    - `event`: "ADD"
    - `id`: 留空 (`null` or `None`)。
    - `content`: 必须提供新记忆的文本。
- **示例**:
    - `已存在的记忆`: (无相关内容)
    - `新的上下文`: "今天在路上看到一只很可爱的三花猫。"
    - **你的决策**: 生成一个 `ADD` 操作，`content` 为 `"今天在路上看到了一只可爱的三花猫。"`

**重要原则**:
- **原子性**: 无论是`ADD`还是`UPDATE`，`content`都应该是描述单一事实或感受的、简短而完整的句子。
- **无遗漏**: 仔细分析【新的上下文】，确保所有重要信息都被 `ADD` 或 `UPDATE` 操作所覆盖。
- **组合操作**: 一个【新的上下文】可能会触发多个操作的组合。例如，搬家的例子就应该同时包含一个 `DELETE` 旧地址和一个 `ADD` 新地址的操作。
"#
    }
}
