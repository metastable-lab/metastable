use anyhow::{Result, anyhow};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use metastable_common::{define_module_client, ModuleClient};
use crate::r2::R2Client;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TTSConfig {
    pub reference_id: Option<String>,
    pub model_name: Option<String>,
    pub text: String,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub chunk_length: Option<i32>,
    pub normalize: Option<bool>,
    pub format: Option<AudioFormat>,
    pub sample_rate: Option<i32>,
    pub mp3_bitrate: Option<i32>,
    pub opus_bitrate: Option<i32>,
    pub latency: Option<Latency>,
    pub prosody: Option<ProsodyControl>,
    pub references: Option<Vec<ReferenceAudio>>,
}

impl Default for TTSConfig {
    fn default() -> Self {
        Self {
            reference_id: None,
            model_name: Some("s1".to_string()),
            text: String::new(),
            temperature: Some(0.7),
            top_p: Some(0.7),
            chunk_length: Some(200),
            normalize: Some(true),
            format: Some(AudioFormat::Mp3),
            sample_rate: None,
            mp3_bitrate: Some(128),
            opus_bitrate: Some(32),
            latency: Some(Latency::Normal),
            prosody: None,
            references: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Wav,
    Pcm,
    Mp3,
    Opus,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Latency {
    Normal,
    Balanced,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProsodyControl {
    pub speed: Option<f64>,
    pub volume: Option<f64>,
}

impl Default for ProsodyControl {
    fn default() -> Self {
        Self {
            speed: Some(1.0),
            volume: Some(0.0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReferenceAudio {
    pub audio: Vec<u8>,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TTSRequestPayload {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    references: Option<Vec<ReferenceAudio>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prosody: Option<ProsodyControl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chunk_length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<AudioFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sample_rate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mp3_bitrate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opus_bitrate: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency: Option<Latency>,
}

impl From<TTSConfig> for TTSRequestPayload {
    fn from(config: TTSConfig) -> Self {
        Self {
            text: config.text,
            temperature: config.temperature,
            top_p: config.top_p,
            references: config.references,
            reference_id: config.reference_id,
            prosody: config.prosody,
            chunk_length: config.chunk_length,
            normalize: config.normalize,
            format: config.format,
            sample_rate: config.sample_rate,
            mp3_bitrate: config.mp3_bitrate,
            opus_bitrate: config.opus_bitrate,
            latency: config.latency,
        }
    }
}

define_module_client! {
    (struct FishAudioClient, "fish_audio")
    client_type: Client,
    env: ["FISH_AUDIO_API_KEY"],
    setup: async {
        Client::new()
    }
}

impl FishAudioClient {
    const BASE_URL: &'static str = "https://api.fish.audio";

    pub async fn generate_tts(&self, config: TTSConfig) -> Result<Vec<u8>> {
        let api_key = std::env::var("FISH_AUDIO_API_KEY")
            .map_err(|_| anyhow!("FISH_AUDIO_API_KEY environment variable not set"))?;

        let payload: TTSRequestPayload = config.clone().into();
        let model = config.model_name.unwrap_or_else(|| "speech-1.5".to_string());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", api_key).parse()
                .map_err(|_| anyhow!("Invalid API key format"))?
        );
        headers.insert(
            "Content-Type",
            "application/json".parse()
                .map_err(|_| anyhow!("Invalid content type"))?
        );
        headers.insert(
            "model",
            model.parse()
                .map_err(|_| anyhow!("Invalid model name"))?
        );

        let response = self.get_client()
            .post(&format!("{}/v1/tts", Self::BASE_URL))
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to send TTS request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("TTS request failed with status {}: {}", status, text));
        }

        let audio_data = response.bytes().await
            .map_err(|e| anyhow!("Failed to read response bytes: {}", e))?;

        Ok(audio_data.to_vec())
    }

    pub async fn generate_and_upload_to_r2(&self, config: TTSConfig, r2_client: &R2Client) -> Result<String> {
        let audio_data = self.generate_tts(config.clone()).await?;

        let file_extension = match config.format.unwrap_or(AudioFormat::Mp3) {
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Wav => "wav",
            AudioFormat::Pcm => "pcm",
            AudioFormat::Opus => "opus",
        };

        let upload = AudioUpload::new(AudioFolder::Generated, file_extension, audio_data);
        r2_client.upload_audio(upload).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioFolder {
    Generated,
    Uploads,
    Temp,
}

impl AudioFolder {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioFolder::Generated => "generated",
            AudioFolder::Uploads => "uploads",
            AudioFolder::Temp => "temp",
        }
    }

    pub fn path(&self) -> String {
        format!("audio/{}", self.as_str())
    }
}

impl Default for AudioFolder {
    fn default() -> Self {
        AudioFolder::Generated
    }
}

#[derive(Debug, Clone)]
pub struct AudioUpload {
    pub folder: AudioFolder,
    pub file_extension: String,
    pub data: Vec<u8>,
}

impl AudioUpload {
    pub fn new(folder: AudioFolder, file_extension: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            folder,
            file_extension: file_extension.into(),
            data,
        }
    }

    pub fn key(&self) -> String {
        let file_id = uuid::Uuid::new_v4();
        format!("{}/{}.{}", self.folder.path(), file_id, self.file_extension)
    }

    pub fn content_type(&self) -> String {
        match self.file_extension.as_str() {
            "mp3" => "audio/mp3".to_string(),
            "wav" => "audio/wav".to_string(),
            "opus" => "audio/opus".to_string(),
            "pcm" => "audio/pcm".to_string(),
            _ => format!("audio/{}", self.file_extension),
        }
    }
}

impl R2Client {
    pub async fn upload_audio(&self, upload: AudioUpload) -> Result<String> {
        let key = upload.key();
        let content_type = upload.content_type();

        self.get_client()
            .put_object()
            .bucket(self.bucket_name())
            .key(&key)
            .body(aws_sdk_s3::primitives::ByteStream::from(upload.data))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to upload audio to R2: {}", e))?;

        Ok(self.public_url(&key))
    }
}

// 磁性男声 https://fish.audio/m/48cce9c01c9a481e90c397d802ed8375
// 磁性少年音 https://fish.audio/m/04a89dac432f4b11a21f9f26c75b1aa3
// 男少年音 https://fish.audio/m/7a02aebcd8f94d8283a02842ce4ddd33
// 低音炮 https://fish.audio/m/4dcc3f1e440d404580b2be02cc52aed9
// 太阳 https://fish.audio/m/c34a4719098341f0a64f07609cfe29cf
// 温柔音 https://fish.audio/m/86bda40527a04e4a815659c142028a42
// 古风 https://fish.audio/m/b4848a16f79a45dd87e64031826d72ef

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fish_audio_chinese_ancient_style() {
        let client = FishAudioClient::setup_connection().await;
        let r2_client = R2Client::setup_connection().await;

        let config = TTSConfig {
            reference_id: Some("b4848a16f79a45dd87e64031826d72ef".to_string()), // 古风
            model_name: Some("s1".to_string()),
            text: "月落乌啼霜满天，江枫渔火对愁眠。姑苏城外寒山寺，夜半钟声到客船。".to_string(),
            temperature: Some(0.7),
            top_p: Some(0.7),
            format: Some(AudioFormat::Mp3),
            ..Default::default()
        };

        let result = client.generate_and_upload_to_r2(config, &r2_client).await;

        match result {
            Ok(audio_data) => {
                println!("✅ TTS generation and upload to R2 successful!");
                println!("📊 Generated {} bytes of audio data", audio_data.len());
                assert!(!audio_data.is_empty(), "Audio data should not be empty");
            }
            Err(e) => {
                println!("❌ TTS generation and upload to R2 failed: {}", e);
            }
        }
    }
}