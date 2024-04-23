_:
    @just --list

ci:
    cargo fmt --all -- --check
    cargo clippy
    cargo build --release --locked --workspace
    cargo test --workspace
