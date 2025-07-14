use sqlx::{FromRow, Postgres, Error as SqlxError, postgres::PgArguments, Executor};

/// Trait to define the schema of a database object for PostgreSQL.
// No async_trait needed here as no methods are async by default in the trait itself.
pub trait SqlxSchema: Send + Sync + Unpin + Clone + std::fmt::Debug {
    /// The type of the primary key for this database object.
    type Id: Send + Sync + for<'q> sqlx::Encode<'q, Postgres> + sqlx::Type<Postgres> + Clone;

    /// The intermediate type that implements FromRow, used for fetching from the database.
    type Row: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin;

    const TABLE_NAME: &'static str;
    const ID_COLUMN_NAME: &'static str;
    const COLUMNS: &'static [&'static str];
    const INDEXES_SQL: &'static [&'static str];

    // Default utility methods to access consts 
    fn id_column_name() -> &'static str { Self::ID_COLUMN_NAME }
    fn table_name() -> &'static str { Self::TABLE_NAME }
    fn columns() -> &'static [&'static str] { Self::COLUMNS }
    fn indexes_sql() -> &'static [&'static str] { Self::INDEXES_SQL }

    /// Retrieves the value of the primary key for an instance of the object.
    fn get_id_value(&self) -> Self::Id;

    /// Converts the intermediate Row type to the Self type.
    fn from_row(row: Self::Row) -> Self;

    // SQL generation methods (to be implemented by the derive macro)
    fn create_table_sql() -> String;
    fn drop_table_sql() -> String;
    fn insert_sql() -> String;
    fn trigger_sql() -> String;
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
    pub find_similarity: Option<(pgvector::Vector, &'static str)>,
    pub similarity_threshold: Option<f32>,
}

impl QueryCriteria {

    /// Creates a new `QueryCriteria` builder.
    /// 
    /// **IMPORTANT**: When building a query, methods that add arguments must be called in the
    /// same order that the final SQL query expects its placeholders (`$1`, `$2`, etc.).
    /// The conventional order is:
    /// 1. `find_similarity()`
    /// 2. `with_similarity_threshold()`
    /// 3. `add_filter()` / `add_valued_filter()`
    /// 4. `order_by()` (does not add arguments)
    /// 5. `limit()`
    /// 6. `offset()`
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            arguments: ::sqlx::postgres::PgArguments::default(),
            has_limit: false,
            has_offset: false,
            order_by: Vec::new(),
            find_similarity: None,
            similarity_threshold: None,
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

    pub fn find_similarity(mut self, vector: pgvector::Vector, as_field: &'static str) -> Result<Self, SqlxError> {
        use ::sqlx::Arguments;
        self.arguments.add(vector.clone()).map_err(SqlxError::Encode)?;
        self.find_similarity = Some((vector, as_field));
        Ok(self)
    }

    pub fn with_similarity_threshold(mut self, threshold: f32) -> Result<Self, SqlxError> {
        use ::sqlx::Arguments;
        self.arguments.add(threshold).map_err(SqlxError::Encode)?;
        self.similarity_threshold = Some(threshold);
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
