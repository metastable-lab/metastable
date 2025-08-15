mod consts;

#[cfg(feature = "embeder")]
mod embeder;
#[cfg(feature = "llm")]
mod llm;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "graph")]
mod neo4j;

#[cfg(feature = "embeder")]
pub use embeder::EmbederClient;
#[cfg(feature = "llm")]
pub use llm::LlmClient;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresClient, PgvectorClient};
#[cfg(feature = "graph")]
pub use neo4j::{GraphClient, EntityTag, Relationship, GraphEntities, Mem0Filter};

pub use consts::*;