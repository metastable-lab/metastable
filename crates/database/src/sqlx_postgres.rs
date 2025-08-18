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

// --- Filtering Structures and Trait ---

/// A trait to allow for boxing of different types that can be encoded as sqlx arguments.
/// This is a helper for the `QueryCriteria` struct to store argument values of different types.
pub trait AsSqlxArg: Send + Sync {
    fn add_to_args<'q>(&self, args: &mut PgArguments) -> Result<(), SqlxError>;
}

/// A blanket implementation of AsSqlxArg for any type that meets the bounds.
/// This allows us to store any value that can be encoded for Postgres.
impl<T> AsSqlxArg for T
where
    T: for<'a> sqlx::Encode<'a, Postgres> + sqlx::Type<Postgres> + Send + Sync + Clone + 'static,
{
    fn add_to_args<'q>(&self, args: &mut PgArguments) -> Result<(), SqlxError> {
        use sqlx::Arguments;
        args.add(self.clone()).map_err(SqlxError::Encode)
    }
}

/// Represents a single filter condition for a database query.
pub struct FilterCondition {
    pub column: &'static str,
    pub operator: &'static str,
    /// Holds the value for the condition's placeholder, if any.
    pub value: Option<Box<dyn AsSqlxArg>>,
}

/// Holds parameters for a vector similarity search.
pub struct SimilaritySearch {
    pub vector: pgvector::Vector,
    pub as_field: &'static str,
    pub threshold: Option<f32>,
}

/// Represents the complete criteria for a filtered database query.
/// This struct holds all the components needed to build a dynamic SQL query.
/// The `SqlxObject` derive macro is responsible for interpreting these components
/// and constructing the final SQL and arguments list.
#[derive(Default)]
pub struct QueryCriteria {
    pub conditions: Vec<FilterCondition>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub order_by: Vec<(&'static str, OrderDirection)>,
    pub similarity_search: Option<SimilaritySearch>,
}

impl QueryCriteria {
    /// Creates a new, empty `QueryCriteria` builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a filter condition that may or may not have a value.
    pub fn add_filter<V>(mut self, column: &'static str, operator: &'static str, value: Option<V>) -> Self
    where
        V: for<'a> ::sqlx::Encode<'a, Postgres> + ::sqlx::Type<Postgres> + Send + Sync + Clone + 'static,
    {
        self.conditions.push(FilterCondition {
            column,
            operator,
            value: value.map(|v| Box::new(v) as Box<dyn AsSqlxArg>),
        });
        self
    }

    /// A convenience method for `add_filter` that requires a value.
    pub fn add_valued_filter<V>(self, column: &'static str, operator: &'static str, value: V) -> Self
    where
        V: for<'a> ::sqlx::Encode<'a, Postgres> + ::sqlx::Type<Postgres> + Send + Sync + Clone + 'static,
    {
        self.add_filter(column, operator, Some(value))
    }
    
    /// Sets the LIMIT for the query.
    pub fn limit(mut self, limit_val: i64) -> Self {
        self.limit = Some(limit_val);
        self
    }

    /// Sets the OFFSET for the query.
    pub fn offset(mut self, offset_val: i64) -> Self {
        self.offset = Some(offset_val);
        self
    }

    /// Adds an ORDER BY clause.
    pub fn order_by(mut self, column: &'static str, direction: OrderDirection) -> Self {
        self.order_by.push((column, direction));
        self
    }

    /// Configures a vector similarity search.
    pub fn find_similarity(mut self, vector: pgvector::Vector, as_field: &'static str) -> Self {
        self.similarity_search = Some(SimilaritySearch {
            vector,
            as_field,
            threshold: None,
        });
        self
    }

    /// Sets the similarity threshold for a vector search.
    /// This is only effective if `find_similarity` has also been called.
    pub fn with_similarity_threshold(mut self, threshold: f32) -> Self {
        if let Some(ss) = &mut self.similarity_search {
            ss.threshold = Some(threshold);
        }
        self
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
        if criteria.limit.is_none() {
            criteria = criteria.limit(1);
        };
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

/// Trait for enums that can be rendered/parsing into localized text forms for prompts and storage.
/// Implementations typically come from a derive macro.
pub trait TextPromptCodec: Sized {
    /// Format for display, using the storage language.
    fn to_lang(&self, lang: &str) -> String;
    /// Parse from any supported language representation.
    fn parse_any_lang(s: &str) -> anyhow::Result<Self>;
    fn parse_with_type_and_content(type_str: &str, content_str: &str) -> anyhow::Result<Self>;
    fn schema(lang: Option<&str>) -> serde_json::Value;
}

#[async_trait::async_trait]
pub trait SchemaMigrator {
    /// Compares the struct's schema with the database and applies necessary changes.
    async fn migrate(pool: &sqlx::PgPool) -> anyhow::Result<()>;
}
