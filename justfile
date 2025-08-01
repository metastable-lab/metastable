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

export GRAPH_URI := env("GRAPH_URI")
export GRAPH_USER := env("GRAPH_USER")
export GRAPH_PASSWORD := env("GRAPH_PASSWORD")

export EMBEDDING_API_KEY := env("EMBEDDING_API_KEY")
export EMBEDDING_BASE_URL := env("EMBEDDING_BASE_URL")
export EMBEDDING_EMBEDDING_MODEL := env("EMBEDDING_EMBEDDING_MODEL")



@api:
    cargo run --package metastable-service --bin metastable_service --release

@sandbox-init:
    cargo run --package metastable-sandbox --bin init --release

@sandbox-run:
    cargo run --package metastable-sandbox --bin run --release

@sandbox-buy-referral:
    cargo run --package metastable-sandbox --bin buy_referral --release

# @sandbox-reset-db:
#     cargo run --package metastable-sandbox --bin reset_db --release
