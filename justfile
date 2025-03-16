#!/usr/bin/env -S just --justfile
# ^ A shebang isn't required, but allows a justfile to be executed
#   like a script, with `./justfile test`, for example.

set shell := ["zsh", "-cu"]
set dotenv-filename := ".env"
set dotenv-load := true

export ETH_RPC_URL := env("ETH_RPC_URL")
export MONGODB_URI := env("MONGODB_URI")
export GITCOIN_PRIVATE_KEY_SALT := env("GITCOIN_PRIVATE_KEY_SALT")
export SECRET_SALT := env("SECRET_SALT")
export OPENAI_API_KEY := env("OPENAI_API_KEY")
export OPENAI_BASE_URL := env("OPENAI_BASE_URL")

@docs:
    cd docs && bunx mintlify dev

@evm:
    cargo run --package voda-runtime-evm --bin takara_lend

@api:
    cargo run --package voda-service-api --bin server
