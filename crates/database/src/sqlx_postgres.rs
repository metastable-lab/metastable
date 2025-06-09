use sqlx::{FromRow, Postgres, Error as SqlxError, postgres::PgArguments, Executor};
use voda_common::CryptoHash;

/// Trait for custom primary key population logic for SqlxObject.
pub trait SqlxPopulateId {
    /// Populates the primary key field (`id`) of the struct.
    fn sql_populate_id(&mut self) -> anyhow::Result<()>;
}

/// Trait to define the schema of a database object for PostgreSQL.
// No async_trait needed here as no methods are async by default in the trait itself.
pub trait SqlxSchema: Send + Sync + Unpin + Clone + std::fmt::Debug + SqlxPopulateId {
    /// The type of the primary key for this database object.
    type Id: Send + Sync + for<'q> sqlx::Encode<'q, Postgres> + sqlx::Type<Postgres> + Clone;

    /// The intermediate type that implements FromRow, used for fetching from the database.
    type Row: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin;

    const TABLE_NAME: &'static str;
    const ID_COLUMN_NAME: &'static str;
    const COLUMNS: &'static [&'static str];

    // Default utility methods to access consts 
    fn id_column_name() -> &'static str { Self::ID_COLUMN_NAME }
    fn table_name() -> &'static str { Self::TABLE_NAME }
    fn columns() -> &'static [&'static str] { Self::COLUMNS }

    /// Retrieves the value of the primary key for an instance of the object.
    fn get_id_value(&self) -> Self::Id;

    /// Converts the intermediate Row type to the Self type.
    fn from_row(row: Self::Row) -> Self;

    // SQL generation methods (to be implemented by the derive macro)
    fn create_table_sql() -> String;
    fn drop_table_sql() -> String;
    fn insert_sql() -> String;
}

/// Trait for CRUD (Create, Read, Update, Delete) operations for PostgreSQL.
#[async_trait::async_trait]
pub trait SqlxCrud: SqlxSchema + SqlxFilterQuery + Sized {
    /// Binds the struct fields to an insert query.
    fn bind_insert<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Binds the struct fields to an update query (typically for updating by ID).
    fn bind_update<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Creates a new record in the database.
    async fn create<'e, E>(mut self, executor: E) -> Result<Self, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send;

    /// Updates an existing record in the database (identified by its primary key).
    /// The derive macro will implement this using a specific update-by-ID SQL query.
    async fn update<'e, E>(self, executor: E) -> Result<Self, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send;

    /// Deletes a record from the database by its primary key.
    /// The derive macro will implement this using a specific delete-by-ID SQL query.
    async fn delete<'e, E>(self, executor: E) -> Result<u64, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send;
} 

