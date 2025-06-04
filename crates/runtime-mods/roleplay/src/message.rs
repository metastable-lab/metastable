use serde::{Deserialize, Serialize};

use voda_common::{get_current_timestamp, CryptoHash};
use voda_database::MongoDbObject;
use voda_runtime::{Message, MessageRole, MessageType, SystemConfig, User};

use super::{Character};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoleplayMessage {
    #[serde(rename = "_id")]
    pub id: CryptoHash,

    pub owner: CryptoHash,
    pub character_id: CryptoHash,

    pub role: MessageRole,
    pub content_type: MessageType,
    
    pub content: String,
    pub created_at: u64,
}

impl Message for RoleplayMessage {
    fn role(&self) -> &MessageRole { &self.role }
    fn owner(&self) -> &CryptoHash { &self.owner }
    
    fn content_type(&self) -> &MessageType { &self.content_type }
    fn text_content(&self) -> Option<String> { Some(self.content.clone()) }
    fn binary_content(&self) -> Option<Vec<u8>> { None }
    fn url_content(&self) -> Option<String> { None }

    fn created_at(&self) -> u64 { self.created_at }
}

impl RoleplayMessage {
    fn replace_placeholders(
        text: &str, character_name: &str, user_name: &str,
    ) -> String {
        text.replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
    }
    
    fn replace_placeholders_system_prompt(
        character_name: &str, user_name: &str,
        system_prompt: &str,
        character_personality: &str, character_example_dialogue: &str, character_scenario: &str
    ) -> String {
        let character_personality = Self::replace_placeholders(character_personality, character_name, user_name);
        let character_example_dialogue = Self::replace_placeholders(character_example_dialogue, character_name, user_name);
        let character_scenario = Self::replace_placeholders(character_scenario, character_name, user_name);
    
        let system_prompt = system_prompt
            .replace("{{char}}", character_name)
            .replace("{{user}}", user_name)
            .replace("{{char_personality}}", &character_personality)
            .replace("{{char_example_dialogue}}", &character_example_dialogue)
            .replace("{{char_scenario}}", &character_scenario);
    
        system_prompt
    }
    

    pub fn system(
        system_config: &SystemConfig, character: &Character, user: &User
    ) -> Self {
        let system_prompt = Self::replace_placeholders_system_prompt(
            &character.name, 
            &user.profile.first_name,
            &system_config.system_prompt, 
            &character.prompts_personality_prompt,
            &character.prompts_example_dialogue,
            &character.prompts_scenario_prompt
        );

        Self {
            id: CryptoHash::default(),
            owner: user.id.clone(),
            character_id: character.id.clone(),
            role: MessageRole::System,
            content_type: MessageType::Text,
            content: system_prompt,
            created_at: get_current_timestamp(),
        }
    }

    pub fn first_message(character: &Character, user: &User) -> Self {
        let first_message = Self::replace_placeholders(
            &character.prompts_first_message, 
            &character.name, 
            &user.profile.first_name
        );

        Self {
            id: CryptoHash::default(),
            owner: user.id.clone(),
            character_id: character.id.clone(),
            role: MessageRole::Assistant,
            content_type: MessageType::Text,
            content: first_message,
            created_at: get_current_timestamp(),
        }
    }
}

impl MongoDbObject for RoleplayMessage {
    const COLLECTION_NAME: &'static str = "roleplay_message";
    type Error = anyhow::Error;

    fn populate_id(&mut self) { self.id = CryptoHash::random() }
    fn get_id(&self) -> CryptoHash { self.id.clone() }
}

// pub fn prepare_chat_messages(
//     system_config: &SystemConfig,
//     character: &Character, user: &User,
    
//     history: &[HistoryMessagePair], new_message: &HistoryMessage,
//     is_new_conversation: bool
// ) -> Result<Vec<ChatCompletionRequestMessage>> {
//     // 1. inject the roleplay system prompt
//     let mut messages = vec![
//         prepare_system_prompt(system_config, character, user)?,
//     ];

//     if is_new_conversation {
//         messages.push(prepare_first_message(character, user)?);
//     }

//     // 2. add the history
//     history
//         .iter()
//         .for_each(|(user_message, assistant_message)| {
//             messages.push(
//                 ChatCompletionRequestMessage::User(
//                     ChatCompletionRequestUserMessageArgs::default()
//                         .content(user_message.content.as_str())
//                         .build()
//                         .expect("Message should build")
//                 )
//             );

//             messages.push(
//                 ChatCompletionRequestMessage::Assistant(
//                     ChatCompletionRequestAssistantMessageArgs::default()
//                         .content(assistant_message.content.as_str())
//                         .build()
//                         .expect("Message should build")
//                 )
//             );


//             for (_, response) in assistant_message
//                 .function_call_request
//                 .iter()
//                 .zip(assistant_message.function_call_response.iter()) 
//             {
//                 messages.push(
//                     ChatCompletionRequestMessage::Tool(
//                         ChatCompletionRequestToolMessageArgs::default()
//                             .content(
//                                 ChatCompletionRequestToolMessageContent::Text(response.to_string())
//                             )
//                             .build()
//                             .expect("Message should build")
//                     )
//                 );
//             }
//         });

//     messages.push(ChatCompletionRequestMessage::User(
//         ChatCompletionRequestUserMessageArgs::default()
//             .content(new_message.content.as_str())
//             .build()
//             .expect("Message should build")
//     ));

//     Ok(messages)
// }
