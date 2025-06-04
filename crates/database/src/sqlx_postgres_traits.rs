use sqlx::{FromRow, Postgres, Error as SqlxError, postgres::PgArguments, Acquire};
use async_trait::async_trait;

/// Trait to define the schema of a database object for PostgreSQL.
pub trait SqlxSchema {
    /// The type of the primary key for this database object.
    type Id: Send + Sync + Clone + for<'q> sqlx::Encode<'q, Postgres> + sqlx::Type<Postgres>;

    /// The intermediate type that implements FromRow, used for fetching from the database.
    type Row: for<'r> FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin;

    /// The name of the database table.
    const TABLE_NAME: &'static str;
    /// The name of the primary key column.
    const ID_COLUMN_NAME: &'static str;
    /// A list of all column names in the table.
    const COLUMNS: &'static [&'static str];

    /// Gets the primary key column name.
    fn id_column_name() -> &'static str { Self::ID_COLUMN_NAME }
    /// Gets the table name.
    fn table_name() -> &'static str { Self::TABLE_NAME }
    /// Gets all column names.
    fn columns() -> &'static [&'static str] { Self::COLUMNS }

    /// Generates the SQL query string for selecting all records.
    /// Example: "SELECT id, name, email FROM users"
    fn select_all_sql() -> String;
    /// Generates the SQL query string for selecting a record by its primary key.
    /// Example: "SELECT id, name, email FROM users WHERE id = $1"
    fn select_by_id_sql() -> String;
    /// Generates the SQL query string for inserting a new record.
    /// This query should use RETURNING to get the inserted row.
    /// Example: "INSERT INTO users (name, email) VALUES ($1, $2) RETURNING id, name, email"
    fn insert_sql() -> String;
    /// Generates the SQL query string for updating an existing record by its primary key.
    /// This query should use RETURNING to get the updated row.
    /// Example: "UPDATE users SET name = $1, email = $2 WHERE id = $3 RETURNING id, name, email"
    fn update_by_id_sql() -> String;
    /// Generates the SQL query string for deleting a record by its primary key.
    /// Example: "DELETE FROM users WHERE id = $1"
    fn delete_by_id_sql() -> String;

    /// Retrieves the value of the primary key for an instance of the object.
    fn get_id_value(&self) -> Self::Id;

    /// Generates the SQL query string for creating the table.
    /// Example: "CREATE TABLE users (id UUID PRIMARY KEY, name TEXT NOT NULL, email TEXT);"
    fn create_table_sql() -> String;

    /// Generates the SQL query string for dropping the table.
    fn drop_table_sql() -> String;

    /// Converts the intermediate Row type to the Self type.
    fn from_row(row: Self::Row) -> Self;
}

/// Trait for CRUD (Create, Read, Update, Delete) operations for PostgreSQL.
#[async_trait]
pub trait SqlxCrud: SqlxSchema + Sized + Send + Sync + Unpin + Clone {
    /// Binds the struct fields to an insert query.
    /// This method is typically implemented by the derive macro.
    fn bind_insert<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Binds the struct fields to an update query.
    /// The ID is typically bound last for the WHERE clause.
    /// This method is typically implemented by the derive macro.
    fn bind_update<'q>(&self, query: sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>)
        -> sqlx::query::QueryAs<'q, Postgres, Self::Row, PgArguments>;

    /// Creates a new record in the database.
    async fn create<'e, A>(self, acquirer: A) -> Result<Self, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::insert_sql();
        let query_with_bindings = self.bind_insert(sqlx::query_as(&sql));
        query_with_bindings.fetch_one(&mut *conn).await.map(Self::from_row)
    }

    /// Finds a record by its primary key.
    async fn find_by_id<'e, A>(id: Self::Id, acquirer: A) -> Result<Option<Self>, SqlxError>
    where
        A: Acquire<'e, Database = Postgres> + Send,
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
    {
        let mut conn = acquirer.acquire().await?;
        let sql = Self::select_all_sql();
        let rows = sqlx::query_as(&sql)
            .fetch_all(&mut *conn)
            .await?;
        Ok(rows.into_iter().map(Self::from_row).collect())
    }
} 