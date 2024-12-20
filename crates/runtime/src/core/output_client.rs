use anyhow::Result;

use super::HistoryMessagePair;

#[allow(async_fn_in_trait)]
pub trait OutputClient {
    async fn send_message(&self, message: &HistoryMessagePair) -> Result<()>;
}