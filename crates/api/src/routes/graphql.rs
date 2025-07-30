use anyhow::anyhow;
use axum::{
    body::{to_bytes, Body}, extract::{Extension, State}, http::{header::{self, HeaderValue}, Request, StatusCode}, middleware, response::Response, routing::post, Router
};
use sqlx::types::Uuid;
use metastable_common::EnvVars;
use metastable_runtime::UserRole;

use crate::{
    ensure_account, env::ApiServerEnv, middleware::authenticate, response::AppError, GlobalState
};

pub fn graphql_route() -> Router<GlobalState> {
    Router::new().route(
        "/graphql",
        post(proxy_to_hasura)
            .route_layer(middleware::from_fn(authenticate)),
    )
}

async fn proxy_to_hasura(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    req: Request<Body>,
) -> Result<Response, AppError> {
    let env = ApiServerEnv::load();
    let hasura_url = env.get_env_var("HASURA_GRAPHQL_URL");
    let (maybe_user, _) = ensure_account(&state.roleplay_client, &user_id_str, 0).await?;
    let (parts, body) = req.into_parts();
    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .map_err(|e| AppError::new(StatusCode::BAD_REQUEST, anyhow!(e)))?;

    let mut headers = parts.headers.clone();
    headers.remove(header::AUTHORIZATION);
    headers.remove(header::HOST);
    let (user_id, user_role) = match maybe_user {
        None => {
            (Uuid::nil(), "anyone")
        }
        Some(ref user) => {
            let role = match user.role {
                UserRole::Admin => "admin",
                UserRole::User => "user",
            };
            (user.id.clone(), role)
        }
    };

    headers.insert(
        "X-Hasura-Role",
        user_role.parse::<HeaderValue>()
            .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))?,
    );
    headers.insert(
        "X-Hasura-User-Id",
        user_id
            .to_string()
            .parse::<HeaderValue>()
            .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))?,
    );
    headers.insert(
        "X-Hasura-Admin-Secret",
        env.get_env_var("HASURA_GRAPHQL_ADMIN_SECRET")
            .to_string()
            .parse::<HeaderValue>()
            .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))?,
    );

    let hasura_response = state.http_client
        .request(parts.method, &hasura_url)
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))?;

    let mut response_builder = Response::builder().status(hasura_response.status());

    if let Some(res_headers) = response_builder.headers_mut() {
        for (key, value) in hasura_response.headers() {
            if key != header::CONNECTION
                && key != header::TRANSFER_ENCODING
                && key != header::CONTENT_LENGTH
                && key != "keep-alive"
                && key != header::UPGRADE
                && key != header::PROXY_AUTHENTICATE
                && key != header::PROXY_AUTHORIZATION
                && key != header::TE
                && key != header::TRAILER
            {
                res_headers.insert(key.clone(), value.clone());
            }
        }
    }

    let response_body = hasura_response
        .bytes()
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))?;
        
    response_builder
        .body(Body::from(response_body))
        .map_err(|e: axum::http::Error| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!(e)))
}



