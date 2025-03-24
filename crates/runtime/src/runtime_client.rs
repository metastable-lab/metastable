use anyhow::Result;
use async_openai::types::FunctionCall;
use voda_database::Database;

use crate::SystemConfig;

use super::{Character, ConversationMemory, HistoryMessage, User};

#[async_trait::async_trait]
pub trait RuntimeClient: Clone + Send + Sync + 'static {
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

    async fn execute_function_call(
        &self, call: &FunctionCall
    ) -> Result<String>;
}
