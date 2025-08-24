use anyhow::anyhow;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use metastable_common::{blake3_hash, get_current_timestamp};

use crate::response::AppError;

pub fn extract_auth_token(req: &Request) -> Result<String, AppError> {
    let maybe_auth_header = req.headers().get(header::AUTHORIZATION);
    let maybe_cookie = req.headers().get(header::COOKIE);

    let maybe_auth_token_from_authorization = maybe_auth_header
        .and_then(|value| {
            value
                .to_str()
                .ok()
                .map(|s| s.split_whitespace().collect::<Vec<_>>())
                .filter(|v| v.len() == 2 && v[0] == "Bearer")
                .map(|v| v[1].to_string())
        });

    let maybe_auth_token_from_cookie = maybe_cookie.and_then(|value| {
        value
            .to_str()
            .ok()
            .map(|s| {
                s.split(';')
                    .find(|c| c.trim().starts_with("metastable.auth_token="))
                    .map(|c| c.trim().split('=').nth(1))
                    .and_then(|s| s.map(|s| s.to_string()))
            })
            .flatten()
    });

    maybe_auth_token_from_authorization
        .or(maybe_auth_token_from_cookie)
        .ok_or(AppError::new(
            StatusCode::UNAUTHORIZED,
            anyhow!("[extract_bearer_token] missing authorization header or cookie"),
        ))
}

pub fn setup_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug,sqlx=warn,hyper_util=warn"));
        // .unwrap_or_else(|_| EnvFilter::new("debug,hyper_util=warn"));

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

pub fn generate_otp(user_id: &str, counter: u64, secret: &str) -> String {
    let data = format!("{}{}{}", user_id, secret, counter);
    tracing::info!("[generate_otp] data: {}", data);
    let hash = blake3_hash(data.as_bytes());

    // We use the first 4 bytes of the hash to generate a number.
    // Using little-endian to match the TypeScript implementation
    let number_value = u32::from_le_bytes([
        hash.hash()[0],
        hash.hash()[1],
        hash.hash()[2],
        hash.hash()[3],
    ]);

    // Modulo by 1,000,000 to get a 6-digit number.
    let otp = number_value % 1_000_000;

    // Pad with leading zeros if necessary to ensure it's always 6 digits.
    format!("{:06}", otp)
}

pub fn generate_timebased_counter() -> u64 {
    const TIME_STEP_SECONDS: u64 = 30;
    let now = get_current_timestamp() as u64;
    now / TIME_STEP_SECONDS
}

pub fn verify_otp(user_id: &str, otp: &str, secret: &str) -> bool {
    const WINDOW: i64 = 10; // 10 steps * 30s = 300s (5 minutes)
    let current_counter = generate_timebased_counter() as i64;

    // Check the OTP for the current time step and a window around it to allow for network delays.
    for i in -WINDOW..=WINDOW {
        let counter = current_counter + i;
        if counter < 0 {
            continue; // Skip negative counters
        }
        
        let generated_otp = generate_otp(user_id, counter as u64, secret);
        if generated_otp == otp {
            tracing::info!("OTP is valid for counter {}.", counter);
            return true;
        }
    }

    false
}
