use anyhow::Result;
use async_openai::types::{ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs};

use crate::{character::Character, system_config::SystemConfig, user::User, HistoryMessage, HistoryMessagePair};

pub fn replace_placeholders(
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
    let character_personality = replace_placeholders(character_personality, character_name, user_name);
    let character_example_dialogue = replace_placeholders(character_example_dialogue, character_name, user_name);
    let character_scenario = replace_placeholders(character_scenario, character_name, user_name);

    let system_prompt = system_prompt
        .replace("{{char}}", character_name)
        .replace("{{user}}", user_name)
        .replace("{{char_personality}}", &character_personality)
        .replace("{{char_example_dialogue}}", &character_example_dialogue)
        .replace("{{char_scenario}}", &character_scenario);

    system_prompt
}

fn prepare_system_prompt(system_config: &SystemConfig, character: &Character, user: &User) -> Result<ChatCompletionRequestMessage> {
    let system_prompt = replace_placeholders_system_prompt(
        &character.name, 
        &user.profile.first_name,
        &system_config.system_prompt, 
        &character.prompts.personality_prompt,
        &character.prompts.example_dialogue,
        &character.prompts.scenario_prompt
    );

    Ok(ChatCompletionRequestMessage::System(
        ChatCompletionRequestSystemMessageArgs::default()
            .content(system_prompt)
            .build()?
    ))
}

pub fn prepare_first_message(character: &Character, user: &User) -> Result<ChatCompletionRequestMessage> {
    Ok(ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessageArgs::default()
            .content(replace_placeholders(
                &character.prompts.first_message, 
                &character.name, 
                &user.profile.first_name
            ))
            .build()?
    ))
}

pub fn prepare_chat_messages(
    system_config: &SystemConfig,
    character: &Character, user: &User,
    
    history: &[HistoryMessagePair], new_message: &HistoryMessage,
    is_new_conversation: bool
) -> Result<Vec<ChatCompletionRequestMessage>> {
    // 1. inject the roleplay system prompt
    let mut messages = vec![
        prepare_system_prompt(system_config, character, user)?,
    ];

    if is_new_conversation {
        messages.push(prepare_first_message(character, user)?);
    }

    // 2. add the history
    history
        .iter()
        .for_each(|(user_message, assistant_message)| {
            messages.push(
                ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(user_message.content.as_str())
                        .build()
                        .expect("Message should build")
                )
            );

            messages.push(
                ChatCompletionRequestMessage::Assistant(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(assistant_message.content.as_str())
                        .build()
                        .expect("Message should build")
                )
            );
        });

    messages.push(ChatCompletionRequestMessage::User(
        ChatCompletionRequestUserMessageArgs::default()
            .content(new_message.content.as_str())
            .build()
            .expect("Message should build")
    ));

    Ok(messages)
}
