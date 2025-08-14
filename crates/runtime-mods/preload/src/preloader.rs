use anyhow::Result;
use metastable_runtime_roleplay::Character;

use metastable_clients::PostgresClient;
use metastable_runtime::{ModuleClient, SystemConfig, User, UserRole};
use metastable_database::{QueryCriteria, SqlxCrud, SqlxFilterQuery};

use crate::characters::get_characters_for_char_creation;

pub struct Preloader {
    db: PostgresClient,
}

impl Preloader {
    async fn setup() -> Self {
        let db = PostgresClient::setup_connection().await;
        Self { db }
    }

    async fn load_characters(&self) -> Result<()> {
        let mut tx = self.db.get_client().begin().await?;
        let admin_user = User::find_one_by_criteria(
            QueryCriteria::new().add_filter("role", "=", Some(UserRole::Admin.to_string())),
            &mut *tx
        ).await?
            .ok_or(anyhow::anyhow!("[Preloader::load_characters] No admin user found"))?;

        let characters = get_characters_for_char_creation(admin_user.id);
        for mut character in characters {
            let existing_char = Character::find_one_by_criteria(
                QueryCriteria::new().add_filter("name", "=", Some(character.name.clone())),
                &mut *tx
            ).await?;

            if existing_char.is_none() {
                character.create(&mut *tx).await?;
            } else {
                character.id = existing_char.unwrap().id;
                character.update(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        tracing::info!("[Preloader::load_characters] Characters preloaded");
        Ok(())
    }

    async fn load_system_configs(&self) -> Result<()> {
        let mut tx = self.db.get_client().begin().await?;
        let system_configs = vec![
            crate::sys_char_creation_v0::get_system_configs_for_char_creation(),
            crate::sys_roleplay_char_v0::get_system_configs_for_char_creation(),
            crate::sys_roleplay_char_v1::get_system_configs_for_char_creation(),
            crate::sys_roleplay_v0::get_system_configs_for_roleplay(),
            crate::sys_roleplay_v1::get_system_configs_for_roleplay(),
        ];
        for preload_config in system_configs {
            let existing_config = SystemConfig::find_one_by_criteria(
                QueryCriteria::new().add_filter("name", "=", Some(preload_config.name.clone())),
                &mut *tx
            ).await?;

            if existing_config.is_none() {
                preload_config.create(&mut *tx).await?;
            } else {
                let mut db_config = existing_config.unwrap();
                let mut needs_update = false;
                if db_config.system_prompt != preload_config.system_prompt {
                    db_config.system_prompt = preload_config.system_prompt.clone();
                    needs_update = true;
                }

                if db_config.openai_model != preload_config.openai_model {
                    db_config.openai_model = preload_config.openai_model.clone();
                    needs_update = true;
                }

                if db_config.openai_temperature != preload_config.openai_temperature {
                    db_config.openai_temperature = preload_config.openai_temperature;
                    needs_update = true;
                }

                if db_config.openai_max_tokens != preload_config.openai_max_tokens {
                    db_config.openai_max_tokens = preload_config.openai_max_tokens;
                    needs_update = true;
                }

                if db_config.functions != preload_config.functions {
                    db_config.functions = preload_config.functions.clone();
                    needs_update = true;
                }

                if needs_update {
                    db_config.system_prompt_version += 1;
                    db_config.update(&mut *tx).await?;
                }
            }
        }
        tx.commit().await?;
        tracing::info!("[Preloader::load_system_configs] System configs preloaded");
        Ok(())
    }

    pub async fn run() -> Result<()> {
        let preloader = Self::setup().await;
        preloader.load_characters().await?;
        preloader.load_system_configs().await?;
        Ok(())
    }
}