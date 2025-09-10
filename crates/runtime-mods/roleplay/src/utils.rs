use anyhow::Result;
use async_openai::types::FunctionCall;
use metastable_runtime::{Message, ToolCall};
use crate::agents::SummarizeCharacter;
use crate::agents::SendMessage;

pub fn validate_parsing(m: &SendMessage) -> Result<FunctionCall> {
    let tc = m.into_tool_call()?;
    let mm = SendMessage::try_from_tool_call(&tc)?;
    if *m != mm {
        return Err(anyhow::anyhow!("Parsing failed"));
    }
    Ok(tc)
}

pub fn try_parse_content(tool_call: &Option<FunctionCall>, content: &str) -> Result<FunctionCall> {
    let t = if let Some(tc) = &tool_call {
        let function_name = &tc.name;
        if function_name == "summarize_character" {
            let t = SummarizeCharacter::try_from_tool_call(&tc)?;
            t.into_tool_call()?
        } else if function_name == "send_message" { // Assumes send_message
            let t = SendMessage::try_from_tool_call(&tc)?;
            let parsed_tool = SendMessage::from_legacy_inputs(&content, &t);
            validate_parsing(&parsed_tool)?
        } else {
            tracing::info!("Skipping {} tool call", function_name);
            tc.clone()
        }
    }  else {
        // No tool call, but has content.
        let assistant_content = content.trim();
        let cleaned_content = assistant_content.trim_matches(|c| c == '*' || c == '.').trim();

        let parsed_tool = SendMessage::from_legacy_inputs(cleaned_content, &SendMessage::default());
        validate_parsing(&parsed_tool)?
    };

    Ok(t)
}

pub fn try_prase_message(message: &Message) -> Result<FunctionCall> {
    try_parse_content(
        &message.assistant_message_tool_call.0, 
        &message.assistant_message_content
    )
}
