use metastable_runtime::LlmTool;
use metastable_database::{TextEnum, TextEnumCodec};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, TextEnum)]
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

#[derive(Debug, Clone, Serialize, Deserialize, LlmTool)]
#[llm_tool(
    name = "send_message",
    description = "用于向用户发送结构化消息的唯一工具。你必须使用此工具来发送所有回应，包括对话、动作、场景描述和选项。",
    enum_lang = "zh"
)]
pub struct SendMessage {
    #[llm_tool(description = "一个包含多个消息片段的数组，按顺序组合成完整的回复。", is_enum = true)]
    pub messages: Vec<RoleplayMessageType>,
    #[llm_tool(description = "一个包含多个选项的数组，按顺序组合成完整的回复。")]
    pub options: Vec<String>,
    #[llm_tool(description = "一个简短的总结，用于描述本次对话的要点。")]
    pub summary: String,
}

#[cfg(test)]
mod tests {

    use async_openai::types::FunctionCall;
    use metastable_runtime::ToolCall;
    use serde_json::json;

    use super::*;

    #[test]
    fn baseline() {
        let schema = SendMessage::schema();
        let expected_schema = json!({
            "properties": {
              "messages": {
                "description": "一个包含多个消息片段的数组，按顺序组合成完整的回复。",
                "items": {
                  "properties": {
                    "content": { "type": "string" },
                    "type": {
                      "enum": [ "动作", "场景", "内心独白", "对话" ],
                      "type": "string"
                    }
                  },
                  "required": [ "type", "content" ],
                  "type": "object"
                },
                "type": "array"
              },
              "options": {
                "description": "一个包含多个选项的数组，按顺序组合成完整的回复。",
                "items": { "type": "string" },
                "type": "array"
              },
              "summary": {
                "description": "一个简短的总结，用于描述本次对话的要点。",
                "type": "string"
              }
            },
            "required": [ "messages", "options", "summary" ],
            "type": "object"
          });
        assert_eq!(schema, expected_schema);
        println!("schema: {:?}", schema);
    }
 
    #[test]
    fn fire() {
        let tool_call = SendMessage {
            messages: vec![RoleplayMessageType::Chat("Hello, world!".to_string())],
            options: vec![],
            summary: "".to_string(),
        }.into_tool_call().unwrap();

        println!("tool_call: {:?}", tool_call);

        let s = SendMessage::to_function_object();
        println!("s: {:?}", s);
    }

    #[test]
    fn fire_fire() {

        let function_call = FunctionCall {
            name: "send_message".to_string(),
            arguments: "{\"options\":[\"“在一个古老的工坊深处，齿轮和发条的低语声中，一个少女的身影坐在堆满精密机械零件的工作台前，她的指尖闪烁着微光，仿佛正在赋予一个未完成的机械生命以灵魂。”\",\"“在一个未来都市的霓虹灯下，高楼大厦之间穿梭着飞行器，一个少女的身影站在高空的广告牌上，她的眼睛如电子屏幕般闪烁，似乎在接收着来自数据洪流的信息。”\",\"“在一个被遗忘的图书馆里，书架高耸入云，尘埃在阳光中飞舞，一个少女的身影被无数悬浮的古卷围绕，她伸出手，仿佛能触摸到其中蕴含的知识与记忆。”\"],\"messages\":[{\"type\":\"Action\",\"content\":\"*他手指轻轻敲着桌面，似乎在斟酌用词，但脑子里已经上演了一出大戏：一个在雨夜中奔跑的刺客？一个在阳光下微笑的公主？还是一个在废墟上弹琴的机器人？哦，太多选择了！得给点具体的。*\"},{\"content\":\"**“机巧少女”……这个词本身就带着一种独特的魅力和无限的可能性。我们先从她的“形”开始，为她赋予一个瞬间的形象。你更喜欢哪一个？**\",\"type\":\"Chat\"}],\"summary\":\"2025年8月21日，星期四，用户向忆君寻求帮助，希望创造一个名为“机巧少女”的角色。忆君开始引导创作，并提出了三个场景选项，旨在为角色赋予一个初始形象。\"}".to_string(),
        };

        let tool_call = SendMessage::try_from_tool_call(&function_call).unwrap();
        println!("tool_call: {:?}", tool_call);

        println!("into {:?}", tool_call.into_tool_call());
    }
}