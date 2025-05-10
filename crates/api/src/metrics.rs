use prometheus::IntCounterVec;
use prometheus::register_int_counter_vec;
use lazy_static::lazy_static;

lazy_static! {

    // Recorded "Memory"
    pub static ref CHARACTER_UNIQUE_USER: IntCounterVec =
        register_int_counter_vec!("character_unique_user", "Number of unique user of an character", &["character"]).unwrap();

    pub static ref CHARACTER_SESSIONS: IntCounterVec =
        register_int_counter_vec!("character_sessions", "Number of sessions of an character", &["character"]).unwrap();

    // recorded  in "Runtime"
    pub static ref CHARACTER_NON_EMPTY_SESSIONS: IntCounterVec =
        register_int_counter_vec!("character_non_empty_sessions", "Number of non-empty sessions of an character", &["character"]).unwrap();
    
    pub static ref CHARACTER_MESSAGES: IntCounterVec =
        register_int_counter_vec!("character_messages", "Number of messages of an character", &["character"]).unwrap();

    pub static ref CHARACTER_REGENERATIONS: IntCounterVec =
        register_int_counter_vec!("character_regenerations", "Number of regenerations of an character", &["character"]).unwrap();

    // Token Usage
    pub static ref TOKEN_USAGE: IntCounterVec =
        register_int_counter_vec!("token_usage", "Number of tokens used", &["model_name", "character"]).unwrap();
}
