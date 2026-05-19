#! /usr/bin/env -S just --justfile

cargo-check:
    RUSTFLAGS="-Dwarnings" cargo check

cargo-fix:
    cargo clippy fix --allow-dirty

fmt:
    cargo fmt
    taplo fmt

fmt-check:
    cargo fmt --check

taplo-check:
    taplo fmt --check

test:
    cargo nextest run