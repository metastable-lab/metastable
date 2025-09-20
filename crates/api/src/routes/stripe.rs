use std::str::FromStr;

use anyhow::anyhow;
use axum::{
    extract::{Extension, State, Path},
    http::{StatusCode, HeaderMap},
    routing::post, Router,
};
use metastable_common::EnvVars;
use serde_json::json;
use stripe::{
    CheckoutSession, CheckoutSessionMode, CreateCheckoutSession, CreateCheckoutSessionLineItems, CreateCheckoutSessionPaymentMethodOptions, CreateCheckoutSessionPaymentMethodOptionsWechatPay, CreateCheckoutSessionPaymentMethodOptionsWechatPayClient, CreateCheckoutSessionPaymentMethodTypes, EventObject, EventType, Webhook
};
use sqlx::types::Uuid;

use crate::{
    ensure_account, middleware::authenticate, response::{AppError, AppSuccess}, ApiServerEnv, GlobalState
};
use metastable_database::{QueryCriteria, SqlxCrud, SqlxFilterQuery};
use metastable_runtime::{User, UserNotification, UserPayment, UserPaymentStatus};
use metastable_common::ModuleClient;

pub fn stripe_routes() -> Router<GlobalState> {
    Router::new()
        .route(
            "/stripe/checkout/{product_id}",
            post(create_checkout_session)
                .route_layer(axum::middleware::from_fn(authenticate)),
        )
        .route("/stripe/webhook", post(stripe_webhook))
}

async fn create_checkout_session(
    State(state): State<GlobalState>,
    Extension(user_id_str): Extension<String>,
    Path(product_id): Path<String>,
    headers: HeaderMap,
) -> Result<AppSuccess, AppError> {
    let user = ensure_account(&state.db, &user_id_str)
        .await?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[create_checkout_session] User not found")))?;

    println!("Header {:?}", headers.get("origin"));
    let origin = headers
        .get("origin")
        .and_then(|o| o.to_str().ok())
        .unwrap_or("http://localhost:3000");

    let user_email = user.user_id.split("email_").nth(1)
        .ok_or(AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("[stripe_checkout] email not avaliable")))?;

    let user_id = user.id.clone();
    let user_id_str = user_id.to_string();
    let success_url = format!("{}/setting/recharge?status=success", origin);
    let cancel_url = format!("{}/setting/recharge?status=success", origin);

    let mut payment_option = CreateCheckoutSessionPaymentMethodOptions::default();
    payment_option.wechat_pay = Some(CreateCheckoutSessionPaymentMethodOptionsWechatPay {
        client: CreateCheckoutSessionPaymentMethodOptionsWechatPayClient::Web,
        ..Default::default()
    });

    let params = CreateCheckoutSession {
        customer_email: Some(user_email),
        client_reference_id: Some(&user_id_str),
        payment_method_types: Some(vec![
            CreateCheckoutSessionPaymentMethodTypes::Alipay,
            CreateCheckoutSessionPaymentMethodTypes::WechatPay,
            CreateCheckoutSessionPaymentMethodTypes::Paynow,
        ]),
        payment_method_options: Some(payment_option),
        line_items: Some(vec![CreateCheckoutSessionLineItems {
            price: Some(product_id.clone()),
            quantity: Some(1),
            ..Default::default()
        }]),
        mode: Some(CheckoutSessionMode::Payment),
        success_url: Some(&success_url), 
        cancel_url: Some(&cancel_url),
        allow_promotion_codes: Some(true),
        ..Default::default()
    };

    let session = CheckoutSession::create(&state.stripe_client, params)
        .await
        .map_err(|e| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("Stripe error: {}", e)))?;
    let url = session.url
        .ok_or_else(|| AppError::new(StatusCode::INTERNAL_SERVER_ERROR, anyhow!("Stripe error: no session url")))?;

    let vip_level = match product_id.as_str() {
        "price_1S7ciiK4f3Ep0fmJMAC6YvSR" => 1,
        "price_1S7ckVK4f3Ep0fmJghyxiL0N" => 2,
        "price_1S7cljK4f3Ep0fmJACVeWBXw" => 3,
        _ => 0,
    };

    // let vip_level_prod = match product_id.as_str() {
    //     "price_1S90m9GokBlvEWxhYFl3v9m4" => 1,
    //     "price_1S90m3GokBlvEWxhQhpPsFsT" => 2,
    //     "price_1S90lzGokBlvEWxhmmCEOBwg" => 3,
    //     _ => 0,
    // };

    let items = session.line_items.unwrap_or_default().data;
    let user_payment = UserPayment {
        id: Uuid::default(),
        user_id: user.id.clone(),
        checkout_session_id: session.id.to_string(),
        url: url.clone(),

        amount_total: session.amount_total.unwrap_or_default(),
        currency: session.currency.unwrap_or_default().to_string(),
        items: sqlx::types::Json(serde_json::to_value(&items)?),
        status: UserPaymentStatus::Pending,
        vip_level,
        created_at: 0,
        updated_at: 0,
    };
    let mut tx = state.db.get_client().begin().await?;
    user_payment.create(&mut *tx).await?;
    tx.commit().await?;

    Ok(AppSuccess::new(
        StatusCode::OK,
        "Checkout session created",
        json!({ "url": url }),
    ))
}