/// Initializes a global, static PostgreSQL connection pool (`PgPool`) and ensures that
/// database tables for specified types are created if they do not already exist.
///
/// This macro is designed to be called once, typically at the start of your application.
/// It defines an `async fn connect() -> &'static PgPool` function in the scope where it's invoked.
/// Subsequent calls to `connect().await` will return a reference to the initialized pool.
///
/// # Arguments
/// The macro takes a comma-separated list of types that implement the `SqlxSchema` trait.
/// For each type provided, it will attempt to execute the SQL statement returned by
/// `<YourType as SqlxSchema>::create_table_sql()`.
///
/// # Generated Function
/// - `async fn connect() -> &'static PgPool`: An asynchronous function that, when called,
///   returns a static reference to the `PgPool`. The first call initializes the pool
///   and runs the table creation logic. Subsequent calls return the existing pool.
///
/// # Panics
/// - If the `POSTGRES_URI` environment variable is not set or is invalid.
/// - If connecting to the PostgreSQL database fails.
/// - If any of the table creation SQL queries fail to execute. The panic message will
///   include the type for which table creation failed and the problematic SQL query.
///
/// # Example
/// ```rust,ignore
/// // Assume User and Post implement SqlxSchema
/// mod my_models {
///     use voda_database::SqlxSchema; // and other necessary traits/structs
///     // ... User struct definition deriving SqlxObject ...
///     // ... Post struct definition deriving SqlxObject ...
/// }
///
/// // In your main.rs or relevant setup module:
/// use voda_database::init_db_pool;
/// use my_models::{User, Post};
///
/// init_db_pool!(User, Post);
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let pool = connect().await; // connect() is generated by init_db_pool!
///     // Now you can use the pool for database operations.
///     // For example, User::find_all(pool).await?;
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! init_db_pool {
    ($($target_type:ty),*) => {
        use $crate::SqlxSchema; // Assumes SqlxSchema is accessible via $crate (voda_database::SqlxSchema)

        static POOL: tokio::sync::OnceCell<PgPool> = tokio::sync::OnceCell::const_new();

        // To use this macro, call it like this:
        // init_db_pool!(MyType1, MyType2, MyType3);
        // 
        // It defines an async function with signature:
        // async fn connect() -> &'static PgPool
        // which will connect to the database and ensure tables for MyType1, MyType2, MyType3 are created.
        //
        // Example usage:
        // let pool = connect().await;
        async fn connect() -> &'static PgPool {
            POOL.get_or_init(|| async {
                let database_url = std::env::var("DATABASE_URL").unwrap();
                
                let pool = PgPool::connect(&database_url).await
                    .expect("Failed to connect to Postgres. Ensure DB is running and POSTGRES_URI is correct.");

                // Drop tables first to ensure a clean schema for tests
                $( 
                    let drop_table_sql_str = <$target_type as $crate::SqlxSchema>::drop_table_sql();
                    if !drop_table_sql_str.trim().is_empty() { 
                        sqlx::query(&drop_table_sql_str)
                            .execute(&pool)
                            .await
                            .unwrap_or_else(|e| {
                                // Log a warning instead of panic for drop errors, as table might not exist
                                eprintln!(
                                    "Warning: Failed to drop table for type '{}'. SQL: \"{}\". Error: {:?}. This might be okay if the table didn't exist.", 
                                    stringify!($target_type), 
                                    drop_table_sql_str, 
                                    e
                                );
                                // Return a default/dummy ExecutionResult or similar if needed, though for unwrap_or_else it expects the same type as Ok variant.
                                // Since we are not using the result of drop, just logging is fine here.
                                sqlx::postgres::PgQueryResult::default() // Provide a dummy result to satisfy unwrap_or_else type if it were strictly needed, but simple unwrap_or_else with eprintln is fine.
                            });
                    }
                )*

                // Create tables for each specified type
                $( 
                    let create_table_sql_str = <$target_type as $crate::SqlxSchema>::create_table_sql();
                    if !create_table_sql_str.trim().is_empty() { // Basic check to avoid empty SQL
                        sqlx::query(&create_table_sql_str)
                            .execute(&pool)
                            .await
                            .unwrap_or_else(|e| panic!(
                                "Failed to create table for type '{}'. SQL: \"{}\". Error: {:?}. Check SQL schema and permissions.", 
                                stringify!($target_type), 
                                create_table_sql_str, 
                                e
                            ));
                    } else {
                        eprintln!("Skipping table creation for {} as create_table_sql() returned empty string.", stringify!($target_type));
                    }
                )*

                pool
            }).await
        }
    };
}

// --- Filtering Structures and Trait ---

/// Specifies the direction for ordering query results.
#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    Asc,
    Desc,
}

impl OrderDirection {
    pub fn as_sql(&self) -> &'static str {
        match self {
            OrderDirection::Asc => "ASC",
            OrderDirection::Desc => "DESC",
        }
    }
}

/// Represents a single filter condition for a database query.
pub struct FilterCondition {
    pub column: &'static str,
    pub operator: &'static str,
    pub uses_placeholder: bool,
}

