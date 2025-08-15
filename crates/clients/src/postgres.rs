use std::sync::Arc;

use metastable_database::init_databases;
use metastable_common::define_module_client;
use sqlx::PgPool;

init_databases!(
    default: [ ],
    pgvector: [ ]
);

define_module_client! {
    (struct PostgresClient, "postgres")
    client_type: Arc<&'static PgPool>,
    env: ["DATABASE_URL"],
    setup: async {
        Arc::new(connect(false, false, false).await)
    }
}

define_module_client! {
    (struct PgvectorClient, "pgvector")
    client_type: Arc<&'static PgPool>,
    env: ["PGVECTOR_URI"],
    setup: async {
        Arc::new(connect_pgvector(false, false, false).await)
    }
}
