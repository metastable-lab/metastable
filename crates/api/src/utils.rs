use anyhow::anyhow;
use axum::extract::Request;
use axum::http::{header, StatusCode};

use crate::response::AppError;

pub fn extract_bearer_token(req: &Request) -> Result<String, AppError> {
    let auth_header = req.headers().get(header::AUTHORIZATION);

    match auth_header {
        Some(value) => {
            let value = value
                .to_str()?
                .split_whitespace()
                .collect::<Vec<_>>();

            if value.len() != 2 {
                return Err(AppError::new(
                    StatusCode::UNAUTHORIZED,
                    anyhow!("invalid authorization header"),
                ));
            }

            if value[0] != "Bearer" {
                return Err(AppError::new(
                    StatusCode::UNAUTHORIZED,
                    anyhow!("invalid authorization header"),
                ));
            }

            Ok(value[1].to_string())
        }
        _ => {
            Err(AppError::new(
                StatusCode::UNAUTHORIZED,
                anyhow!("missing authorization header"),
            ))
        }
    }
}

pub fn setup_tracing() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}
