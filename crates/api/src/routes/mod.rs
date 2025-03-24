mod characters;
mod user;
mod misc;
mod system_config;
mod runtime;
mod conversation;
mod tts;

pub use characters::character_routes;
pub use user::user_routes;
pub use misc::misc_routes;
pub use system_config::system_config_routes;
pub use runtime::runtime_routes;
pub use conversation::conversation_routes;
pub use tts::voice_routes;