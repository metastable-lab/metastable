use metastable_runtime::LlmTool;
use metastable_database::{TextEnum};
use serde::{Deserialize, Serialize};

type MessageConstructor = fn(String) -> RoleplayMessageType;
const PREFIXES_AND_CONSTRUCTORS: &[(&str, MessageConstructor)] = &[
    ("动作：", RoleplayMessageType::Action as MessageConstructor),
    ("场景：", RoleplayMessageType::Scenario as MessageConstructor),
    ("内心独白：", RoleplayMessageType::InnerThoughts as MessageConstructor),
    ("对话：", RoleplayMessageType::Chat as MessageConstructor),
    ("Action:", RoleplayMessageType::Action as MessageConstructor),
    ("Scenario:", RoleplayMessageType::Scenario as MessageConstructor),
    ("InnerThoughts:", RoleplayMessageType::InnerThoughts as MessageConstructor),
    ("Chat:", RoleplayMessageType::Chat as MessageConstructor),
];

#[derive(LlmTool, Debug, Clone, Serialize, Deserialize)]
#[llm_tool(
    name = "show_story_options",
    description = "向用户呈现故事选项以继续角色扮演。"
)]
pub struct ShowStoryOptions {
    #[llm_tool(description = "向用户呈现的用于继续故事的选项列表，内容也需要是中文。")]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum RoleplayMessageType {
    #[prefix(lang = "zh", content = "动作")]
    Action(String),
    #[prefix(lang = "zh", content = "场景")]
    Scenario(String),
    #[prefix(lang = "zh", content = "内心独白")]
    InnerThoughts(String),
    #[catch_all(include_prefix = true)]
    #[prefix(lang = "zh", content = "对话")]
    Chat(String),
}

#[derive(LlmTool, Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[llm_tool(
    name = "send_message",
    description = "用于向用户发送结构化消息的唯一工具。你必须使用此工具来发送所有回应，包括对话、动作、场景描述和选项。"
)]
pub struct SendMessage {
    #[llm_tool(description = "一个包含多个消息片段的数组，按顺序组合成完整的回复。", is_enum = true)]
    pub messages: Vec<RoleplayMessageType>,
    #[llm_tool(description = "一个包含多个选项的数组，按顺序组合成完整的回复。")]
    pub options: Vec<String>,
    #[llm_tool(description = "一个简短的总结，用于描述本次对话的要点。例如：“用户告诉我他想要去水族馆，我赞同了之后和她一起去了。")]
    pub summary: String,
}

impl SendMessage {
    pub fn from_legacy_inputs(content: &str, function_call: &SendMessage) -> Self {
        let (content_without_options, mut new_options) = Self::parse_options_from_legacy(content);

        let mut messages = RoleplayMessageType::from_legacy_message(&content_without_options);

        for message in function_call.messages.iter() {
            let mut parsed_messages = Vec::new();
            match message {
                RoleplayMessageType::Chat(text) => {
                    let (text_without_options, options_from_text) =
                        Self::parse_options_from_legacy(text);
                    new_options.extend(options_from_text);
                    if !text_without_options.is_empty() {
                        parsed_messages
                            .extend(RoleplayMessageType::from_legacy_message(&text_without_options));
                    }
                }
                _ => {
                    parsed_messages.push(message.clone());
                }
            };
            messages.extend(parsed_messages);
        }

        let mut options = function_call.options.clone();
        options.extend(new_options);
        options.sort();
        options.dedup();

        Self {
            messages,
            options,
            summary: function_call.summary.clone(),
        }
    }

    fn parse_options_from_legacy(content: &str) -> (String, Vec<String>) {
        if let Some(options_part_index) = content.find("选项：") {
            let message_part = &content[..options_part_index];
            let options_part = &content[options_part_index + "选项：".len()..];

            let options: Vec<String> = options_part
                .lines()
                .map(|line| line.trim())
                .filter(|line| line.starts_with('-'))
                .map(|line| {
                    let option = line.strip_prefix('-').unwrap_or("").trim();
                    let option = option.strip_prefix('"').unwrap_or(option);
                    let option = option.strip_suffix('"').unwrap_or(option);
                    option.trim().to_string()
                })
                .filter(|s| !s.is_empty())
                .collect();

            return (message_part.trim().to_string(), options);
        }

        (content.to_string(), vec![])
    }
}

impl RoleplayMessageType {
    /// Parse legacy message formats that may come from various sources
    /// Returns a vector of messages since some inputs contain multiple messages
    pub fn from_legacy_message(content: &str) -> Vec<Self> {
        let content = content.trim();
        if content == "内容生成完毕" || content == "内容生成完毕。" || content == "**内容生成完毕。**" {
            return vec![];
        }

        // Case 1: content is a string that represents a JSON object like {"type": ..., "content": ...}
        if let Some(message) = Self::parse_structured_json(content) {
            return vec![message];
        }

        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(content) {
            // Case 2: content is a JSON string literal containing a JSON object.
            if let Some(s) = json_value.as_str() {
                if let Some(message) = Self::parse_structured_json(s) {
                    return vec![message];
                }
            }

            // Case 3: content is a send_message function call.
            if let Some(messages) = Self::extract_messages_from_json(&json_value) {
                return messages; // Return directly as it's recursively parsed
            }
        }

        // Case 4: Handle multiple lines with Text： prefixes, this should be before recursive colon style
        if content.contains("Text：") && content.contains('\n') {
            return Self::parse_multiple_text_prefixes(content);
        }

        // Case 5: Handle colon-style prefixes (动作：, 内心独白：, 对话：, 场景：)
        let mut content_mut = content;
        while let Some(stripped) = content_mut.strip_prefix("Text：") {
            content_mut = stripped.trim();
        }
        let mut messages = Self::parse_recursive_colon_style(content_mut);

        // Case 6: Default to catch-all Text variant for raw content
        if messages.is_empty() && !content.is_empty() {
            messages.push(RoleplayMessageType::Chat(content.to_string()));
        }

        Self::postprocess_for_quotes(&mut messages);
        messages
    }

