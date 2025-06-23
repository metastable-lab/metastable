mod crypto;
mod crypto_hash;
mod env;

pub use crypto::{encrypt, decrypt, blake3_hash};
pub use crypto_hash::CryptoHash;
pub use env::EnvVars;

pub fn get_current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
