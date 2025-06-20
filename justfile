#!/usr/bin/env -S just --justfile
# ^ A shebang isn't required, but allows a justfile to be executed
#   like a script, with `./justfile test`, for example.

set shell := ["zsh", "-cu"]
set dotenv-filename := ".env"
set dotenv-load := true

# export ETH_RPC_URL := env("ETH_RPC_URL")
# export MONGODB_URI := env("MONGODB_URI")
# export GITCOIN_PRIVATE_KEY_SALT := env("GITCOIN_PRIVATE_KEY_SALT")

export SECRET_SALT := env("SECRET_SALT")
export OPENAI_API_KEY := env("OPENAI_API_KEY")
export OPENAI_BASE_URL := env("OPENAI_BASE_URL")
export FISH_AUDIO_API_KEY := env("FISH_AUDIO_API_KEY")
export HASURA_GRAPHQL_URL := env("HASURA_GRAPHQL_URL")
export DATABASE_URL := env("DATABASE_URL")
export HASURA_GRAPHQL_ADMIN_SECRET := env("HASURA_GRAPHQL_ADMIN_SECRET")

@docs:
    cd docs && bunx mintlify dev

@api:
    cargo run --package voda-service --bin voda_service --release

@sandbox-init:
    cargo run --package voda-sandbox --bin init --release

@sandbox-run:
    cargo run --package voda-sandbox --bin run --release

@sandbox-buy-referral:
    cargo run --package voda-sandbox --bin buy_referral --release

# @sandbox-reset-db:
#     cargo run --package voda-sandbox --bin reset_db --release