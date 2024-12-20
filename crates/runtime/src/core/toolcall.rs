use anyhow::Result;
use async_openai::types::ChatCompletionTool;

#[allow(async_fn_in_trait)]
pub trait ToolCall {
    fn into_openai_toolcall(self) -> ChatCompletionTool;
    async fn execute(self) -> Result<()>;
}