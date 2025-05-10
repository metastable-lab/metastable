use axum::{routing::get, Router};
use voda_runtime::RuntimeClient;

pub fn misc_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/health", 
            get(|| async { "OK" })
        )
        .route("/metrics", 
            get(metrics)
        )
}

async fn metrics() -> String {
    let metrics = prometheus::TextEncoder::new()
        .encode_to_string(&prometheus::gather()).unwrap();
    metrics
}