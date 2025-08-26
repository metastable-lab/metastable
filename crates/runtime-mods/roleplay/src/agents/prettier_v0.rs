use anyhow::Result;

use metastable_common::ModuleClient;
use metastable_runtime::{Agent, Message, MessageRole, MessageType, Prompt, SystemConfig};
use metastable_clients::{PostgresClient, LlmClient};
use serde_json::Value;

use metastable_runtime::LlmTool;
use serde::{Deserialize, Serialize};

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "apply_chat_style",
    description = "Apply the chat style with tailwindcss classes."
)]
pub struct ApplyChatStyle {
    #[llm_tool(description = "The container of the chat bubble.")]
    pub container: String,
    #[llm_tool(description = "The content of the chat bubble.")]
    pub content: String,
    #[llm_tool(description = "The tag of the chat bubble, e.g. the name of the user.")]
    pub tag: String,
}

#[derive( Clone)]
pub struct PrettierV0Agent {
    db: PostgresClient,
    llm: LlmClient,
    system_config: SystemConfig,
}

impl PrettierV0Agent {
    pub async fn new() -> Result<Self> {
        let db = PostgresClient::setup_connection().await;
        let llm = LlmClient::setup_connection().await;
        let system_config = Self::preload(&db).await?;

        Ok(Self { db, llm, system_config })
    }
}

#[async_trait::async_trait]
impl Agent for PrettierV0Agent {
    const SYSTEM_CONFIG_NAME: &'static str = "prettier_v0";
    type Tool = ApplyChatStyle;
    type Input = ();

    fn llm_client(&self) -> &LlmClient { &self.llm }
    fn db_client(&self) -> &PostgresClient { &self.db }
    fn model() -> &'static str { "google/gemini-2.5-flash-lite" }
    fn system_config(&self) -> &SystemConfig { &self.system_config }

    async fn build_input(&self, input: &Self::Input) -> Result<Vec<Prompt>> {
        
        let sys_msg = Prompt::new_system(Self::system_prompt());

        let first_msg = Prompt {
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: "你好，发给我图片吧，我会帮你识别".to_string(),
            toolcall: None,
            created_at: 1,
        };

        let message_0 = Prompt { 
            role: MessageRole::User, 
            content_type: MessageType::Image, 
            content: "https://img.fine.wtf/Screenshot%202025-08-26%20at%204.07.25%E2%80%AFPM.png".to_string(), 
            toolcall: None, 
            created_at: 2,

            
        };

        let message_1 = Prompt { 
            role: MessageRole::User, 
            content_type: MessageType::Text, 
            content: "我想要这个样式的聊天框".to_string(), 
            toolcall: None, 
            created_at: 3,
        };

        Ok(vec![sys_msg, first_msg, message_0, message_1])
    }

    async fn handle_output(&self, input: &Self::Input, message: &Message, tool: &Self::Tool) -> Result<(Message, Option<Value>)> {

        println!("message: {:?}", message);

        println!("apply_chat_style: {:?}", tool);
        Ok((message.clone(), None))
    }

    fn system_prompt() ->  &'static str {
        r#"Your mission is to act as an expert UI designer and frontend developer, with a specialization in creating pixel-perfect UI replicas using only Tailwind CSS. Your goal is to precisely and completely replicate a chat bubble style from an image provided by the user.

**Your primary directive is total fidelity to the source image. No detail is too small or too complex to be ignored.** You must meticulously analyze the image, paying extreme attention to every single detail of the chat bubble's appearance. This includes, but is not limited to:

- **Illustrations & Graphical Elements:** This is the most critical part. You must deconstruct any illustrations, icons, patterns, or complex graphical elements into shapes, gradients, and layers that can be recreated with Tailwind CSS. You must use pseudo-elements (`before:`, `after:`), absolute positioning, z-index, transforms, and complex background gradients to rebuild these visuals. Do not simplify or omit them. Your CSS must reproduce the illustration itself.
- **Colors:** Background colors, text colors, border colors (including multi-step gradients and subtle variations).
- **Typography:** Font family, size, weight, style (e.g., italic), letter spacing, line height, and any text shadows.
- **Layout & Sizing:** Precise padding, margins, width, height, and element alignment.
- **Borders & Effects:** Border width, style, color, corner roundness (including different radii for different corners), and complex multi-layer box shadows.

You must always use the `apply_chat_style` tool to output the styles. The output must be a tool call to `apply_chat_style` with the following fields, containing the Tailwind CSS classes:
- `container`: The classes for the main chat bubble container. This must include styles for any pseudo-elements needed for illustrations.
- `content`: The classes for the text content within the bubble.
- `tag`: The classes for any associated tags, like a username or timestamp.

Your generated CSS must be a perfect, pixel-perfect replication of the image. **Do not take any creative liberties.** Your output must be a faithful and complete reproduction of the source image. **Failure to replicate graphical elements is a failure to complete the task.**

Example:

user: (sends an image of a white chat bubble with a decorative blue and purple gradient ribbon folded over the top-right corner.)

assistant:
(call `apply_chat_style` with container="relative bg-white dark:bg-gray-800 rounded-lg shadow-lg p-4 after:content-[''] after:absolute after:top-0 after:right-0 after:w-[60px] after:h-[30px] after:bg-gradient-to-br after:from-blue-500 after:to-purple-600 after:rounded-tr-lg after:shadow-md before:content-[''] before:absolute before:top-[30px] before:right-0 before:w-0 before:h-0 before:border-l-[0px] before:border-r-[10px] before:border-t-[10px] before:border-l-transparent before:border-r-purple-700 before:border-t-purple-700", content="text-gray-900 dark:text-gray-100", tag="text-xs text-gray-500 dark:text-gray-400 mb-1")
"#
    }
}