use std::env;
use anyhow::{Result, anyhow};
use metastable_common::{define_module_client, ModuleClient};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use aws_sdk_s3::{Client as S3Client, config::{Builder as S3ConfigBuilder, Credentials, Region}};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ImageFolder {
    Generated,
    Uploads,
    Avatars,
    Thumbnails,
    Temp,
}

impl ImageFolder {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageFolder::Generated => "generated",
            ImageFolder::Uploads => "uploads",
            ImageFolder::Avatars => "avatars",
            ImageFolder::Thumbnails => "thumbnails",
            ImageFolder::Temp => "temp",
        }
    }

    pub fn path(&self) -> String {
        format!("images/{}", self.as_str())
    }
}

impl Default for ImageFolder {
    fn default() -> Self {
        ImageFolder::Generated
    }
}

#[derive(Debug, Clone)]
pub struct ImageUpload {
    pub folder: ImageFolder,
    pub file_extension: String,
    pub data: Vec<u8>,
}

impl ImageUpload {
    pub fn new(folder: ImageFolder, file_extension: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            folder,
            file_extension: file_extension.into(),
            data,
        }
    }

    pub fn from_base64(folder: ImageFolder, base64_data: &str) -> Result<Self> {
        let (file_extension, data) = parse_base64_image(base64_data)?;
        Ok(Self::new(folder, file_extension, data))
    }

    pub fn key(&self) -> String {
        let file_id = Uuid::new_v4();
        format!("{}/{}.{}", self.folder.path(), file_id, self.file_extension)
    }

    pub fn content_type(&self) -> String {
        format!("image/{}", self.file_extension)
    }
}

fn parse_base64_image(base64_data: &str) -> Result<(String, Vec<u8>)> {
    const DATA_URL_PREFIX: &str = "data:image/";

    if !base64_data.starts_with(DATA_URL_PREFIX) {
        return Err(anyhow!("Invalid data URL format"));
    }

    let content = &base64_data[DATA_URL_PREFIX.len()..];
    let parts: Vec<&str> = content.splitn(2, ";base64,").collect();

    if parts.len() != 2 {
        return Err(anyhow!("Invalid base64 data URL format"));
    }

    let file_extension = parts[0].to_string();
    let base64_content = parts[1];

    use base64::Engine;
    let image_data = base64::engine::general_purpose::STANDARD
        .decode(base64_content)
        .map_err(|e| anyhow!("Failed to decode base64 image: {}", e))?;

    Ok((file_extension, image_data))
}

define_module_client! {
    (struct R2Client, "r2")
    client_type: S3Client,
    env: ["R2_ACCOUNT_ID", "R2_ACCESS_KEY_ID", "R2_SECRET_ACCESS_KEY", "R2_BUCKET_NAME"],
    setup: async {
        let account_id = env::var("R2_ACCOUNT_ID").expect("R2_ACCOUNT_ID is not set");
        let access_key_id = env::var("R2_ACCESS_KEY_ID").expect("R2_ACCESS_KEY_ID is not set");
        let secret_access_key = env::var("R2_SECRET_ACCESS_KEY").expect("R2_SECRET_ACCESS_KEY is not set");

        let endpoint_url = format!("https://{}.r2.cloudflarestorage.com", account_id);

        let credentials = Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "r2-client"
        );

        let s3_config = S3ConfigBuilder::new()
            .endpoint_url(endpoint_url)
            .credentials_provider(credentials)
            .region(Region::new("auto"))
            .behavior_version_latest()
            .build();

        S3Client::from_conf(s3_config)
    }
}

impl R2Client {
    pub fn bucket_name(&self) -> String {
        env::var("R2_BUCKET_NAME").expect("R2_BUCKET_NAME is not set")
    }

    pub fn public_domain(&self) -> String {
        env::var("R2_PUBLIC_DOMAIN").unwrap_or_else(|_| format!("{}.r2.dev", self.bucket_name()))
    }

    pub fn public_url(&self, key: &str) -> String {
        format!("https://{}/{}/{}", self.public_domain(), self.bucket_name(), key)
    }

    /// Upload image with clean, simple interface
    pub async fn upload(&self, upload: ImageUpload) -> Result<String> {
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
            .map_err(|e| anyhow!("Failed to upload image to R2: {}", e))?;

        Ok(self.public_url(&key))
    }

    /// Upload from base64 data
    pub async fn upload_base64(&self, folder: ImageFolder, base64_data: &str) -> Result<String> {
        let upload = ImageUpload::from_base64(folder, base64_data)?;
        self.upload(upload).await
    }

    /// Upload raw image data
    pub async fn upload_bytes(&self, folder: ImageFolder, file_extension: &str, data: &[u8]) -> Result<String> {
        let upload = ImageUpload::new(folder, file_extension, data.to_vec());
        self.upload(upload).await
    }

    /// Generate pre-signed URL for direct uploads
    pub async fn generate_presigned_url(&self, folder: ImageFolder, file_extension: &str, expires_in_secs: u64) -> Result<(String, String)> {
        let file_id = Uuid::new_v4();
        let key = format!("{}/{}.{}", folder.path(), file_id, file_extension);

        let presigned_req = self.get_client()
            .put_object()
            .bucket(self.bucket_name())
            .key(&key)
            .content_type(format!("image/{}", file_extension))
            .presigned(
                aws_sdk_s3::presigning::PresigningConfig::expires_in(
                    std::time::Duration::from_secs(expires_in_secs)
                ).map_err(|e| anyhow!("Failed to create presigning config: {}", e))?
            )
            .await
            .map_err(|e| anyhow!("Failed to generate presigned URL: {}", e))?;

        let public_url = self.public_url(&key);
        Ok((presigned_req.uri().to_string(), public_url))
    }

    /// Upload using pre-signed URL
    pub async fn upload_via_presigned_url(&self, presigned_url: &str, data: &[u8], content_type: &str) -> Result<()> {
        let response = reqwest::Client::new()
            .put(presigned_url)
            .header("Content-Type", content_type)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| anyhow!("Failed to upload via presigned URL: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Upload failed with status: {}", response.status()));
        }

        Ok(())
    }
}