mod crypto;
mod crypto_hash;

pub use crypto::{encrypt, decrypt, blake3_hash};
pub use crypto_hash::CryptoHash;

pub fn get_current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