    /// Extract messages from send_message JSON structure
    fn extract_messages_from_json(json: &serde_json::Value) -> Option<Vec<Self>> {
        // Check if it's a send_message function call
        if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
            if name == "send_message" {
                if let Some(args_str) = json.get("arguments").and_then(|v| v.as_str()) {
                    if let Ok(args_json) = serde_json::from_str::<serde_json::Value>(args_str) {
                        if let Some(messages_array) =
                            args_json.get("messages").and_then(|v| v.as_array())
                        {
                            let mut results = Vec::new();
                            for msg in messages_array {
                                if let Some(msg_str) = msg.as_str() {
                                    // Recursively parse each message in the array
                                    results.extend(Self::from_legacy_message(msg_str));
                                }
                            }
                            return Some(results);
                        }
                    }
                }
            }
        }
        None
    }

    /// Parse multiple lines with Text： prefixes
    fn parse_multiple_text_prefixes(content: &str) -> Vec<Self> {
        let mut results = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("Text：") {
                let content_part = &line["Text：".len()..];
                let parsed = Self::from_legacy_message(content_part);
                results.extend(parsed);
            } else if !line.is_empty() {
                results.extend(Self::from_legacy_message(line));
            }
        }

        results
    }

    /// Parse colon-style prefixes (动作：, 内心独白：, 对话：, 场景：)
    fn parse_recursive_colon_style(content: &str) -> Vec<Self> {
        let mut found_prefixes: Vec<(usize, &str, MessageConstructor)> = Vec::new();
        for (prefix, constructor) in PREFIXES_AND_CONSTRUCTORS {
            for (i, _) in content.match_indices(prefix) {
                found_prefixes.push((i, prefix, *constructor));
            }
        }

        if found_prefixes.is_empty() {
            return vec![];
        }

        found_prefixes.sort_by_key(|k| k.0);

        let mut messages = Vec::new();

        if found_prefixes[0].0 > 0 {
            let text = content[..found_prefixes[0].0].trim();
            if !text.is_empty() {
                messages.push(RoleplayMessageType::Chat(text.to_string()));
            }
        }

        for i in 0..found_prefixes.len() {
            let (start_index, prefix, constructor) = found_prefixes[i];
            let content_start = start_index + prefix.len();

            let content_end = if i + 1 < found_prefixes.len() {
                found_prefixes[i + 1].0
            } else {
                content.len()
            };

            let message_content = content[content_start..content_end].trim().to_string();

            if !message_content.is_empty() {
                messages.push(constructor(message_content));
            }
        }

        messages
    }

    fn strip_prefixes(s: &str) -> String {
        let mut current_s = s.trim();
        for (prefix, _) in PREFIXES_AND_CONSTRUCTORS {
            if let Some(stripped) = current_s.strip_prefix(prefix) {
                current_s = stripped.trim();
                return current_s.to_string();
            }
        }
        current_s.to_string()
    }

    /// Parse structured JSON with type/content
    fn parse_structured_json(content: &str) -> Option<Self> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
            if let (Some(type_val), Some(content_val)) = (
                json.get("type").and_then(|v| v.as_str()),
                json.get("content").and_then(|v| v.as_str()),
            ) {
                let content_val = Self::strip_prefixes(content_val);
                // Handle various type name formats
                match type_val.to_lowercase().as_str() {
                    "动作" | "action" | "动作：" | "action:" => {
                        return Some(RoleplayMessageType::Action(content_val));
                    }
                    "内心独白" | "innerthoughts" | "inner_thoughts" | "内心独白：" | "innerthoughts:" => {
                        return Some(RoleplayMessageType::InnerThoughts(content_val));
                    }
                    "对话" | "chat" | "对话：" | "chat:" => {
                        return Some(RoleplayMessageType::Chat(content_val));
                    }
                    "场景" | "scenario" | "场景：" | "scenario:" => {
                        return Some(RoleplayMessageType::Scenario(content_val));
                    }
                    _ => {
                        // Unknown type, treat as text
                        return Some(RoleplayMessageType::Chat(content_val));
                    }
                }
            }
        }
        None
    }

    fn postprocess_for_quotes(messages: &mut Vec<Self>) {
        for msg in messages.iter_mut() {
            if let RoleplayMessageType::Chat(content) = msg {
                let trimmed = content.trim();
                if (trimmed.starts_with('“') && trimmed.ends_with('”'))
                    || (trimmed.starts_with('"') && trimmed.ends_with('"'))
                {
                    *msg = RoleplayMessageType::Chat(Self::trim_quotes(trimmed).to_string());
                }
            } else if let RoleplayMessageType::Chat(content) = msg {
                *content = Self::trim_quotes(content).to_string();
            }
        }
    }

    fn trim_quotes(s: &str) -> &str {
        let s = s.trim();
        let s = s.strip_prefix('“').unwrap_or(s);
        let s = s.strip_suffix('”').unwrap_or(s);
        let s = s.strip_prefix('"').unwrap_or(s);
        let s = s.strip_suffix('"').unwrap_or(s);
        s.trim()
    }
}
