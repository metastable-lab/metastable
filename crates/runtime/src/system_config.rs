use async_openai::types::FunctionObject;
use serde::{Deserialize, Serialize};

use voda_db_macros::SqlxObject;
use sqlx::types::Json;
use voda_common::CryptoHash;
use voda_database::sqlx_postgres::SqlxPopulateId;

#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "system_configs"]
pub struct SystemConfig {
    pub id: CryptoHash,

    pub name: String,
    
    pub system_prompt: String,
    pub system_prompt_version: i64,

    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_temperature: f32,
    pub openai_max_tokens: i32,

    pub functions: Json<Vec<FunctionObject>>,
    pub updated_at: i64,
}

impl SqlxPopulateId for SystemConfig {
    fn sql_populate_id(&mut self) {
        self.id = CryptoHash::random();
    }
}

impl SystemConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: String,
        system_prompt: String,
        system_prompt_version: i64,
        openai_base_url: String,
        openai_model: String,
        openai_temperature: f32,
        openai_max_tokens: i32,
        functions: Vec<FunctionObject>,
        updated_at: i64,
    ) -> Self {
        Self {
            id: CryptoHash::random(),
            name,
            system_prompt,
            system_prompt_version,
            openai_base_url,
            openai_model,
            openai_temperature,
            openai_max_tokens,
            functions: Json(functions),
            updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voda_database::sqlx_postgres::{SqlxCrud, SqlxSchema};
    use sqlx::{types::Uuid, PgPool};
    use tokio::sync::OnceCell;

    static POOL: OnceCell<PgPool> = OnceCell::const_new();

    async fn init_db() -> &'static PgPool {
        POOL.get_or_init(|| async {
            let database_url = "postgresql://postgres:IgbXkBSlvHeHMAdAbXJyTQEPNJunQwHg@crossover.proxy.rlwy.net:10849/railway";
            let pool = PgPool::connect(&database_url).await
                .expect("Failed to connect to Postgres for init_db. Ensure DB is running and URL is correct.");

            let drop_sql = SystemConfig::drop_table_sql();
            sqlx::query(&drop_sql).execute(&pool).await.expect("Failed to drop system_configs table");
            
            let create_sql = SystemConfig::create_table_sql();
            sqlx::query(&create_sql).execute(&pool).await.expect("Failed to create system_configs table");
            
            pool
        }).await
    }

    fn create_sample_function() -> FunctionObject {
        FunctionObject {
            name: "get_weather".to_string(),
            description: Some("Get current weather in a given location".to_string()),
            parameters: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {
                        "type": "string",
                        "description": "The city and state, e.g. San Francisco, CA"
                    },
                    "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
                },
                "required": ["location"]
            })),
            strict: Some(false),
        }
    }

    fn create_sample_config() -> SystemConfig {
        SystemConfig {
            id: CryptoHash::default(),
            name: format!("test_config_{}", Uuid::new_v4().to_string()),
            system_prompt: "Test prompt".to_string(),
            system_prompt_version: 1i64,
            openai_base_url: "http://test".to_string(),
            openai_model: "test-gpt".to_string(),
            openai_temperature: 0.5,
            openai_max_tokens: 100,
            functions: Json(vec![create_sample_function()]),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_create_and_find_system_config() -> Result<(), anyhow::Error> {
        let pool = init_db().await;
        let mut tx = pool.begin().await?;

        let mut sample_config = create_sample_config();
        sample_config.name = format!("create_find_test_{}", Uuid::new_v4().to_string());

        let created_config = sample_config.clone().create(&mut tx).await?;
        assert_eq!(created_config.name, sample_config.name);

        let found_config_opt = SystemConfig::find_by_id(created_config.id.hash().to_vec(), &mut tx).await?;
        assert!(found_config_opt.is_some(), "Config should be found by ID");
        let found_config = found_config_opt.unwrap();
        assert_eq!(found_config.name, sample_config.name);
        Ok(())
    }

    #[tokio::test]
    async fn test_update_system_config() -> Result<(), anyhow::Error> {
        let pool = init_db().await;
        let mut tx = pool.begin().await?;

        let mut sample_config = create_sample_config();
        sample_config.name = format!("update_test_{}", Uuid::new_v4().to_string()); 

        let mut created_config = sample_config.create(&mut tx).await?;
        let original_id_crypto = created_config.id.clone();

        created_config.openai_model = "updated_test_model".to_string();
        let updated_config = created_config.update(&mut tx).await?;

        assert_eq!(updated_config.id, original_id_crypto); 
        assert_eq!(updated_config.openai_model, "updated_test_model");
        Ok(())
    }

    #[tokio::test]
    async fn test_delete_system_config() -> Result<(), anyhow::Error> {
        let pool = init_db().await;
        let mut tx = pool.begin().await?;

        let mut sample_config = create_sample_config();
        sample_config.name = format!("delete_test_{}", Uuid::new_v4().to_string());

        let created_config = sample_config.create(&mut tx).await?;
        let id_to_delete_crypto = created_config.id.clone();

        let rows_affected = created_config.delete(&mut tx).await?;
        assert_eq!(rows_affected, 1);

        let found_config_opt = SystemConfig::find_by_id(id_to_delete_crypto.hash().to_vec(), &mut tx).await?;
        assert!(found_config_opt.is_none(), "Config should be deleted");
        Ok(())
    }
    
    #[tokio::test]
    async fn test_find_all_system_configs() -> Result<(), anyhow::Error> {
        let pool = init_db().await;
        let mut tx = pool.begin().await?;
        
        let mut config1 = create_sample_config();
        config1.name = format!("findall_test1_{}", Uuid::new_v4().to_string());
        let config1_name = config1.name.clone();
        config1.system_prompt_version = 10i64;
        config1.create(&mut tx).await?;

        let mut config2 = create_sample_config();
        config2.name = format!("findall_test2_{}", Uuid::new_v4().to_string());
        let config2_name = config2.name.clone();
        config2.system_prompt_version = 20i64;
        config2.create(&mut tx).await?;

        let all_configs = SystemConfig::find_all(&mut tx).await?;
        assert_eq!(all_configs.len(), 2, "Should find exactly 2 configs created in this test.");
        
        assert!(all_configs.iter().any(|c| c.name == config1_name && c.system_prompt_version == 10i64));
        assert!(all_configs.iter().any(|c| c.name == config2_name && c.system_prompt_version == 20i64));

        Ok(())
    }
}
