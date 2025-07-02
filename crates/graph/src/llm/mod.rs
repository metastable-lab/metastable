mod extract_entity;
mod extract_relationship;

use async_openai::types::FunctionObject;

pub struct LlmConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: i32,
    pub system_prompt: String,
    pub tools: Vec<FunctionObject>
}

pub use crate::llm::extract_entity::{EntitiesToolcall, SingleEntityToolcall, get_extract_entity_config};
pub use crate::llm::extract_relationship::{RelationshipsToolcall, SingleRelationshipToolcall, get_extract_relationship_config};

#[cfg(test)]
mod tests {
    use crate::{GraphDatabase};

    use super::*;

    #[tokio::test]
    async fn test_extract_entity_config() {
        let config = get_extract_entity_config("123".to_string());
        let config2 = get_extract_relationship_config("123".to_string());

        let db = GraphDatabase::new().await;

        let text = r#"Hey everyone! So, I just finished exploring the ancient ruins we stumbled upon last session, and I found this really cool artifact that looks like it might be a key to unlocking the hidden chamber we heard about in the tavern. I’m thinking it’s some kind of magical relic, but I’m not sure what it does yet.
Also, I overheard some NPCs talking about a dragon sighting near the mountains, and I think we should check it out. I mean, who wouldn’t want to face a dragon, right? But we should probably gather some more supplies first—maybe hit up the blacksmith for some better weapons and armor.
Oh, and I’ve been working on my character’s backstory a bit more. Turns out, my rogue used to be part of a thieves' guild, but they betrayed him, and now he’s on a quest for revenge. I think it could add some interesting dynamics to our party, especially if we run into any old guild members.
Let me know what you all think! Should we head to the blacksmith first or go dragon hunting? I’m down for either, but I think we should be prepared for whatever comes our way. Can’t wait to hear your thoughts"#;

        let response = db.llm(&config, text).await.unwrap();
        let response2 = db.llm(&config2, text).await.unwrap();

        println!("response: {}", response);
        println!("response2: {}", response2);
    }
}