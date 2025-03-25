use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Result;
use axum::{
    response::IntoResponse,
    body::Body,
};
use voda_common::EnvVars;

use crate::env::ApiServerEnv;
use crate::response::AppError;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ServeReferenceAudio {
    audio: Vec<u8>,
    text: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Wav,
    Pcm,
    #[default]
    Mp3,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Latency {
    #[default]
    Normal,
    Balanced,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TTSRequest {
    text: String,
    chunk_length: i32,
    format: AudioFormat,
    mp3_bitrate: i32,
    references: Vec<ServeReferenceAudio>,
    reference_id: Option<String>,
    normalize: bool,
    latency: Latency,
}

impl TTSRequest {
    pub async fn send_request(text: &str, reference_id: String) -> Result<impl IntoResponse, AppError> {
        let client = Client::new();
        let request = Self {
            text: text.to_string(),
            reference_id: Some(reference_id),
            chunk_length: 280,
            format: AudioFormat::Mp3,
            mp3_bitrate: 128,
            references: vec![],
            normalize: true,
            latency: Latency::Normal,
        };
        let serialized = serde_json::to_string(&request)?;
        let env = ApiServerEnv::load();
        println!("Sending request to Fish Audio: {}", serialized);
        let response = client
            .post("https://api.fish.audio/v1/tts")
            .header("authorization", format!("Bearer {}", env.get_env_var("FISH_AUDIO_API_KEY")))
            .header("content-type", "application/json")
            .body(serialized)
            .send()
            .await?;

        let body = Body::from_stream(response.bytes_stream());
        Ok((
            [
                (axum::http::header::CONTENT_TYPE, "audio/mp3"),
            ],
            body
        ).into_response())
    }
}
