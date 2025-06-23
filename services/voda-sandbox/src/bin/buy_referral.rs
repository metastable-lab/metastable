use anyhow::Result;
use colored::*;
use reqwest::Client;
use tracing::info;
use voda_database::{init_db_pool, QueryCriteria, SqlxCrud, SqlxFilterQuery};
use voda_runtime::User;
use voda_sandbox::{api_client::ApiClient, config::get_normal_user};

init_db_pool!(
    voda_runtime::User,
    voda_runtime::UserUsage,
    voda_runtime::UserUrl,
    voda_runtime::UserReferral,
    voda_runtime::UserBadge,
    voda_runtime::SystemConfig,
    voda_runtime_roleplay::Character,
    voda_runtime_roleplay::RoleplaySession,
    voda_runtime_roleplay::RoleplayMessage,
    voda_runtime_roleplay::AuditLog
);

const BASE_URL: &str = "http://localhost:3033";

async fn get_or_create_user(pool: &sqlx::PgPool) -> Result<User> {
    let template_user = get_normal_user();
    let user_id = template_user.user_id.clone();
    let criteria = QueryCriteria::new()
        .add_filter("user_id", "=", Some(user_id))?
        .limit(1)?;

    match User::find_one_by_criteria(criteria, pool).await? {
        Some(user) => {
            info!(
                "Found existing user in DB: {} ({})",
                user.user_aka.cyan(),
                user.id
            );
            Ok(user)
        }
        None => {
            info!("User not found, creating a new one in the database...");
            let new_user = template_user.create(pool).await?;
            info!(
                "Created new user in DB: {} ({})",
                new_user.user_aka.cyan(),
                new_user.id
            );
            Ok(new_user)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let db = connect(false, false).await;
    let user = get_or_create_user(&db).await?;
    info!("Welcome, {}! (ID: {})", user.user_aka.cyan(), user.id);
    let secret_key = std::env::var("SECRET_SALT").expect("SECRET_SALT must be set");

    let http_client = Client::new();
    let api_client = ApiClient::new(
        BASE_URL.to_string(),
        user.clone(),
        secret_key.clone(),
        http_client.clone(),
    );

    println!("Buying 5 referrals...");
    match api_client.buy_referral(5).await {
        Ok(_) => {
            println!("{}", "Successfully bought 20 referrals.".green());
        }
        Err(e) => {
            eprintln!(
                "{}{}",
                "Failed to buy referrals: ".red(),
                e.to_string().red()
            );
        }
    }

    Ok(())
}
