use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, CryptoHash};
use voda_database::MongoDbObject;
use voda_db_macros::SqlxObject;
use voda_database::sqlx_postgres::SqlxPopulateId;
use voda_runtime::User;

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "characters"]
pub struct Character {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub name: String,
    pub description: String,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub metadata_creator: CryptoHash,
    pub metadata_version: i64,
    pub metadata_enable_voice: bool,
    pub metadata_enable_roleplay: bool,

    pub prompts_scenario_prompt: String,
    pub prompts_personality_prompt: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,

    pub tags: Vec<String>,

    pub background_image_url: Option<String>,
    pub avatar_image_url: Option<String>,
    pub voice_model_id: Option<String>,

    pub created_at: i64,
    pub updated_at: i64,
    pub published_at: i64,
}

impl MongoDbObject for Character {
    const COLLECTION_NAME: &'static str = "characters";
    type Error = anyhow::Error;

    fn populate_id(&mut self) { 
        self.id = blake3_hash(self.name.as_bytes());
    }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}

impl SqlxPopulateId for Character {
    fn sql_populate_id(&mut self) {
        self.id = ::voda_common::blake3_hash(self.name.as_bytes());
    }
}

impl Character {
    pub fn clean(&mut self) -> Result<()> {
        self.populate_id();

        // stipe all strings
        self.name = self.name.trim().to_string();
        self.description = self.description.trim().to_string();

        // strip everything in prompts
        self.prompts_scenario_prompt = self.prompts_scenario_prompt.trim().to_string();
        self.prompts_personality_prompt = self.prompts_personality_prompt.trim().to_string();
        self.prompts_example_dialogue = self.prompts_example_dialogue.trim().to_string();
        self.prompts_first_message = self.prompts_first_message.trim().to_string();

        // remove all empty strings from tags and lowercase all tags
        let mut processed_tags: Vec<String> = self.tags.iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();

        let gender_tags_const = ["male", "female", "multiple", "others"];
        let language_tags_const = ["en", "zh", "jp", "kr"];

        let mut gender_val: Option<String> = None;
        let mut language_val: Option<String> = None;

        // Extract first found gender and language tags, and filter them out from processed_tags
        processed_tags.retain(|tag| {
            if gender_tags_const.contains(&tag.as_str()) {
                if gender_val.is_none() {
                    gender_val = Some(tag.clone());
                    return false; // Remove to re-insert later
                }
            }
            if language_tags_const.contains(&tag.as_str()) {
                if language_val.is_none() {
                    language_val = Some(tag.clone());
                    return false; // Remove to re-insert later
                }
            }
            true // Keep other tags
        });

        if gender_val.is_none() {
            bail!("Character must have a gender tag (male/female/multiple/others)");
        }
        if language_val.is_none() {
            bail!("Character must have a language tag (en/zh/jp/kr)");
        }

        let mut final_tags = Vec::new();
        if let Some(gen) = gender_val {
            final_tags.push(gen);
        }
        if let Some(lang) = language_val {
            final_tags.push(lang);
        }
        final_tags.extend(processed_tags);
        self.tags = final_tags;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*; // Imports Character, User, SqlxPopulateId, etc.
    use voda_database::sqlx_postgres::{SqlxCrud, SqlxSchema};
    use sqlx::{Executor, PgPool};

    // Helper function to get a database pool for tests
    async fn get_test_pool() -> PgPool {
        let db_url = "postgresql://postgres:IgbXkBSlvHeHMAdAbXJyTQEPNJunQwHg@crossover.proxy.rlwy.net:10849/railway";
        PgPool::connect(&db_url).await.expect("Failed to connect to Postgres for tests")
    }

    // Helper to quickly create tables
    async fn create_tables(pool: &PgPool) {
        pool.execute(User::create_table_sql().as_str()).await.expect("Failed to create users table");
        pool.execute(Character::create_table_sql().as_str()).await.expect("Failed to create characters table");
    }

    // Helper to drop tables (optional, good for cleanup)
    async fn drop_tables(pool: &PgPool) {
        pool.execute(Character::drop_table_sql().as_str()).await.ok(); // .ok() to ignore if table doesn't exist
        pool.execute(User::drop_table_sql().as_str()).await.ok();
    }

    #[test]
    fn test_character_clean_gender_and_language_tags() {
        let mut character = Character {
            name: " Test Character ".to_string(),
            description: "  A description.  ".to_string(),
            tags: vec!["random", "en", "female", "extra"].into_iter().map(String::from).collect(),
            prompts_scenario_prompt: " Scenario ".to_string(),
            // ... other fields can be default ...
            ..Default::default()
        };

        character.clean().expect("Clean should succeed");

        assert_eq!(character.name, "Test Character");
        assert_eq!(character.description, "A description.");
        assert_eq!(character.prompts_scenario_prompt, "Scenario");
        
        // Check tag order and content
        assert_eq!(character.tags.len(), 4); // female, en, random, extra (lowercase, no empty)
        assert_eq!(character.tags[0], "female");
        assert_eq!(character.tags[1], "en");
        assert!(character.tags.contains(&"random".to_string()));
        assert!(character.tags.contains(&"extra".to_string()));

        let mut char2 = Character {
            name: "Char2".to_string(),
            tags: vec!["jp", "male"].into_iter().map(String::from).collect(),
            ..Default::default()
        };
        char2.clean().unwrap();
        assert_eq!(char2.tags[0], "male");
        assert_eq!(char2.tags[1], "jp");
    }

    #[test]
    fn test_character_clean_missing_gender_tag_fails() {
        let mut character = Character {
            name: "NoGenderChar".to_string(),
            tags: vec!["en", "fantasy"].into_iter().map(String::from).collect(),
            ..Default::default()
        };
        assert!(character.clean().is_err());
    }

    #[test]
    fn test_character_clean_missing_language_tag_fails() {
        let mut character = Character {
            name: "NoLangChar".to_string(),
            tags: vec!["male", "sci-fi"].into_iter().map(String::from).collect(),
            ..Default::default()
        };
        assert!(character.clean().is_err());
    }

    #[tokio::test]
    async fn test_character_foreign_key_metadata_creator() {
        let pool = get_test_pool().await;
        drop_tables(&pool).await; // Clean slate
        create_tables(&pool).await;

        // 1. Create a User
        let mut test_user = User::default();
        test_user.sql_populate_id(); // Manually populate ID before create, or rely on create
        let created_user = test_user.create(&pool).await.expect("Failed to create user");
        let user_id = created_user.id.clone();

        // 2. Create a Character linked to the User
        let mut test_character = Character {
            id: CryptoHash::default(),
            name: "LinkedCharacter".to_string(),
            description: "A character linked to a user".to_string(),
            metadata_creator: user_id.clone(), // Link to the created user
            metadata_version: 1,
            metadata_enable_voice: true,
            metadata_enable_roleplay: true,
            prompts_scenario_prompt: "Scenario".to_string(),
            prompts_personality_prompt: "Personality".to_string(),
            prompts_example_dialogue: "Dialogue".to_string(),
            prompts_first_message: "Hello".to_string(),
            tags: vec!["male".to_string(), "en".to_string()], 
            created_at: 0, // Timestamps can be set as needed
            updated_at: 0,
            published_at: 0,
            ..Default::default()
        };
        // ID will be populated by clean() -> populate_id() for SqlxPopulateId
        test_character.clean().expect("Character clean failed");
        
        let created_character = test_character.create(&pool).await.expect("Failed to create character");

        // 3. Fetch the Character and then its metadata_creator User
        let fetched_character = Character::find_by_id(created_character.id.hash().to_vec(), &pool)
            .await
            .expect("Failed to fetch character by ID")
            .expect("Character not found by ID");

        let fetched_creator_user_opt = fetched_character.fetch_metadata_creator(&pool).await.expect("Failed to fetch metadata_creator");
        
        assert!(fetched_creator_user_opt.is_some(), "Fetched creator should be Some");
        let fetched_creator_user = fetched_creator_user_opt.unwrap();

        assert_eq!(fetched_creator_user.id, created_user.id, "Fetched user ID does not match original");
        // assert_eq!(fetched_creator_user.username, created_user.username, "Fetched username does not match");

        // Clean up (optional)
        drop_tables(&pool).await;
    }
}
