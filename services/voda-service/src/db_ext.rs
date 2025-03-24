use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use voda_common::{blake3_hash, get_current_timestamp, CryptoHash};
use voda_database::MongoDbObject;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Url {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub created_at: u64,

    pub url_type: String,
    pub created_by: CryptoHash,
    pub used_by: HashSet<CryptoHash>,

    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GitcoinGrant {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub name: String,
    pub description: String,
    pub url: String,
    pub twitter: String,
    pub recipient_id: String,
}


impl MongoDbObject for Url {
    const COLLECTION_NAME: &'static str = "url";
    type Error = anyhow::Error;

    fn populate_id(&mut self) {  }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}


impl MongoDbObject for GitcoinGrant {
    const COLLECTION_NAME: &'static str = "gitcoin_grants";
    type Error = anyhow::Error;

    fn populate_id(&mut self) { self.id = blake3_hash(self.recipient_id.as_bytes()) }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}

impl Url {
    pub fn new(created_by: CryptoHash, path: String, url_type: String) -> Self {
        let mut url = Self::default();
        url.id = CryptoHash::random();
        url.created_at = get_current_timestamp();
        url.created_by = created_by;
        url.path = path;
        url.url_type = url_type;
        url
    }
}