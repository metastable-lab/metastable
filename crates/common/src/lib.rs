mod crypto;
mod crypto_hash;
mod env;

use chrono::Utc;
use chrono_tz::Asia::Shanghai;

pub use crypto::{encrypt, decrypt, blake3_hash};
pub use crypto_hash::CryptoHash;
pub use env::EnvVars;

pub fn get_current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn get_time_in_utc8() -> String {
    Utc::now().with_timezone(&Shanghai).to_rfc3339()
}