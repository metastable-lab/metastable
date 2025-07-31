use sqlx::types::Uuid;
use metastable_common::get_current_timestamp;
use metastable_runtime::{User, UserRole};

pub fn get_admin_user() -> User {
    User {
        id: Uuid::new_v4(),
        user_id: format!("test_user_1"),
        user_aka: "Sandbox Admin".to_string(),
        role: UserRole::Admin,
        provider: "sandbox".to_string(),
        generated_referral_count: 0,
        running_claimed_balance: 0,
        running_purchased_balance: 0,
        running_misc_balance: 0,
        balance_usage: 0,
        free_balance_claimed_at: 0,
        last_balance_deduction_at: 0,
        first_name: None,
        last_name: None,
        email: None,
        phone: None,
        avatar: None,
        bio: None,
        extra: None,
        llm_access_level: 0,
        created_at: get_current_timestamp(),
        updated_at: get_current_timestamp(),
    }
}

pub fn get_normal_user() -> User {
    User {
        id: Uuid::new_v4(),
        user_id: format!("test_user_2"),
        user_aka: "Sandbox User".to_string(),
        role: UserRole::User,
        provider: "sandbox".to_string(),
        generated_referral_count: 0,
        running_claimed_balance: 0,
        running_purchased_balance: 0,
        running_misc_balance: 0,
        balance_usage: 0,
        free_balance_claimed_at: 0,
        last_balance_deduction_at: 0,
        first_name: None,
        last_name: None,
        email: None,
        phone: None,
        avatar: None,
        bio: None,
        extra: None,
        llm_access_level: 0,
        created_at: get_current_timestamp(),
        updated_at: get_current_timestamp(),
    }
}