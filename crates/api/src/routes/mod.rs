mod misc;
mod runtime;
mod tts;
mod graphql;
mod user;

pub use misc::misc_routes;
pub use runtime::runtime_routes;
pub use tts::voice_routes;
pub use graphql::graphql_route;
pub use user::user_routes;