use axum::{routing::get, Router};

use crate::GlobalState;

pub fn misc_routes() -> Router<GlobalState> {
    Router::new()
        .route("/health", 
            get(|| async { "OK" })
        )
}