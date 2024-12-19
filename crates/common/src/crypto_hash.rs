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
        Self::new(rand::random())
    }

    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    pub fn to_string(&self) -> String {
        hex::encode(self.hash())
    }

    pub fn from_string(str: &str) -> Result<Self> {
        let hash = hex::decode(str)?;
        Ok(
            Self::new(hash
                .try_into()
                .map_err(|_| anyhow!("Wrong Length for CryptoHash"))?
        ))
    }
}

impl Hash for CryptoHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.hash());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crypto_hash() {
        let hash = CryptoHash::random();
        println!("{}", hash.to_string());
    }
}