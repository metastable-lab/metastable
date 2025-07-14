mod pgvector;
mod graph;

mod llm;

mod engine;
mod env;
mod raw_message;
mod message;

mod memory;

pub use engine::Mem0Engine;
pub use raw_message::{GraphEntities, EntityTag, Mem0Filter};
pub use message::Mem0Messages;
pub use pgvector::EmbeddingMessage;

pub type Embedding = Vec<f32>;
pub const EMBEDDING_DIMS: i32 = 1024;
pub const EMBEDDING_MODEL: &str = "Qwen/Qwen3-Embedding-0.6B";

pub const DEFAULT_VECTOR_DB_SEARCH_LIMIT: usize = 100;
pub const DEFAULT_GRAPH_DB_SEARCH_LIMIT: usize = 100;

/// used for merge similar items in the graph db
pub const DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD: f32 = 0.9;
/// used for general search in the graph db 
pub const DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD: f32 = 0.7;
pub const DEFAULT_VECTOR_DB_SEARCH_TRESHOLD: f32 = 0.7;