async fn stripe_webhook(
    State(state): State<GlobalState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<AppSuccess, AppError> {
    let env = ApiServerEnv::load();
    let sig = headers
        .get("stripe-signature")
        .and_then(|s| s.to_str().ok())
        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("Missing stripe-signature header")))?;

    let event = Webhook::construct_event(
        &String::from_utf8(body.to_vec()).unwrap(),
        sig,
        &env.get_env_var("STRIPE_WEBHOOK_SECRET"),
    )
    .map_err(|e| AppError::new(StatusCode::BAD_REQUEST, anyhow!("Webhook error: {}", e)))?;

    if let EventObject::CheckoutSession(session) = event.data.object {
        let session_id = session.id.to_string();
        let mut tx = state.db.get_client().begin().await?;
        match event.type_ {
            EventType::CheckoutSessionCompleted => {
                let user_id = session.client_reference_id.clone().ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, anyhow!("Missing client_reference_id")))?;
                let user_uuid = Uuid::from_str(&user_id);
                if user_uuid.is_err() {
                    tracing::error!("[stripe_webhook] Invalid user id: {}", user_id);
                } else {
                    let mut user = User::find_one_by_criteria(
                        QueryCriteria::new().add_valued_filter("id", "=", user_uuid.unwrap()),
                        &mut *tx
                    ).await?.ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, anyhow!("[stripe_webhook] User not found")))?;

                    let maybe_payment = UserPayment::find_one_by_criteria(
                        QueryCriteria::new().add_valued_filter("checkout_session_id", "=", session_id.clone()),
                        &mut *tx
                    ).await?;
    
                    if maybe_payment.is_none() {
                        tracing::error!("[stripe_webhook] Payment not found for session {} for user {}", session_id, user_id);
                    } else {
                        let mut payment = maybe_payment.unwrap();
                        if payment.user_id.to_string() != user_id {
                            tracing::error!("[stripe_webhook] user_id mismatch, unexpected session: {} user_id:     {}", session_id, user_id);
                        } else {
                            payment.status = UserPaymentStatus::Completed;
                            let log = user.purchase(payment.vip_level);
                            tracing::info!("[stripe_webhook] Purchase successful for user {} on level {}", user_id, payment.vip_level);
                            let notify = UserNotification::payment_processed(user.id.clone(), format!("Payment proceed at level {}", payment.vip_level));
                            notify.create(&mut *tx).await?;
                            payment.update(&mut *tx).await?;
                            log.create(&mut *tx).await?;
                            user.update(&mut *tx).await?;
                        }
                    }
                }
            }
            _ => {
                // unhandled event type
            }
        }

        tx.commit().await?;
    }
    Ok(AppSuccess::new(StatusCode::OK, "Webhook received", json!({})))
}
