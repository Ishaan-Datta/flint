#! /usr/bin/env -S just --justfile

default:
    just --list

check: cargo-check fmt-check taplo-check

# Check for clippy lint errors
cargo-check:
    RUSTFLAGS="-Dwarnings" cargo check

# Fix lint errors with clippy
cargo-fix:
    cargo clippy fix --allow-dirty

# Format all .toml and .rs files
fmt:
    cargo fmt
    taplo fmt

# Verify formatting for .rs files
fmt-check:
    cargo fmt --check

# Verify formatting for .toml files
taplo-check:
    taplo fmt --check

# Shortcut for testing with cargo nextest
test *args:
    cargo nextest run {{ args }}

# Shortcut for running flint on an example flake.nix
[working-directory('flint/tests/common')]
local-test *args:
    cargo run -- {{ args }}