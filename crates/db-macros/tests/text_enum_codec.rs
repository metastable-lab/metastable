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
    fn test_unit_variant_to_text() {
        let scenario = TestMessageType::Scenario;
        let result = scenario.to_text("zh");
        assert_eq!(result, "Scenario");
    }

    #[test]
    fn test_content_variant_to_text() {
        let action = TestMessageType::Action("test action".to_string());
        let result = action.to_text("zh");
        // Should be JSON with Chinese type name
        assert!(result.contains("\"content\":\"test action\""));
        assert!(result.contains("\"type\":\"动作\""));
    }

    #[test]
    fn test_catch_all_variant_to_text() {
        let text = TestMessageType::Text("some content".to_string());
        let result = text.to_text("zh");
        assert_eq!(result, "some content");
    }

    #[test]
    fn test_round_trip_unit_variant() {
        let original = TestMessageType::Scenario;
        let text = original.to_text("zh");
        let parsed = TestMessageType::from_text(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_round_trip_content_variant() {
        let original = TestMessageType::Action("hello world".to_string());
        let text = original.to_text("zh");
        let parsed = TestMessageType::from_text(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_round_trip_catch_all_variant() {
        let original = TestMessageType::Text("catch all content".to_string());
        let text = original.to_text("zh");
        let parsed = TestMessageType::from_text(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_parse_pure_string_as_catch_all() {
        let input = "some random text";
        let parsed = TestMessageType::from_text(input).unwrap();
        match parsed {
            TestMessageType::Text(content) => assert_eq!(content, input),
            _ => panic!("Should parse as catch-all Text variant"),
        }
    }

    #[test]
    fn test_parse_structured_json() {
        let input = r#"{"content": "test content", "type": "动作"}"#;
        let parsed = TestMessageType::from_text(input).unwrap();
        match parsed {
            TestMessageType::Action(content) => assert_eq!(content, "test content"),
            _ => panic!("Should parse as Action variant"),
        }
    }

    #[test]
    fn test_simple_enum_unit_variants() {
        let option_a = SimpleEnum::OptionA;
        let text = option_a.to_text("en");
        assert_eq!(text, "OptionA");

        let parsed = SimpleEnum::from_text(&text).unwrap();
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
        // Should be the same as to_text with default language
        let text_str = action.to_text("zh");
        assert_eq!(display_str, text_str);

        let scenario = TestMessageType::Scenario;
        let display_str = format!("{}", scenario);
        assert_eq!(display_str, "Scenario");
    }

    #[test]
    fn test_from_str_trait() {
        // Test parsing unit variant
        let result: TestMessageType = "Scenario".parse().unwrap();
        assert!(matches!(result, TestMessageType::Scenario));

        // Test parsing structured JSON
        let json_str = r#"{"content": "test content", "type": "动作"}"#;
        let result: TestMessageType = json_str.parse().unwrap();
        match result {
            TestMessageType::Action(content) => assert_eq!(content, "test content"),
            _ => panic!("Expected Action variant"),
        }

        // Test parsing catch-all
        let result: TestMessageType = "some random text".parse().unwrap();
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
