use voda_common::{blake3_hash, CryptoHash};
use voda_db_macros::SqlxObject;
use voda_database::sqlx_postgres::SqlxPopulateId;
use serde::{Serialize, Deserialize}; // Added for potential future use

#[derive(Clone, Debug, Default, Serialize, Deserialize, SqlxObject)]
#[table_name = "users"]
pub struct User {
    pub id: CryptoHash,
    pub username: String,
    // Add other user-specific fields here if needed in the future
    // e.g., email: Option<String>, created_at: i64, etc.
}

impl SqlxPopulateId for User {
    fn sql_populate_id(&mut self) {
        // Populate ID based on username only if ID is zero (default) and username is not empty.
        // This allows for ID to be set manually or by other means if necessary.
        if *self.id.hash() == [0u8; 32] && !self.username.is_empty() {
            self.id = blake3_hash(self.username.as_bytes());
        }
        // If username is empty and ID is zero, the ID remains zero.
        // Creation might fail if ID is a primary key and not nullable,
        // which our macro sets up unless it's an Option.
        // For a non-Option ID, it must be populated before insert.
    }
}

// Optional: Add helper methods for User struct if needed
// impl User {
//     pub fn new(username: String) -> Self {
//         let mut user = User {
//             id: CryptoHash::default(), // Will be populated by sql_populate_id or manually
//             username,
//         };
//         user.sql_populate_id(); // Ensure ID is populated if username is provided
//         user
//     }
// } 