use metastable_database::TextEnum;

// Core character enums moved from character.rs
#[derive(Debug, Clone, Eq, PartialEq, Default, TextEnum)]
pub enum CharacterStatus {
    #[default]
    Draft,
    Reviewing,
    Rejected(String),

    Published,
    Archived(String),
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum CharacterGender {
    Male,
    Female,
    Multiple,
    #[catch_all]
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Default, TextEnum)]
pub enum CharacterLanguage {
    #[default]
    English,
    Chinese,
    Japanese,
    Korean,
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Default, TextEnum)]
pub enum CharacterFeature {
    #[default]
    Roleplay,
    CharacterCreation,

    BackgroundImage(String),
    AvatarImage(String),

    Voice(String),

    DynamicImage(Vec<String>),
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Default, TextEnum)]
pub enum CharacterOrientation {
    #[default]
    Female,
    Male,
    Full,
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum BackgroundStories {
    #[prefix(lang = "en", content = "Professions")]
    #[prefix(lang = "zh", content = "职业")]
    Professions(String),

    #[prefix(lang = "en", content = "ChildhoodExperience")]
    #[prefix(lang = "zh", content = "童年经历")]
    ChildhoodExperience(String),

    #[prefix(lang = "en", content = "GrowthEnvironment")]
    #[prefix(lang = "zh", content = "成长环境")]
    GrowthEnvironment(String),

    #[prefix(lang = "en", content = "SignificantEvents")]
    #[prefix(lang = "zh", content = "重大经历")]
    SignificantEvents(String),

    #[prefix(lang = "en", content = "ValuesAndBeliefs")]
    #[prefix(lang = "zh", content = "价值观")]
    ValuesAndBeliefs(String),

    #[prefix(lang = "en", content = "Regrets")]
    #[prefix(lang = "zh", content = "过去的遗憾或创伤，无法释怀的事")]
    Regrets(String),

    #[prefix(lang = "en", content = "Dreams")]
    #[prefix(lang = "zh", content = "梦想，渴望的事情，追求的事情")]
    Dreams(String),

    #[catch_all(include_prefix = true)]
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum Relationships {
    #[prefix(lang = "en", content = "IntimatePartner")]
    #[prefix(lang = "zh", content = "亲密伴侣")]
    IntimatePartner(String),

    #[prefix(lang = "en", content = "Family")]
    #[prefix(lang = "zh", content = "家庭")]
    Family(String),

    #[prefix(lang = "en", content = "Friends")]
    #[prefix(lang = "zh", content = "朋友")]
    Friends(String),

    #[prefix(lang = "en", content = "Enemies")]
    #[prefix(lang = "zh", content = "敌人")]
    Enemies(String),

    #[prefix(lang = "en", content = "SocialCircle")]
    #[prefix(lang = "zh", content = "社交圈")]
    SocialCircle(String),
    #[catch_all(include_prefix = true)]
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum SkillsAndInterests {
    #[prefix(lang = "en", content = "ProfessionalSkills")]
    #[prefix(lang = "zh", content = "职业技能")]
    ProfessionalSkills(String),

    #[prefix(lang = "en", content = "LifeSkills")]
    #[prefix(lang = "zh", content = "生活技能")]
    LifeSkills(String),

    #[prefix(lang = "en", content = "HobbiesAndInterests")]
    #[prefix(lang = "zh", content = "兴趣爱好")]
    HobbiesAndInterests(String),

    #[prefix(lang = "en", content = "Weaknesses")]
    #[prefix(lang = "zh", content = "弱点，不擅长的领域")]
    Weaknesses(String),

    #[prefix(lang = "en", content = "Virtues")]
    #[prefix(lang = "zh", content = "优点，擅长的事情")]
    Virtues(String),

    #[prefix(lang = "en", content = "InnerConflicts")]
    #[prefix(lang = "zh", content = "内心矛盾冲突")]
    InnerConflicts(String),

    #[prefix(lang = "en", content = "Kinks")]
    #[prefix(lang = "zh", content = "性癖")]
    Kinks(String),

    #[catch_all(include_prefix = true)]
    Others(String),
}

#[derive(Debug, Clone, Eq, PartialEq, TextEnum)]
pub enum BehaviorTraits {
    #[prefix(lang = "en", content = "PhysicalBehavior")]
    #[prefix(lang = "zh", content = "行为举止")]
    PhysicalBehavior(String),
    #[prefix(lang = "en", content = "PhysicalAppearance")]
    #[prefix(lang = "zh", content = "外貌特征")]
    PhysicalAppearance(String),

    #[prefix(lang = "en", content = "ClothingStyle")]
    #[prefix(lang = "zh", content = "穿搭风格")]
    ClothingStyle(String),

    #[prefix(lang = "en", content = "EmotionalExpression")]
    #[prefix(lang = "zh", content = "情绪表达方式")]
    EmotionalExpression(String),

    #[prefix(lang = "en", content = "GenralCommunicationStyle")]
    #[prefix(lang = "zh", content = "个人沟通习惯")]
    GenralCommunicationStyle(String),

    #[prefix(lang = "en", content = "CommunicationStyleWithUser")]
    #[prefix(lang = "zh", content = "与用户的沟通习惯")]
    CommunicationStyleWithUser(String),

    #[prefix(lang = "en", content = "GeneralBehaviorTraits")]
    #[prefix(lang = "zh", content = "个人行为特征")]
    GeneralBehaviorTraits(String),

    #[prefix(lang = "en", content = "BehaviorTraitsWithUser")]
    #[prefix(lang = "zh", content = "与用户的沟通特征")]
    BehaviorTraitsWithUser(String),

    #[catch_all(include_prefix = true)]
    Others(String),
}
