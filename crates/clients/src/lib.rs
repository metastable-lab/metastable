mod consts;

#[cfg(feature = "embeder")]
mod embeder;
#[cfg(feature = "llm")]
mod llm;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "r2")]
mod r2;
#[cfg(feature = "fish_audio")]
mod fish_audio;

#[cfg(feature = "embeder")]
pub use embeder::EmbederClient;
#[cfg(feature = "llm")]
pub use llm::LlmClient;
#[cfg(feature = "postgres")]
pub use postgres::{PostgresClient, PgvectorClient};
#[cfg(feature = "r2")]
pub use r2::{R2Client, ImageFolder, ImageUpload};
#[cfg(feature = "fish_audio")]
pub use fish_audio::{FishAudioClient, TTSConfig, AudioFormat, Latency, ProsodyControl, ReferenceAudio, AudioFolder, AudioUpload};

mod vector;
pub use vector::{EmbeddingMessage, MemoryEvent, MemoryUpdateEntry, BatchUpdateSummary, Mem0Filter};

pub use consts::*;
