use anyhow::{anyhow, Result};
use async_openai::types::CompletionUsage;
use serde::{Deserialize, Serialize};
use voda_common::{get_current_timestamp, CryptoHash, blake3_hash};
use strum_macros::{Display, EnumString}; // Note: StrumDefault alias
use voda_db_macros::SqlxObject; // Added for SqlxObject derive
use voda_database::sqlx_postgres::SqlxPopulateId; // Added for SqlxPopulateId trait

pub const BALANCE_CAP: u64 = 500;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Display, EnumString, Default)] // Added strum derives
pub enum UserRole {
    Admin,
    #[default]
    User,
}

#[derive(Debug, Serialize, Deserialize, Clone, SqlxObject, Default)] // Added Default here as well for the main User struct
#[table_name = "users"] // Define the SQL table name
pub struct User {
    #[serde(rename = "_id")]
    pub id: CryptoHash,
    pub user_id: String,

    pub role: UserRole,
    pub provider: String,
    pub network_name: Option<String>,

    #[sqlx_skip_column]         // This field will be skipped for direct SQL column mapping
    pub profile: UserProfile,
    #[sqlx_skip_column]         // This field will be skipped
    pub points: UserPoints,
    #[sqlx_skip_column]         // This field will be skipped
    pub usage: Vec<UserUsage>,

    pub last_active: i64,
    pub created_at: i64,
}

impl SqlxPopulateId for User {
    fn sql_populate_id(&mut self) {
        if *self.id.hash() == [0u8; 32] && !self.user_id.is_empty() {
            self.id = blake3_hash(self.user_id.as_bytes());
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)] // Added PartialEq
pub struct UserProfile {
    pub id: CryptoHash,

    pub user_personality: Vec<String>,

    pub username: String,
    pub first_name: String, 
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
    pub bio: Option<String>,
}


#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)] // Added PartialEq
pub struct UserPoints {
    pub running_claimed_balance: u64,
    pub running_purchased_balance: u64,
    pub running_misc_balance: u64,

    pub balance_usage: u64,

    pub free_balance_claimed_at: u64,
    pub last_balance_deduction_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserUsage {
    pub created_at: u64,
    pub model_name: String,
    pub usage: CompletionUsage,
}

impl UserUsage {
    pub fn new(model_name: String, usage: CompletionUsage) -> Self {
        Self {
            created_at: get_current_timestamp(),
            model_name,
            usage,
        }
    }
}

impl UserPoints {
    /* BALANCE ADDITION */
    // Rate limit: ONE Claim per day
    // running balance sum should be <= BALANCE_CAP
    // if over BALANCE_CAP, reduce free balance first
    // don't reduce paid balance
    pub fn try_claim_free_balance(&mut self, amount: u64) -> Result<()> {
        let current_timestamp = get_current_timestamp();
        // ONE Claim per day
        if current_timestamp - self.free_balance_claimed_at < 24 * 60 * 60 {
            return Err(anyhow!("Too frequent to claim free balance"));
        }

        self.running_claimed_balance += amount;
        self.free_balance_claimed_at = current_timestamp;

        // capping logic - no errors
            if amount + 
            self.running_claimed_balance + 
            self.running_purchased_balance + 
            self.running_misc_balance > BALANCE_CAP 
        {
            if self.running_purchased_balance + self.running_misc_balance < BALANCE_CAP {
                self.running_claimed_balance = BALANCE_CAP - self.running_purchased_balance - self.running_misc_balance;
            } else {
                // noop
            }
        }

        Ok(())
    }

    pub fn purchase_balance(&mut self, amount: u64) {
        self.running_purchased_balance += amount;
    }

    pub fn add_misc_balance(&mut self, amount: u64) {
        self.running_misc_balance += amount;
    }

    /* BALANCE SUBTRACTION */
    pub fn pay(&mut self, amount: u64) -> bool {
        let mut remaining = amount;
        let self_clone = self.clone();
        let current_timestamp = get_current_timestamp();

        // Try free_claimed_balance first
        if self.running_claimed_balance > 0 {
            let deduct = remaining.min(self.running_claimed_balance);
            self.running_claimed_balance -= deduct;
            self.last_balance_deduction_at = current_timestamp;
            remaining -= deduct;
        }

        // Try misc_balance next
        if remaining > 0 {
            let deduct = remaining.min(self.running_misc_balance);
            self.running_misc_balance -= deduct;
            remaining -= deduct;
        }

        // Finally try paid_avaliable_balance
        if remaining > 0 {
            if self.running_purchased_balance >= remaining {
                self.running_purchased_balance -= remaining;
                self.last_balance_deduction_at = current_timestamp;
                remaining = 0;
            }
        }

        // If we couldn't pay the full amount, revert all changes
        if remaining > 0 {
            *self = self_clone;
            false
        } else {
            self.balance_usage += amount;
            true
        }
    }

