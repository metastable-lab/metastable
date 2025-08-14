use std::sync::Arc;

use metastable_database::init_databases;
use metastable_runtime::define_module_client;
use sqlx::PgPool;

init_databases!(
    default: [
        metastable_runtime::User,
        metastable_runtime::UserUsage,
        metastable_runtime::UserUrl,
        metastable_runtime::UserReferral,
        metastable_runtime::UserBadge,
        metastable_runtime::UserFollow,
        metastable_runtime::SystemConfig,
        metastable_runtime::CardPool,
        metastable_runtime::Card,
        metastable_runtime::DrawHistory,

        metastable_runtime_roleplay::Character,
        metastable_runtime_roleplay::CharacterHistory,
        metastable_runtime_roleplay::CharacterSub,
        metastable_runtime_roleplay::RoleplaySession,
        metastable_runtime_roleplay::RoleplayMessage,
        metastable_runtime_roleplay::AuditLog,

        metastable_runtime_character_creation::CharacterCreationMessage
    ],
    pgvector: [
        metastable_runtime_mem0::EmbeddingMessage
    ]
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
