#!/usr/bin/env -S just --justfile
# ^ A shebang isn't required, but allows a justfile to be executed
#   like a script, with `./justfile test`, for example.

set shell := ["zsh", "-cu"]
set dotenv-filename := ".env"
set dotenv-load := true

export SECRET_SALT := env("SECRET_SALT")
export OPENAI_API_KEY := env("OPENAI_API_KEY")
export OPENAI_BASE_URL := env("OPENAI_BASE_URL")
export FISH_AUDIO_API_KEY := env("FISH_AUDIO_API_KEY")
export HASURA_GRAPHQL_URL := env("HASURA_GRAPHQL_URL")
export DATABASE_URL := env("DATABASE_URL")
export HASURA_GRAPHQL_ADMIN_SECRET := env("HASURA_GRAPHQL_ADMIN_SECRET")
export PGVECTOR_URI := env("PGVECTOR_URI")
export OTP_SECRET_KEY := env("OTP_SECRET_KEY")
export MAILEROO_API_KEY := env("MAILEROO_API_KEY")
export STRIPE_SECRET_KEY := env("STRIPE_SECRET_KEY")
export STRIPE_WEBHOOK_SECRET := env("STRIPE_WEBHOOK_SECRET")

export EMBEDDING_API_KEY := env("EMBEDDING_API_KEY")
export EMBEDDING_BASE_URL := env("EMBEDDING_BASE_URL")
export EMBEDDING_EMBEDDING_MODEL := env("EMBEDDING_EMBEDDING_MODEL")



@api:
    cargo run --package metastable-service --bin metastable_service --release

@sandbox-init:
    cargo run --package metastable-sandbox --bin init --release

@sandbox-run:
    cargo run --package metastable-sandbox --bin run --release

@sandbox-migrate:
    cargo run --package metastable-sandbox --bin migrate --release

@sandbox-migrate-messages:
    cargo run --package metastable-sandbox --bin messages --release

@sandbox-text-migration:
    cargo run --package metastable-sandbox --bin text_migration --release

@sandbox-messages-migration:
    cargo run --package metastable-sandbox --bin messages_robust --release

@sandbox-messages-migration-prod:
    cargo run --package metastable-sandbox --bin messages_robust --release production

@test:
    cargo test --package metastable-runtime-roleplay --test sandbox -- test::test_prettier_v0 --exact --show-output
