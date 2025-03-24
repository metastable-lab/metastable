use axum::{routing::get, Router};
use voda_runtime::RuntimeClient;

pub fn misc_routes<S: RuntimeClient>() -> Router<S> {
    Router::new()
        .route("/health", 
            get(|| async { "OK" })
        )
}
