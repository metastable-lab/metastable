pub type Embedding = Vec<f32>;
pub const EMBEDDING_DIMS: i32 = 1024;
pub const EMBEDDING_MODEL: &str = "Qwen/Qwen3-Embedding-0.6B";

pub const DEFAULT_GRAPH_DB_TEXT_SEARCH_THRESHOLD: f32 = 0.7;
pub const DEFAULT_GRAPH_DB_SEARCH_LIMIT: usize = 100;
pub const DEFAULT_GRAPH_DB_VECTOR_SEARCH_THRESHOLD: f32 = 0.9;