    pub fn get_available_balance(&self) -> u64 {
        self.running_purchased_balance + self.running_claimed_balance + self.running_misc_balance
    }
}

#[cfg(test)]
mod sql_tests {
    use super::*; 
    use voda_database::sqlx_postgres::{SqlxCrud, SqlxSchema};
    use sqlx::{Executor, PgPool, Postgres, Transaction};
    use hex;
    use voda_common::blake3_hash;
    use tokio::sync::OnceCell; 

    static USER_TEST_POOL: OnceCell<PgPool> = OnceCell::const_new();

    // Initialize the database pool and ensure schema is set up once.
    async fn init_user_test_db() -> &'static PgPool {
        USER_TEST_POOL.get_or_init(|| async {
            eprintln!("User Tests: Initializing DB Pool and performing one-time schema setup for 'users' table...");
            let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_|
                "postgresql://postgres:IgbXkBSlvHeHMAdAbXJyTQEPNJunQwHg@crossover.proxy.rlwy.net:10849/railway".to_string()
            );
            let pool = PgPool::connect(&db_url).await.expect("Failed to connect to Postgres for USER_TEST_POOL");
            
            // Robust DDL: Drop table (handles composite type), then drop type just in case of other conflicts.
            let drop_table_res = pool.execute("DROP TABLE IF EXISTS \"users\" CASCADE;").await;
            if let Err(e) = &drop_table_res {
                if !e.to_string().to_lowercase().contains("does not exist") {
                    eprintln!("Error during (User Tests) DROP TABLE IF EXISTS users CASCADE: {:?}", e);
                }
            }

            let drop_type_res = pool.execute("DROP TYPE IF EXISTS \"users\" CASCADE;").await;
            if let Err(e) = &drop_type_res {
                if !e.to_string().to_lowercase().contains("does not exist") {
                     eprintln!("Error during (User Tests) DROP TYPE IF EXISTS users CASCADE: {:?}", e);
                }
            }
            
            let create_sql = User::create_table_sql();
            pool.execute(create_sql.as_str())
                .await
                .unwrap_or_else(|e| panic!("(User Tests) One-time schema setup: Failed to create 'users' table. Error: {:?}. SQL: {}", e, create_sql));
            eprintln!("User Tests: One-time schema setup for 'users' table COMPLETE.");
            pool
        }).await
    }