/// Represents the complete criteria for a filtered database query.
pub struct QueryCriteria {
    pub conditions: Vec<FilterCondition>,
    pub arguments: ::sqlx::postgres::PgArguments,
    pub has_limit: bool,
    pub has_offset: bool,
    pub order_by: Vec<(&'static str, OrderDirection)>,
}

impl QueryCriteria {

    pub fn by_id(id: &CryptoHash) -> Result<Self, SqlxError> {
        Self::new().add_filter("id", "=", Some(id.to_hex_string()))
    }

    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            arguments: ::sqlx::postgres::PgArguments::default(),
            has_limit: false,
            has_offset: false,
            order_by: Vec::new(),
        }
    }

    pub fn add_filter<V>(mut self, column: &'static str, operator: &'static str, value: Option<V>) -> Result<Self, SqlxError>
    where
        V: for<'a> ::sqlx::Encode<'a, Postgres> + ::sqlx::Type<Postgres> + Send + Sync + 'static,
    {
        use ::sqlx::Arguments;
        let uses_ph = value.is_some();
        if let Some(val) = value {
            self.arguments.add(val).map_err(SqlxError::Encode)?;
        }
        self.conditions.push(FilterCondition {
            column,
            operator,
            uses_placeholder: uses_ph,
        });
        Ok(self)
    }

    pub fn add_valued_filter<V>(self, column: &'static str, operator: &'static str, value: V) -> Result<Self, SqlxError>
    where
        V: for<'a> ::sqlx::Encode<'a, Postgres> + ::sqlx::Type<Postgres> + Send + Sync + 'static,
    {
        self.add_filter(column, operator, Some(value))
    }
    
    pub fn limit(mut self, limit_val: i64) -> Result<Self, SqlxError> {
        use ::sqlx::Arguments;
        self.arguments.add(limit_val).map_err(SqlxError::Encode)?;
        self.has_limit = true;
        Ok(self)
    }

    pub fn offset(mut self, offset_val: i64) -> Result<Self, SqlxError> {
        use ::sqlx::Arguments;
        self.arguments.add(offset_val).map_err(SqlxError::Encode)?;
        self.has_offset = true;
        Ok(self)
    }

    pub fn order_by(mut self, column: &'static str, direction: OrderDirection) -> Result<Self, SqlxError> {
        self.order_by.push((column, direction));
        Ok(self)
    }
}

/// Trait for finding records based on dynamic filter criteria.
#[async_trait::async_trait]
pub trait SqlxFilterQuery: SqlxSchema + Sized {
    /// Finds records based on the provided criteria.
    /// The implementation of this method is typically generated by the SqlxObject derive macro.
    async fn find_by_criteria<'e, E>(
        criteria: QueryCriteria,
        executor: E,
    ) -> Result<Vec<Self>, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send;

    /// Finds a single optional record based on the provided criteria.
    /// If multiple records match, this default implementation takes the first one returned by find_by_criteria.
    /// For more control, ensure criteria include ordering and LIMIT 1, or implement this method directly.
    async fn find_one_by_criteria<'e, E>(
        mut criteria: QueryCriteria, // Take ownership to potentially add LIMIT 1
        executor: E,
    ) -> Result<Option<Self>, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send
    {
        // Default implementation: ensure LIMIT 1 and use find_by_criteria.
        if !criteria.has_limit {
            criteria = criteria.limit(1)?; // Add LIMIT 1 if not present
        }
        let mut results = Self::find_by_criteria(criteria, executor).await?;
        Ok(results.pop()) // Returns None if empty, or the single element.
    }

    /// Deletes records based on the provided criteria.
    /// The implementation of this method is typically generated by the SqlxObject derive macro.
    async fn delete_by_criteria<'e, E>(
        criteria: QueryCriteria,
        executor: E,
    ) -> Result<u64, SqlxError>
    where
        E: Executor<'e, Database = Postgres> + Send,
        Self: Send;
}
