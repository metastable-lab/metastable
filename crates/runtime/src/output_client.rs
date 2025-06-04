use anyhow::Result;

use crate::Message;

#[async_trait::async_trait]
pub trait OutputClient {
    async fn send_message(&self, message: &impl Message) -> Result<()>;
}