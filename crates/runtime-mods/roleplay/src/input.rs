use anyhow::{anyhow, Result};

use metastable_common::ModuleClient;
use metastable_database::{QueryCriteria, SqlxCrud, SqlxFilterQuery};
use metastable_runtime::{Message, Prompt, SystemConfig};
use serde::{Deserialize, Serialize};
use metastable_clients::PostgresClient;
use sqlx::types::Uuid;

use crate::RoleplaySession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoleplayInput {
    ContinueSession(Uuid, Prompt), // session_id
    RegenerateSession(Uuid), // session_id
}

impl RoleplayInput {
    pub async fn build_inputs(&self, db: &PostgresClient, system_config: &SystemConfig) -> Result<Vec<Prompt>> {
        let mut tx = db.get_client().begin().await?;

        let (session, character, user, user_message) = match &self {
            Self::ContinueSession(session_id, user_message) => {
                let session = RoleplaySession::find_one_by_criteria(
                    QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
                    &mut *tx
                ).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] Session not found"))?;

                let user = session.fetch_owner(&mut *tx).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] User not found"))?;
                let character = session.fetch_character(&mut *tx).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] Character not found"))?;

                (session, character, user, user_message.clone())
            },
            Self::RegenerateSession(session_id) => {
                let session = RoleplaySession::find_one_by_criteria(
                    QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
                    &mut *tx
                ).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] Session not found"))?;

                let user = session.fetch_owner(&mut *tx).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] User not found"))?;
                let character = session.fetch_character(&mut *tx).await?
                    .ok_or(anyhow!("[RoleplayCharacterCreationV0Agent::build_input] Character not found"))?;

                (session, character, user, Prompt::empty())
            }
        };

        let system_prompt = character.build_system_prompt(&system_config.system_prompt, &user.user_aka);
        let first_message = character.build_first_message(&user.user_aka);

        let mut prompts = vec![system_prompt, first_message];
        let history = session
            .fetch_history(&mut *tx).await?
            .iter()
            .flat_map(|v| Prompt::from_message(v))
            .collect::<Vec<_>>();

        prompts.extend(history);
        prompts.push(user_message.clone());

        if let Self::RegenerateSession(_) = &self {
            if prompts.len() < 3 {
                return Err(anyhow!("[RoleplayInput::build_inputs] too little messages to do regenerate"));
            }
            prompts.pop(); // pop the empty user msg
            prompts.pop(); // pop the last message
        }

        tx.commit().await?;
        Ok(prompts)
    }

    pub async fn handle_outputs(&self, db: &PostgresClient, message: &Message) -> Result<()> {
        let mut tx = db.get_client().begin().await?;
        let session_id = match &self {
            Self::ContinueSession(session_id, _) => session_id.clone(),
            Self::RegenerateSession(session_id) => session_id.clone(),
        };
        let mut session = RoleplaySession::find_one_by_criteria(
            QueryCriteria::new().add_valued_filter("id", "=", session_id.clone()),
            &mut *tx
        ).await?
            .ok_or(anyhow!("[RoleplayInput::handle_outputs] Session not found"))?;

        let message = message.clone();
        let message = message.create(&mut *tx).await?;
        session.append_message_to_history(&message.id, &mut *tx).await?;
        Ok(())
    }
}
