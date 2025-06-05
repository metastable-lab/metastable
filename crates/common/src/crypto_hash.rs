use std::hash::{Hash, Hasher};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct CryptoHash {
    #[serde(with = "hex::serde")]
    hash: [u8; 32],
}

impl CryptoHash {
    pub fn new(hash: [u8; 32]) -> Self {
        Self { hash }
    }

    pub fn random() -> Self {
        let mut arr = [0u8; 32];
        rand::Rng::fill(&mut rand::thread_rng(), &mut arr[..]);
        Self::new(arr)
    }

    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    pub fn to_vec(&self) -> Vec<u8> {
        self.hash.to_vec()
    }

    pub fn to_hex_string(&self) -> String {
        hex::encode(self.hash)
    }

    pub fn from_hex_string(s: &str) -> Result<Self> {
        let decoded_hash = hex::decode(s)?;
        if decoded_hash.len() != 32 {
            return Err(anyhow!("Wrong length for CryptoHash from hex string: expected 32 bytes, got {}", decoded_hash.len()));
        }
        Ok(
            Self::new(decoded_hash
                .try_into()
                .expect("Slice with checked length 32 should convert to [u8; 32]")
            )
        )
    }
}

impl Hash for CryptoHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.hash());
    }
}
