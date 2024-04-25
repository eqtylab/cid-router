_:
    @just --list

# Run main CI job
ci:
    just readme-check
    cargo fmt --all -- --check
    cargo clippy
    cargo build --release --locked --workspace
    cargo test --workspace

# Update auto-generated portions of README.md
readme-update:
    .readme/update.sh README.md

# Check auto-generated portions of README.md
readme-check: _tmp
    cp README.md tmp/README.md
    .readme/update.sh tmp/README.md
    diff README.md tmp/README.md

_tmp:
    mkdir -p tmp
