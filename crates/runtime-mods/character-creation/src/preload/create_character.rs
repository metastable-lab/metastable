use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_runtime::{ExecutableFunctionCall, LLMRunResponse};
use metastable_runtime_roleplay::{
    BackgroundStories, BehaviorTraits, Relationships, SkillsAndInterests,
    Character, CharacterFeature, CharacterGender, CharacterLanguage, CharacterOrientation, CharacterStatus,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextOrList {
    Text(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedItem {
    #[serde(rename = "type")]
    pub type_name: String,
    pub content: TextOrList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizeCharacterToolCall {
    pub name: String,
    pub description: String,
    pub gender: CharacterGender,
    pub language: CharacterLanguage,
    pub prompts_personality: String,
    pub prompts_scenario: String,
    pub prompts_example_dialogue: String,
    pub prompts_first_message: String,
    pub background_stories: Vec<TypedItem>,
    pub behavior_traits: Vec<TypedItem>,
    pub relationships: Vec<TypedItem>,
    pub skills_and_interests: Vec<TypedItem>,
    pub additional_example_dialogue: Vec<String>,
    pub additional_info: Vec<String>,
    pub tags: Vec<String>,
}

#[async_trait::async_trait]
impl ExecutableFunctionCall for SummarizeCharacterToolCall {
    type CTX = ();
    type RETURN = Character;

    fn name() -> &'static str { "summarize_character" }

    async fn execute(&self, 
        llm_response: &LLMRunResponse, 
        _execution_context: &Self::CTX
    ) -> Result<Character> {
        fn normalize_content(value: &TextOrList) -> String {
            match value {
                TextOrList::Text(s) => s.trim().to_string(),
                TextOrList::List(items) => {
                    let items = items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect::<Vec<_>>();
                    format!("[{}]", items.join(", "))
                }
            }
        }

        fn map_background(item: &TypedItem) -> BackgroundStories {
            let c = normalize_content(&item.content);
            match item.type_name.as_str() {
                "职业" => BackgroundStories::Professions(c),
                "童年经历" => BackgroundStories::ChildhoodExperience(c),
                "成长环境" => BackgroundStories::GrowthEnvironment(c),
                "重大经历" => BackgroundStories::SignificantEvents(c),
                "价值观" => BackgroundStories::ValuesAndBeliefs(c),
                "过去的遗憾或创伤，无法释怀的事" => BackgroundStories::Regrets(c),
                "梦想，渴望的事情，追求的事情" => BackgroundStories::Dreams(c),
                _ => BackgroundStories::Others(c),
            }
        }

        fn map_behavior(item: &TypedItem) -> BehaviorTraits {
            let c = normalize_content(&item.content);
            match item.type_name.as_str() {
                "行为举止" => BehaviorTraits::PhysicalBehavior(c),
                "外貌特征" => BehaviorTraits::PhysicalAppearance(c),
                "穿搭风格" => BehaviorTraits::ClothingStyle(c),
                "情绪表达方式" => BehaviorTraits::EmotionalExpression(c),
                "个人沟通习惯" => BehaviorTraits::GenralCommunicationStyle(c),
                "与用户的沟通习惯" => BehaviorTraits::CommunicationStyleWithUser(c),
                "个人行为特征" => BehaviorTraits::GeneralBehaviorTraits(c),
                "与用户的沟通特征" => BehaviorTraits::BehaviorTraitsWithUser(c),
                _ => BehaviorTraits::Others(c),
            }
        }

        fn map_relationship(item: &TypedItem) -> Relationships {
            let c = normalize_content(&item.content);
            match item.type_name.as_str() {
                "亲密伴侣" => Relationships::IntimatePartner(c),
                "家庭" => Relationships::Family(c),
                "朋友" => Relationships::Friends(c),
                "敌人" => Relationships::Enemies(c),
                "社交圈" => Relationships::SocialCircle(c),
                _ => Relationships::Others(c),
            }
        }

        fn map_skills(item: &TypedItem) -> SkillsAndInterests {
            let c = normalize_content(&item.content);
            match item.type_name.as_str() {
                "职业技能" => SkillsAndInterests::ProfessionalSkills(c),
                "生活技能" => SkillsAndInterests::LifeSkills(c),
                "兴趣爱好" => SkillsAndInterests::HobbiesAndInterests(c),
                "弱点，不擅长的领域" => SkillsAndInterests::Weaknesses(c),
                "优点，擅长的事情" => SkillsAndInterests::Virtues(c),
                "内心矛盾冲突" => SkillsAndInterests::InnerConflicts(c),
                "性癖" => SkillsAndInterests::Kinks(c),
                _ => SkillsAndInterests::Others(c),
            }
        }

        let character = Character {
            id: Uuid::new_v4(),
            name: self.name.clone(),
            description: self.description.clone(),
            gender: self.gender.clone(),
            language: self.language.clone(),
            features: vec![CharacterFeature::Roleplay],
            orientation: CharacterOrientation::default(),
            prompts_scenario: self.prompts_scenario.clone(),
            prompts_personality: self.prompts_personality.clone(),
            prompts_example_dialogue: self.prompts_example_dialogue.clone(),
            prompts_first_message: self.prompts_first_message.clone(),
            prompts_background_stories: self.background_stories.iter().map(map_background).collect(),
            prompts_behavior_traits: self.behavior_traits.iter().map(map_behavior).collect(),
            prompts_additional_example_dialogue: self.additional_example_dialogue.clone(),
            prompts_relationships: self.relationships.iter().map(map_relationship).collect(),
            prompts_skills_and_interests: self.skills_and_interests.iter().map(map_skills).collect(),
            prompts_additional_info: self.additional_info.clone(),
            tags: self.tags.clone(),
            creator: llm_response.caller.clone(),
            version: 1,
            status: CharacterStatus::Draft,
            creator_notes: None,
            created_at: get_current_timestamp(),
            updated_at: get_current_timestamp(),
        };
        Ok(character)
    }
}