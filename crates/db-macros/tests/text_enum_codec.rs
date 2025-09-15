use metastable_database::TextEnumCodec;
use metastable_db_macros::TextEnum;

#[derive(Debug, Clone, PartialEq, TextEnum)]
#[text_enum(type_lang = "zh", schema_lang = "en")]
pub enum TestMessageType {
    #[prefix(lang = "zh", content = "动作")]
    Action(String),              // Should output: {"content": "text", "type": "动作"}

    #[prefix(lang = "zh", content = "对话")]
    Chat(String),                // Should output: {"content": "text", "type": "对话"}

    Scenario,                    // Should output: "Scenario" (pure string)

    #[catch_all(include_prefix = false)]
    Text(String),                // Should output: "any content" (pure string)
}

#[derive(Debug, Clone, PartialEq, TextEnum)]
#[text_enum(type_lang = "en", schema_lang = "en")]
pub enum SimpleEnum {
    OptionA,                     // Should output: "OptionA"
    OptionB,                     // Should output: "OptionB"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_variant_to_json() {
        let scenario = TestMessageType::Scenario;
        let result = serde_json::to_value(&scenario).unwrap();
        assert_eq!(result, serde_json::json!("Scenario"));
    }

    #[test]
    fn test_content_variant_to_json() {
        let action = TestMessageType::Action("test action".to_string());
        let result = serde_json::to_value(&action).unwrap();
        
        let expected_type = if TestMessageType::type_lang() == "zh" { "动作" } else { "Action" };
        assert_eq!(result, serde_json::json!({"content": "test action", "type": expected_type}));
    }

    #[test]
    fn test_catch_all_variant_to_json() {
        let text = TestMessageType::Text("some content".to_string());
        let result = serde_json::to_value(&text).unwrap();
        assert_eq!(result, serde_json::json!("some content"));
    }

    #[test]
    fn test_to_prompt_text() {
        let action = TestMessageType::Action("test action".to_string());
        let result = action.to_prompt_text("zh");
        assert_eq!(result, "动作: test action");

        let scenario = TestMessageType::Scenario;
        let result = scenario.to_prompt_text("zh");
        assert_eq!(result, "Scenario");

        let text = TestMessageType::Text("some content".to_string());
        let result = text.to_prompt_text("zh");
        assert_eq!(result, "some content");
    }

    #[test]
    fn test_round_trip_unit_variant() {
        let original = TestMessageType::Scenario;
        let value = serde_json::to_value(&original).unwrap();
        let parsed: TestMessageType = serde_json::from_value(value).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_round_trip_content_variant() {
        let original = TestMessageType::Action("hello world".to_string());
        let value = serde_json::to_value(&original).unwrap();
        let parsed: TestMessageType = serde_json::from_value(value).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_round_trip_catch_all_variant() {
        let original = TestMessageType::Text("catch all content".to_string());
        let value = serde_json::to_value(&original).unwrap();
        let parsed: TestMessageType = serde_json::from_value(value).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_parse_pure_string_as_catch_all() {
        let input = "some random text";
        let parsed: TestMessageType = serde_json::from_value(serde_json::json!(input)).unwrap();
        match parsed {
            TestMessageType::Text(content) => assert_eq!(content, input),
            _ => panic!("Should parse as catch-all Text variant"),
        }
    }

    #[test]
    fn test_parse_structured_json() {
        let input = serde_json::json!({"content": "test content", "type": "动作"});
        let parsed: TestMessageType = serde_json::from_value(input).unwrap();
        match parsed {
            TestMessageType::Action(content) => assert_eq!(content, "test content"),
            _ => panic!("Should parse as Action variant"),
        }
    }

    #[test]
    fn test_simple_enum_unit_variants() {
        let option_a = SimpleEnum::OptionA;
        let value = serde_json::to_value(&option_a).unwrap();
        assert_eq!(value, serde_json::json!("OptionA"));

        let parsed: SimpleEnum = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, SimpleEnum::OptionA);
    }

    #[test]
    fn test_configuration_methods() {
        assert_eq!(TestMessageType::type_lang(), "zh");
        assert_eq!(TestMessageType::schema_lang(), "en");

        assert_eq!(SimpleEnum::type_lang(), "en");
        assert_eq!(SimpleEnum::schema_lang(), "en");
    }

    #[test]
    fn test_default_implementation() {
        let default = TestMessageType::default();
        match default {
            TestMessageType::Text(content) => assert_eq!(content, ""),
            _ => panic!("Default should be catch-all Text variant"),
        }
    }

    #[test]
    fn test_display_trait() {
        let action = TestMessageType::Action("test content".to_string());
        let display_str = format!("{}", action);
        // Display should now produce a valid JSON string.
        let parsed: TestMessageType = serde_json::from_str(&display_str).unwrap();
        assert_eq!(action, parsed);

        let scenario = TestMessageType::Scenario;
        let display_str = format!("{}", scenario);
        assert_eq!(display_str, r#""Scenario""#);
        let parsed: TestMessageType = serde_json::from_str(&display_str).unwrap();
        assert_eq!(scenario, parsed);
    }

    #[test]
    fn test_from_str_trait() {
        // Test parsing unit variant
        let result: TestMessageType = r#""Scenario""#.parse().unwrap();
        assert!(matches!(result, TestMessageType::Scenario));

        // Test parsing structured JSON
        let json_str = r#"{"content": "test content", "type": "动作"}"#;
        let result: TestMessageType = json_str.parse().unwrap();
        match result {
            TestMessageType::Action(content) => assert_eq!(content, "test content"),
            _ => panic!("Expected Action variant"),
        }

        // Test parsing catch-all. Note: a raw string for a catch-all needs to be valid JSON string syntax.
        let result: TestMessageType = r#""some random text""#.parse().unwrap();
        match result {
            TestMessageType::Text(content) => assert_eq!(content, "some random text"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_default_trait() {
        // TestMessageType has a catch-all Text variant, so Default should be implemented
        let default_value = TestMessageType::default();
        match default_value {
            TestMessageType::Text(content) => assert_eq!(content, ""),
            _ => panic!("Expected Text variant with empty string"),
        }
    }

    #[test]
    fn test_comprehensive_trait_integration() {
        // Test the full round-trip with Display and FromStr traits
        let original = TestMessageType::Action("hello world".to_string());

        // Display trait
        let display_str = format!("{}", original);

        // FromStr trait
        let parsed: TestMessageType = display_str.parse().unwrap();

        // Should be equal
        assert_eq!(original, parsed);

        // Test with catch-all
        let original_text = TestMessageType::Text("catch-all content".to_string());
        let display_str = format!("{}", original_text);
        let parsed: TestMessageType = display_str.parse().unwrap();
        assert_eq!(original_text, parsed);
    }
}
