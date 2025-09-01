mod crypto;
mod crypto_hash;
mod env;
mod client;

use chrono::Utc;
use chrono_tz::Asia::Shanghai;

pub use crypto::{encrypt, decrypt, blake3_hash};
pub use crypto_hash::CryptoHash;
pub use env::EnvVars;
pub use client::ModuleClient;

pub fn get_current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub fn get_today_start_timestamp_utc8() -> i64 {
    // get current timestamp
    let current_timestamp = get_current_timestamp();
    // UTC+8 offset (8 hours = 8 * 60 * 60 seconds)
    let utc8_offset = 8 * 60 * 60;
    // calculate today's start time in UTC+8 timezone
    let utc8_today_start = current_timestamp - (current_timestamp % (24 * 60 * 60)) + utc8_offset;
    // convert back to UTC timestamp
    utc8_today_start - utc8_offset
}

pub fn get_time_in_utc8() -> String {
    Utc::now().with_timezone(&Shanghai).to_rfc3339()
}