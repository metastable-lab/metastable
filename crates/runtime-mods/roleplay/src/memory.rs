use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use anyhow::Result;
use voda_database::{Database, MongoDbObject};
use voda_common::{get_current_timestamp, CryptoHash};
use voda_runtime::{Memory, MessageRole, MessageType};

use super::message::RoleplayMessage;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoleplaySession {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub public: bool,

    pub owner_id: CryptoHash,
    pub character_id: CryptoHash,

    pub history: Vec<CryptoHash>,

    pub updated_at: u64,
    pub created_at: u64,
}

impl MongoDbObject for RoleplaySession {
    const COLLECTION_NAME: &'static str = "roleplay_sessions";
    type Error = anyhow::Error;

    fn populate_id(&mut self) {  }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}

#[derive(Clone)]
pub struct RoleplayMemory {
    db: Database,
}

#[async_trait::async_trait]
impl Memory for RoleplayMemory {
    type MessageType = RoleplayMessage;

    async fn initialize(&self) -> Result<()> { Ok(()) }

    async fn add_message(&self, message: &RoleplayMessage) -> Result<()> {
        Ok(())
    }

    async fn get_one(&self, memory_id: &CryptoHash) -> Result<Option<RoleplayMessage>> {
        Ok(None)
    }

    async fn get_all(&self, user_id: &CryptoHash, limit: u64, offset: u64) -> Result<Vec<RoleplayMessage>> {
        Ok(vec![])
    }

    async fn search(&self, message: &RoleplayMessage, limit: u64, offset: u64) -> Result<Vec<RoleplayMessage>> {
        Ok(vec![])
    }

    async fn update(&self, _memory_id: &CryptoHash, _message: &Self::MessageType) -> Result<()> {
        Ok(())
    }

    async fn delete(&self, _memory_id: &CryptoHash) -> Result<()> {
        Ok(())
    }

    async fn reset(&self, _user_id: &CryptoHash) -> Result<()> {
        Ok(())
    }
}


// impl RoleplayRawMemory {
//     pub fn new(is_public: bool, owner_id: CryptoHash, character_id: CryptoHash) -> Self {
//         Self { 
//             id: CryptoHash::random(), 
//             public: is_public,
//             is_concluded: false,
//             owner_id,
//             character_id,

//             history: vec![],
//             updated_at: get_current_timestamp(), 
//             created_at: get_current_timestamp()
//         }
//     }

//     pub async fn find_public_conversations_by_character(
//         db: &Database, 
//         character_id: &CryptoHash, 
//         limit: u64, offset: u64
//     ) -> Result<Vec<Self>> {
//         let col = db.collection::<Document>(Self::COLLECTION_NAME);
//         let options = FindOptions::builder()
//             .sort(doc! { "updated_at": -1 })  // Sort by nonce in descending order
//             .limit(limit as i64)
//             .skip(offset)
//             .build();

//         let filter = doc! { "public": true, "character_id": character_id.to_string() };
//         let mut docs = col.find(filter, Some(options)).await?;
        
//         let mut conversations = vec![]; 
//         while let Some(doc) = docs.next().await {
//             let convo = bson::from_document::<Self>(doc?)
//                 .map_err(anyhow::Error::from)?;
//             conversations.push(convo);
//         }
//         Ok(conversations)
//     }

//     pub async fn find_latest_conversations(db: &Database, user_id: &CryptoHash, character_id: &CryptoHash, limit: u64) -> Result<Vec<Self>> {
//         let col = db.collection::<Document>(Self::COLLECTION_NAME);
//         let options = FindOptions::builder()
//             .sort(doc! { "updated_at": -1 })  // Sort by nonce in descending order
//             .limit(limit as i64)
//             .build();

//         let filter = doc! { "owner_id": user_id.to_string(), "character_id": character_id.to_string() };
//         let mut docs = col.find(filter, Some(options)).await?;

//         let mut conversations = vec![];
//         while let Some(doc) = docs.next().await {
//             let convo = bson::from_document::<Self>(doc?)
//                 .map_err(anyhow::Error::from)?;
//             conversations.push(convo);
//         }
//         Ok(conversations)
//     }

//     // get character list of user, with the number of conversations
//     pub async fn find_character_list_of_user(db: &Database, user_id: &CryptoHash) -> Result<HashMap<CryptoHash, usize>> {
//         let col = db.collection::<Document>(Self::COLLECTION_NAME);
//         let filter = doc! { "owner_id": user_id.to_string() };
//         let mut docs = col.find(filter, None).await?;

//         let mut conversations = HashMap::new();
//         while let Some(doc) = docs.next().await {
//             let convo = bson::from_document::<Self>(doc?)
//                 .map_err(anyhow::Error::from)?;
//             conversations.entry(convo.character_id).and_modify(|count| *count += 1).or_insert(1);
//         }
//         Ok(conversations)
//     }
// }