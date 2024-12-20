use serde::{Deserialize, Serialize};

use voda_common::{blake3_hash, CryptoHash};
use voda_database::MongoDbObject;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SystemConfig {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub name: String,
    
    pub system_prompt: String,
    pub system_prompt_version: u64,

    pub openai_base_url: String,
    pub openai_model: String,
    pub openai_temperature: f32,
    pub openai_max_tokens: u16,

    pub updated_at: u64,
}

impl MongoDbObject for SystemConfig {
    const COLLECTION_NAME: &'static str = "system_config";
    type Error = anyhow::Error;

    fn populate_id(&mut self) { 
        self.name = self.name.to_lowercase().trim().to_string();
        self.id = blake3_hash(self.name.as_bytes()); 
    }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}
