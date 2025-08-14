use std::sync::Arc;
use anyhow::Result;
use sqlx::PgPool;

use crate::Memory;


#[async_trait::async_trait]
pub trait Engine: Clone + Send + Sync + 'static {
    const NAME: &'static str;
    type MemoryType: Memory;

    fn get_price(&self) -> u64;

    async fn preload(db: Arc<PgPool>) -> Result<()>;

    async fn on_shutdown(&self) -> Result<()>;
    async fn on_new_message(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<()>;
    async fn on_rollback(&self, message: &<Self::MemoryType as Memory>::MessageType) -> Result<()>;
}