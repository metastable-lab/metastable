use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, CryptoHash};
use voda_database::MongoDbObject;
use voda_db_macros::SqlxObject;
use voda_database::sqlx_postgres::SqlxPopulateId;

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "characters"]
pub struct Character {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub name: String,
    pub description: String,

    // Fields from CharacterMetadata (prefixed)
    pub metadata_creator: CryptoHash,
    pub metadata_version: i64,
    pub metadata_enable_voice: bool,
    pub metadata_enable_roleplay: bool,

    // Fields from CharacterPrompts (prefixed)
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

        // remove all empty strings from tags
        // lowercase all tags
        self.tags = self.tags.iter()
            .map(|tag| tag.to_lowercase())
            .filter(|tag| !tag.is_empty())
            .map(|tag| tag.to_string())
            .collect();

        // make sure the tag list contains either "male" or "female" or "multiple"
        // make sure it's the first tag
        // if not, move it to the front
        let gender_tags = ["male", "female", "multiple", "others"];
        let has_gender = self.tags.iter().any(|tag| gender_tags.contains(&tag.as_str()));

        if !has_gender {
            bail!("Character must have a gender tag (male/female/multiple/others)");
        }

        // Find the gender tag and move it to front if it's not already there
        if let Some(pos) = self.tags.iter().position(|tag| gender_tags.contains(&tag.as_str())) {
            if pos != 0 {
                let gender_tag = self.tags.remove(pos);
                self.tags.insert(0, gender_tag);
            }
        }

        let language_tag = ["en", "zh", "jp", "kr"];
        let has_language = self.tags.iter().any(|tag| language_tag.contains(&tag.as_str()));
        if !has_language {
            bail!("Character must have a language tag (en/zh/jp/kr)");
        }

        // Find the language tag and move it to front if it's not already there
        if let Some(pos) = self.tags.iter().position(|tag| language_tag.contains(&tag.as_str())) {
            if pos != 0 {
                let language_tag = self.tags.remove(pos);
                self.tags.insert(0, language_tag);
            }
        }

        Ok(())
    }
}
