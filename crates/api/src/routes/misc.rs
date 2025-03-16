use axum::{routing::get, Router};
use voda_runtime::{ExecutableFunctionCall, RuntimeClient};

pub fn misc_routes<S: RuntimeClient<F>, F: ExecutableFunctionCall>() -> Router<S> {
    Router::new()
        .route("/health", 
            get(|| async { "OK" })
        )
}
