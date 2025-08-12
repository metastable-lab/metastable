use anyhow::Result;
use serde::{Deserialize, Serialize};
use metastable_runtime::{ExecutableFunctionCall, LLMRunResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShowStoryOptionsToolCall {
    pub options: Vec<String>,
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for ShowStoryOptionsToolCall {
    type CTX = ();
    type RETURN = Vec<String>;

    fn name() -> &'static str { "show_story_options" }

    async fn execute(&self, 
        _llm_response: &LLMRunResponse, 
        _execution_context: &Self::CTX
    ) -> Result<Vec<String>> {
        tracing::info!("[ShowStoryOptionsToolCall::execute] Showing story options: {:?}", self.options);
        Ok(self.options.clone())
    }
}