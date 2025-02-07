#!/usr/bin/env -S just --justfile
# ^ A shebang isn't required, but allows a justfile to be executed
#   like a script, with `./justfile test`, for example.

set shell := ["zsh", "-cu"]
set dotenv-filename := ".env"
set dotenv-load := true

@docs:
    cd docs && bunx mintlify dev