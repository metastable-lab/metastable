use anyhow::Result;
use voda_database::Database;

use crate::{ExecutableFunctionCall, SystemConfig};

use super::{Character, ConversationMemory, HistoryMessage, User};

#[allow(async_fn_in_trait)]
pub trait RuntimeClient<
    F: ExecutableFunctionCall
>: Clone + Send + Sync + 'static {
    fn get_price(&self) -> u64;
    fn get_db(&self) -> &Database;

    async fn run(
        &self, 
        character: &Character, user: &mut User, system_config: &SystemConfig,
        memory: &mut ConversationMemory, message: &HistoryMessage
    ) -> Result<HistoryMessage>;

    async fn regenerate(
        &self, 
        character: &Character, user: &mut User, system_config: &SystemConfig,
        memory: &mut ConversationMemory
    ) -> Result<HistoryMessage>;

    async fn find_system_config_by_character(
        &self, character: &Character
    ) -> Result<SystemConfig>;
}
