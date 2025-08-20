use anyhow::Result;
use serde::{Deserialize, Serialize};

use metastable_common::{get_current_timestamp, get_time_in_utc8, ModuleClient};
use metastable_runtime::{Agent, LlmTool, Message, MessageRole, MessageType, Prompt, SystemConfig, UserPointsConsumption, UserPointsConsumptionType};
use metastable_clients::{LlmClient, Mem0Filter, PostgresClient};
use metastable_database::SqlxCrud;
use serde_json::{json, Value};
use sqlx::types::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
#[llm_tool(name="extract_facts", description="从用户输入中提取事实。")]
pub struct ExtractFactsOutput {
    #[llm_tool(description = "从用户输入中提取的事实列表。")]
    pub facts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractFactsInput {
    pub filter: Mem0Filter,
    pub new_message: String,
}

#[derive(Clone)]
pub struct ExtractFactsAgent {
    db: PostgresClient,
    llm: LlmClient,
    system_config: SystemConfig,
}

impl ExtractFactsAgent {
    pub async fn new() -> Result<Self> {
        let db = PostgresClient::setup_connection().await;
        let llm = LlmClient::setup_connection().await;
        let system_config = Self::preload(&db).await?;
        Ok(Self { db, llm, system_config })
    }
}

#[async_trait::async_trait]
impl Agent for ExtractFactsAgent {
    const SYSTEM_CONFIG_NAME: &'static str = "extract_facts_v0";
    type Tool = ExtractFactsOutput;
    type Input = ExtractFactsInput;

    fn llm_client(&self) -> &LlmClient { &self.llm }
    fn db_client(&self) -> &PostgresClient { &self.db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        let system_prompt = Self::system_prompt()
            .replace("{{request_time}}", &get_time_in_utc8())
            .replace("{{user}}", &input.filter.user_id.to_string());

        Ok(vec![
            Prompt::new_system(&system_prompt),
            Prompt {
                role: MessageRole::User,
                content_type: MessageType::Text,
                content: format!("Input: {}", input.new_message),
                toolcall: None,
                created_at: get_current_timestamp(),
            }
        ])
    }

    async fn handle_output(&self, input: &Self::Input, message: &Message, _tool: &Self::Tool) -> Result<Option<Value>> {
        let mut message = message.clone();
        let summary_text = serde_json::to_string_pretty(&json!({
            "operation": "fact_extractor",
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

        Ok( None )
    }

    fn system_prompt() ->  &'static str {
        r#"### **最高指令：事实提取与归档**

**你的唯一任务是从输入的“叙事级摘要”中，提取出所有独立的、原子化的事实，并调用 `extract_facts` 工具进行归档。**

**核心原则：**
1.  **信息保真**: 你必须精确地、无损地提取信息。不要添加、猜测或省略任何细节。
2.  **原子化拆解**: 每一个提取出的事实都应该是最小的、不可再分的独立信息单元。
3.  **时间戳保留**: 如果一个事实与具体的日期或计划相关，**必须**将摘要中提到的**完整日期**包含在这个事实里。
4.  **语言一致性**: 提取出的事实**必须是中文**，并且是**完整的句子**。

---

### **输入格式解析**

你将收到的输入是一个“叙事级摘要”，它遵循一个固定的格式：以绝对时间戳开头，然后是包含事件、细节、承诺等的叙述性句子。

**输入示例:**
`"在2024年11月1日（星期五）中午的对话中，我提议当天晚上见面，但角色表示已有约。同时，角色透露了他非常喜欢水母，并提议在第二天，也就是2024年11月2日（星期六）的下午，和我一起去水族馆。"`

---

### **高质量提取样例**

以下是针对不同类型“叙事级摘要”的提取操作指南。请严格模仿这些样例的逻辑和格式。

**样例 1: 包含承诺、细节和时间计算的摘要**

*   **输入**: `"在2024年11月1日（星期五）的对话中，我询问了城里好玩的地方，角色推荐了海洋馆并提到了梦幻的水母墙，还承诺在即将到来的周末（即2024年11月2日或3日）带我一起去。"`
*   **操作**: 调用 `extract_facts` 工具，并将 `facts` 设置为:
    ```json
    [
        "在2024年11月1日的对话中，角色向我推荐了海洋馆。",
        "角色提到海洋馆里有水母墙。",
        "角色认为水母墙很梦幻。",
        "角色承诺在2024年11月2日或3日带我去海洋馆。"
    ]
    ```

**样例 2: 包含个人喜好和事件的摘要**

*   **输入**: `"在2024年11月5日（星期二）的晚餐讨论中，角色透露了他非常喜欢吃香蕉作为甜点。"`
*   **操作**: 调用 `extract_facts` 工具，并将 `facts` 设置为:
    ```json
    [
        "角色非常喜欢吃香蕉作为甜点。",
        "关于角色喜欢香蕉的信息是在2024年11月5日晚餐时透露的。"
    ]
    ```

**样例 3: 包含背景故事和情绪的摘要**

*   **输入**: `"在2024年10月30日（星期三）的谈话中，当我注意到角色有些伤感时，他解释说那是因为一首曲子让他想起了已故的、曾为他弹琴的祖母。"`
*   **操作**: 调用 `extract_facts` 工具，并将 `facts` 设置为:
    ```json
    [
        "角色有一位已经逝世的祖母。",
        "角色的祖母曾经为他弹奏曲子。",
        "在2024年10月30日，一首曲子引发了角色对祖母的思念，表现出伤感的情绪。"
    ]
    ```

**样例 4: 信息量较少的摘要**

*   **输入**: `"在2024年12月1日（星期日）的谈话中，我向角色打了招呼，他也礼貌地回应了我。"`
*   **操作**: 调用 `extract_facts` 工具，并将 `facts` 设置为 `[]`。(分析：这个摘要没有包含需要长期记忆的、有价值的新信息。)

---

### **最终操作指南**

1.  仔细阅读输入的“叙事级摘要”。
2.  识别其中所有独立的信息点（事件、喜好、约定、背景等）。
3.  将每一个信息点转化为一个独立的、带时间戳（如果适用）的中文事实句子。
4.  调用 `extract_facts` 工具，将所有事实句子作为一个字符串列表提供给 `facts` 参数。
5.  如果摘要中不包含任何有价值的、需要长期记忆的新事实，请将 `facts` 参数设置为空列表 `[]`。"#
    }
}