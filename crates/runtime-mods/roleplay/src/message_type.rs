use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub enum RoleplayMessageType {
    Action(String),
    Scenario(String),
    InnerThoughts(String),
    Chat(String),
    Text(String),
}

impl Default for RoleplayMessageType {
    fn default() -> Self {
        RoleplayMessageType::Text("".to_string())
    }
}

impl RoleplayMessageType {
    pub fn to_text(&self) -> String {
        match self {
            RoleplayMessageType::Action(s) => format!("动作：{}", s),
            RoleplayMessageType::Scenario(s) => format!("场景：{}", s),
            RoleplayMessageType::InnerThoughts(s) => format!("内心独白：{}", s),
            RoleplayMessageType::Chat(s) => format!("对话：{}", s),
            RoleplayMessageType::Text(s) => s.clone(),
        }
    }

    pub fn batch_to_text(msg: &[Self]) -> String {
        let mut text = String::new();
        for m in msg {
            text.push_str(&m.to_text());
            text.push('\n');
        }
        text
    }

    pub fn from_text_batch(text: &str) -> Result<Vec<Self>> {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.parse())
            .collect()
    }
}

impl fmt::Display for RoleplayMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_text())
    }
}

impl FromStr for RoleplayMessageType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(inner) = s.strip_prefix("动作：") {
            Ok(RoleplayMessageType::Action(inner.trim().to_string()))
        } else if let Some(inner) = s.strip_prefix("场景：") {
            Ok(RoleplayMessageType::Scenario(inner.trim().to_string()))
        } else if let Some(inner) = s.strip_prefix("对话：") {
            Ok(RoleplayMessageType::Chat(inner.trim().to_string()))
        } else if let Some(inner) = s.strip_prefix("内心独白：") {
            Ok(RoleplayMessageType::InnerThoughts(inner.trim().to_string()))
        }else {
            Ok(RoleplayMessageType::Text(s.to_string()))
        }   
    }
}