    // Helper to get a transaction for a test
    async fn begin_user_tx() -> Transaction<'static, Postgres> {
        let pool = init_user_test_db().await;
        pool.begin().await.expect("Failed to begin transaction for user test")
    }

    fn create_test_user(user_id_prefix: &str) -> User {
        let timestamp_nanos = get_current_timestamp();
        let unique_data = format!("{}_{}", user_id_prefix, timestamp_nanos);
        let hash_bytes = blake3_hash(unique_data.as_bytes()).hash().clone();
        let unique_suffix = hex::encode(&hash_bytes[0..8]);
        let user_id = format!("test_user_{}_{}", user_id_prefix, unique_suffix);
        let mut u = User {
            user_id,
            role: UserRole::User,
            provider: "test_provider".to_string(),
            network_name: Some("test_network".to_string()),
            last_active: get_current_timestamp() as i64,
            created_at: get_current_timestamp() as i64 - 3600,
            ..Default::default()
        };
        u.sql_populate_id();
        u
    }

    #[tokio::test]
    async fn test_user_create_and_find_by_id() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let test_user_original = create_test_user("crud1");
        let original_user_id = test_user_original.user_id.clone();
        let original_last_active = test_user_original.last_active;
        let original_created_at = test_user_original.created_at;
        let original_provider = test_user_original.provider.clone();
        let original_network_name = test_user_original.network_name.clone();
        let original_role = test_user_original.role.clone();
        let created_user = test_user_original.create(&mut tx).await?;
        assert_eq!(created_user.user_id, original_user_id);
        assert_ne!(*created_user.id.hash(), [0u8; 32], "ID should be populated");
        let fetched_user_opt = User::find_by_id(created_user.id.hash().to_vec(), &mut tx).await?;
        assert!(fetched_user_opt.is_some(), "User should be found by ID");
        let fetched_user = fetched_user_opt.unwrap();
        assert_eq!(fetched_user.id, created_user.id);
        assert_eq!(fetched_user.user_id, original_user_id);
        assert_eq!(fetched_user.role, original_role);
        assert_eq!(fetched_user.provider, original_provider);
        assert_eq!(fetched_user.network_name, original_network_name);
        assert_eq!(fetched_user.last_active, original_last_active);
        assert_eq!(fetched_user.created_at, original_created_at);
        assert_eq!(fetched_user.profile, UserProfile::default());
        assert_eq!(fetched_user.points, UserPoints::default());
        assert!(fetched_user.usage.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_user_update() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let user_to_create = create_test_user("update1");
        let original_user_id = user_to_create.user_id.clone(); 
        let original_created_at = user_to_create.created_at; 
        let mut created_user = user_to_create.create(&mut tx).await?;
        let id_of_created_user = created_user.id.clone();
        created_user.role = UserRole::Admin;
        created_user.network_name = None;
        let new_last_active = get_current_timestamp() as i64;
        created_user.last_active = new_last_active;
        created_user.provider = "updated_provider".to_string();
        let updated_user = created_user.update(&mut tx).await?;
        assert_eq!(updated_user.id, id_of_created_user);
        assert_eq!(updated_user.role, UserRole::Admin);
        assert_eq!(updated_user.network_name, None);
        assert_eq!(updated_user.last_active, new_last_active);
        assert_eq!(updated_user.provider, "updated_provider");
        assert_eq!(updated_user.user_id, original_user_id); 
        assert_eq!(updated_user.created_at, original_created_at);
        Ok(())
    }

    #[tokio::test]
    async fn test_user_delete() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let user_to_create = create_test_user("delete1");
        let user_id_for_check = user_to_create.user_id.clone();
        let created_user = user_to_create.create(&mut tx).await?;
        let id_of_created_user_for_check = created_user.id.clone();
        let deletion_result = created_user.delete(&mut tx).await?;
        assert!(deletion_result > 0, "Delete should return rows affected > 0");
        let fetched_user_opt = User::find_by_id(id_of_created_user_for_check.hash().to_vec(), &mut tx).await?;
        assert!(fetched_user_opt.is_none(), "User should not be found after deletion, checked user_id: {}", user_id_for_check);
        Ok(())
    }

    #[tokio::test]
    async fn test_user_find_all() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let user1 = create_test_user("findall1");
        let user1_id = user1.user_id.clone();
        user1.create(&mut tx).await?;
        let user2 = create_test_user("findall2");
        let user2_id = user2.user_id.clone();
        user2.create(&mut tx).await?;
        let all_users = User::find_all(&mut tx).await?;
        assert_eq!(all_users.len(), 2, "Should find 2 users within this transaction. Found: {}, Users: {:?}", all_users.len(), all_users.iter().map(|u| u.user_id.clone()).collect::<Vec<_>>());
        assert!(all_users.iter().any(|u| u.user_id == user1_id));
        assert!(all_users.iter().any(|u| u.user_id == user2_id));
        Ok(())
    }
    
    #[tokio::test]
    async fn test_user_role_handling() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let mut user_admin = create_test_user("role_admin");
        user_admin.role = UserRole::Admin;
        let created_admin = user_admin.create(&mut tx).await?;
        let fetched_admin_opt = User::find_by_id(created_admin.id.hash().to_vec(), &mut tx).await?;
        assert_eq!(fetched_admin_opt.unwrap().role, UserRole::Admin);
        let user_default = create_test_user("role_default");
        let created_default = user_default.create(&mut tx).await?;
        let fetched_default_opt = User::find_by_id(created_default.id.hash().to_vec(), &mut tx).await?;
        assert_eq!(fetched_default_opt.unwrap().role, UserRole::User);
        Ok(())
    }

    #[tokio::test]
    async fn test_user_nullable_network_name() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let mut user_with_network = create_test_user("net_some");
        user_with_network.network_name = Some("my_specific_network".to_string());
        let created_with_net = user_with_network.create(&mut tx).await?;
        let fetched_with_net = User::find_by_id(created_with_net.id.hash().to_vec(), &mut tx).await?.unwrap();
        assert_eq!(fetched_with_net.network_name, Some("my_specific_network".to_string()));
        let mut user_without_network = create_test_user("net_none");
        user_without_network.network_name = None;
        let created_without_net = user_without_network.create(&mut tx).await?;
        let fetched_without_net = User::find_by_id(created_without_net.id.hash().to_vec(), &mut tx).await?.unwrap();
        assert_eq!(fetched_without_net.network_name, None);
        Ok(())
    }

    #[tokio::test]
    async fn test_user_update_only_one_field() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let user = create_test_user("partial_update");
        let original_user_id_val = user.user_id.clone();
        let original_created_at_val = user.created_at;
        let original_provider_val = user.provider.clone();
        let original_role_val = user.role.clone();
        let mut created_user = user.create(&mut tx).await?;
        let new_last_active = created_user.last_active + 1000;
        created_user.last_active = new_last_active;
        let updated_user = created_user.update(&mut tx).await?;
        assert_eq!(updated_user.last_active, new_last_active);
        assert_eq!(updated_user.user_id, original_user_id_val);
        assert_eq!(updated_user.created_at, original_created_at_val);
        assert_eq!(updated_user.provider, original_provider_val);
        assert_eq!(updated_user.role, original_role_val);
        Ok(())
    }

    #[tokio::test]
    async fn test_user_sql_populate_id_idempotency() -> Result<(), anyhow::Error> {
        let _tx = begin_user_tx().await; // Ensure DB is set up, tx not directly used for this test logic if not interacting with DB
        let mut user = create_test_user("populate_idem");
        let first_id = user.id.clone();
        assert_ne!(*first_id.hash(), [0u8;32], "ID should be populated by create_test_user");
        user.sql_populate_id();
        assert_eq!(user.id, first_id, "sql_populate_id should be idempotent if id is already set");
        let mut user2 = create_test_user("populate_new");
        user2.id = CryptoHash::default();
        assert_eq!(*user2.id.hash(), [0u8;32], "ID should be default before population");
        user2.sql_populate_id();
        assert_ne!(*user2.id.hash(), [0u8;32], "ID should be populated from user_id if default");
        let expected_id = blake3_hash(user2.user_id.as_bytes());
        assert_eq!(user2.id, expected_id, "ID should be hash of user_id");
        Ok(())
    }

    #[tokio::test]
    async fn test_user_create_with_empty_user_id_if_id_is_populated() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let user = User {
            id: blake3_hash(b"pre_populated_id"),
            user_id: "".to_string(),
            role: UserRole::Admin,
            provider: "manual_id_provider".to_string(),
            last_active: get_current_timestamp() as i64,
            created_at: get_current_timestamp() as i64,
            ..Default::default()
        };
        let created_user = user.create(&mut tx).await?;
        assert_eq!(created_user.id, blake3_hash(b"pre_populated_id"));
        assert_eq!(created_user.user_id, "");
        let fetched_user = User::find_by_id(created_user.id.hash().to_vec(), &mut tx).await?.unwrap();
        assert_eq!(fetched_user.user_id, "");
        Ok(())
    }

    #[tokio::test]
    async fn test_user_fields_max_length_or_constraints_implicit() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let long_string = "a".repeat(10000);
        let mut user = create_test_user("long_fields");
        let user_id_suffix_part = long_string.chars().take(200).collect::<String>();
        user.user_id = format!("long_user_id_{}", user_id_suffix_part);
        user.provider = format!("long_provider_{}", long_string);
        user.network_name = Some(format!("long_network_{}", long_string));
        user.sql_populate_id();
        let created_user = user.create(&mut tx).await?;
        let fetched_user_opt = User::find_by_id(created_user.id.hash().to_vec(), &mut tx).await?;
        assert!(fetched_user_opt.is_some());
        let fetched_user = fetched_user_opt.unwrap();
        assert!(fetched_user.provider.contains(&long_string));
        assert!(fetched_user.network_name.as_ref().unwrap().contains(&long_string));
        assert!(fetched_user.user_id.starts_with("long_user_id_"));
        assert!(fetched_user.user_id.ends_with(&user_id_suffix_part), "User ID should end with the part of the long string used for its creation.");
        Ok(())
    }

    #[tokio::test]
    async fn test_update_non_existent_user() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let non_existent_user = create_test_user("non_existent");
        let update_res = non_existent_user.update(&mut tx).await;
        assert!(update_res.is_err(), "Update should fail for a non-existent user");
        
        match update_res.err().unwrap() {
            sqlx::Error::RowNotFound => { /* This is the expected error */ Ok(()) }
            e => panic!("Expected sqlx::Error::RowNotFound, got different error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_delete_non_existent_user() -> Result<(), anyhow::Error> {
        let mut tx = begin_user_tx().await;
        let non_existent_crypto_hash = blake3_hash(b"does_not_exist");
        let non_existent_user_instance = User {
            id: non_existent_crypto_hash,
            user_id: "user_that_does_not_exist".to_string(),
            ..Default::default()
        };
        let deletion_result = non_existent_user_instance.delete(&mut tx).await?;
        assert_eq!(deletion_result, 0, "Delete should affect 0 rows for a non-existent user ID");
        Ok(())
    }
}
