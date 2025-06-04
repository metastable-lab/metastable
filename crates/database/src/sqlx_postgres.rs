use sqlx::{FromRow, Postgres, Error as SqlxError, postgres::PgArguments, Acquire, PgPool};
use async_trait::async_trait;

/// Trait for custom primary key population logic for SqlxObject.
pub trait SqlxPopulateId {
    /// Populates the primary key field (`id`) of the struct.
    fn sql_populate_id(&mut self);
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

    // populate_id() is inherited from SqlxPopulateId supertrait.

    /// Converts the intermediate Row type to the Self type.
    fn from_row(row: Self::Row) -> Self;

    // SQL generation methods (to be implemented by the derive macro)
    fn create_table_sql() -> String;
    fn drop_table_sql() -> String;
    fn select_all_sql() -> String;
    fn select_by_id_sql() -> String;
    fn insert_sql() -> String;
    fn update_by_id_sql() -> String;
    fn delete_by_id_sql() -> String;
}

/// Trait for CRUD (Create, Read, Update, Delete) operations for PostgreSQL.
#[async_trait]
pub trait SqlxCrud: SqlxSchema + Sized { // Removed redundant Send + Sync + Unpin + Clone, implied by SqlxSchema or Sized context
    /// Binds the struct fields to an insert query.
    fn bind_insert<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Binds the struct fields to an update query.
    fn bind_update<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Creates a new record in the database.
    async fn create<'e, A>(mut self, acquirer: A) -> Result<Self, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
        Self: Send // ensure Self is Send for async operations
    {
        self.sql_populate_id(); // Call the method from SqlxPopulateId
        let mut conn = acquirer.acquire().await?;
        let sql = Self::insert_sql();
        let query_with_bindings = self.bind_insert(sqlx::query_as(&sql));
        query_with_bindings.fetch_one(&mut *conn).await.map(Self::from_row)
    }

    /// Finds a record by its primary key.
    async fn find_by_id<'e, A>(id: Self::Id, acquirer: A) -> Result<Option<Self>, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
        Self: Send // ensure Self is Send
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::select_by_id_sql();
        sqlx::query_as(&sql)
            .bind(id)
            .fetch_optional(&mut *conn)
            .await
            .map(|opt_row| opt_row.map(Self::from_row))
    }

    /// Updates an existing record in the database.
    async fn update<'e, A>(self, acquirer: A) -> Result<Self, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
        Self: Send
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::update_by_id_sql();
        let query_with_bindings = self.bind_update(sqlx::query_as(&sql));
        query_with_bindings.fetch_one(&mut *conn).await.map(Self::from_row)
    }

    /// Deletes a record from the database by its primary key.
    async fn delete<'e, A>(self, acquirer: A) -> Result<u64, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
        Self: Send
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::delete_by_id_sql();
        sqlx::query(&sql)
            .bind(self.get_id_value())
            .execute(&mut *conn)
            .await
            .map(|result| result.rows_affected())
    }
    
    /// Retrieves all records from the table.
    async fn find_all<'e, A>(acquirer: A) -> Result<Vec<Self>, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
        Self: Send 
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::select_all_sql();
        let rows_intermediate = sqlx::query_as(&sql) // Changed variable name to avoid conflict if Self::Row is Vec
            .fetch_all(&mut *conn)
            .await?;
        Ok(rows_intermediate.into_iter().map(Self::from_row).collect())
    }
} 