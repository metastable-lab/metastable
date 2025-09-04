use anyhow::anyhow;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use serde_json::json;
use axum::{
    extract::{Extension, State}, 
    http::StatusCode, middleware, 
    routing::post, Json, Router
};

use metastable_common::{encrypt, EnvVars, ModuleClient};
use metastable_database::{QueryCriteria, SqlxFilterQuery, SqlxCrud};
use metastable_runtime::{User, UserRole};

use crate::{
    ensure_account, middleware::authenticate, response::{AppError, AppSuccess}, utils::{generate_otp, generate_timebased_counter, verify_otp}, ApiServerEnv, GlobalState
};

pub fn auth_routes() -> Router<GlobalState> {
    Router::new()
        .route("/auth/send_otp",
            post(send_otp)
        )
        .route("/auth/login",
            post(login)
        )

        .route("/auth/session",
            get(session)
            .route_layer(middleware::from_fn(authenticate))
        )

        .route("/auth/bind_email", 
            post(bind_email)
            .route_layer(middleware::from_fn(authenticate))
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendOtpRequest { pub email: String }
async fn send_otp(
    Json(payload): Json<SendOtpRequest>,
) -> Result<AppSuccess, AppError> {
    let env = ApiServerEnv::load();
    let maileroo_api_key = env.get_env_var("MAILEROO_API_KEY");

    let otp = generate_otp(
        &format!("email_{}", payload.email),
        generate_timebased_counter(),
        &env.get_env_var("OTP_SECRET_KEY")
    );
    // Compose the form data for the Maileroo API
    let email_body = format!("Your OTP is: {}. It will expire in 5 minutes.", otp);
    let form = vec![
        ("from", "Metastable <dudu@metastable.art>"),
        ("to", &payload.email),
        ("subject", "Metastable OTP"),
        ("plain", email_body.as_str()),
    ];

    // Use reqwest to send the POST request
    let client = reqwest::Client::new();
    let res = client
        .post("https://smtp.maileroo.com/send")
        .header("X-API-Key", maileroo_api_key)
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[send_otp] Failed to send email: {}", e)))?;

    let status = res.status();
    let result_json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[send_otp] Failed to parse Maileroo response: {}", e)))?;

    let success = result_json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
    let message = result_json.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

    if !status.is_success() || !success {
        tracing::error!("[send_otp] Error sending email: {}", message);
        return Err(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[send_otp] Error sending email: {}", message)));
    } else {
        tracing::info!("[send_otp] Email sent successfully: {}", message);
    }

    Ok(AppSuccess::new(StatusCode::OK, "OTP sent successfully", json!(())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest { pub raw_id: String, pub otp: String, pub provider: String }
async fn login(
    Json(payload): Json<LoginRequest>,
) -> Result<AppSuccess, AppError> {
    let env = ApiServerEnv::load();

    let user_id = match payload.provider.as_str() {
        "email" => format!("email_{}", payload.raw_id),
        "phone" => format!("phone_86_{}", payload.raw_id),
        _ => return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[login] Invalid provider: {}", payload.provider))),
    };

    if verify_otp(
        &user_id, &payload.otp,
        &env.get_env_var("OTP_SECRET_KEY")
    ) {
        let payload = json!({
            "user_id": user_id,
            "timestamp": metastable_common::get_current_timestamp(),
            "origin": "api-auth"
        });
        let payload_str = payload.to_string();
        let auth_token = encrypt(&payload_str, &env.get_env_var("SECRET_SALT"))
            .expect("[User::generate_auth_token] failed to encrypt auth token");

        Ok(AppSuccess::new(StatusCode::OK, "Login successful", json!({
            "auth_token": auth_token
        })))
    } else {
        Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[login] Invalid OTP")))
    }
}

async fn session(
    State(state): State<GlobalState>,
    Extension(user_id): Extension<String>,
) -> Result<AppSuccess, AppError> {
    let mut tx = state.db.get_client().begin().await?;
    let user = User::find_one_by_criteria(
        QueryCriteria::new().add_valued_filter("user_id", "=", user_id.clone()),
        &mut *tx
    ).await?;

    let (is_registered, is_admin) = if let Some(mut user) = user {
        if user.banned {
            return Err(AppError::new(StatusCode::FORBIDDEN, anyhow!("[session] user id banned")));
        }

        // let _ = user.daily_checkin(100); // try daily checkin - DISABLED
        let is_admin = user.role == UserRole::Admin;
        user.update(&mut *tx).await?;
        (true, is_admin)
    } else {
        (false, false)
    };

    tx.commit().await?;
    Ok(AppSuccess::new(StatusCode::OK, "Session data", json!({
        "user_id": user_id,
        "is_admin": is_admin,
        "is_registered": is_registered,
    })))
}


#[derive(Debug, Serialize, Deserialize)]
pub struct BindEmailRequest {
    pub raw_id: String,
    pub otp: String,
}
async fn bind_email(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Json(payload): Json<BindEmailRequest>,
) -> Result<AppSuccess, AppError> {
    let env = ApiServerEnv::load();
    let mut user = ensure_account(&state.db, &user_id_str).await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[bind_email] User not found")))?;
    
    // Quick validation if raw_id is a valid email address
    let raw_id = payload.raw_id.trim();
    if !raw_id.contains('@') || raw_id.starts_with('@') || raw_id.ends_with('@') {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[id_to_email] Invalid email address")));
    }

    if verify_otp(
        &format!("email_{}", raw_id), &payload.otp,
        &env.get_env_var("OTP_SECRET_KEY")
    ) {
        let mut tx = state.db.get_client().begin().await?;
        user.phone = Some(user.user_id.clone());
        user.user_id = format!("email_{}", raw_id);
        user.provider = "email".to_string();

        user.update(&mut *tx).await?;
        tx.commit().await?;
    } else {
        return Err(AppError::new(StatusCode::BAD_REQUEST, anyhow!("[bind_email] Invalid OTP")));
    }

    Ok(AppSuccess::new(StatusCode::OK, "Email bound successfully", json!(())))
}