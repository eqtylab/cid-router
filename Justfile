_:
    @just --list

# Run main CI job
ci:
    cargo fmt --all -- --check
    cargo sort --workspace --grouped --check
    cargo build --locked --workspace
    cargo clippy
    cargo test --workspace
    just readme-check

# Update auto-generated portions of README.md
readme-update:
    .readme/update.sh README.md

# Check auto-generated portions of README.md
readme-check: _tmp
    cp README.md tmp/README.md
    .readme/update.sh tmp/README.md
    diff README.md tmp/README.md

# Creates openapi rust client
openapi:
   cargo run --bin cid-router openapi
   ./scripts/generate_openapi_clients.sh cid-router.json
   rm cid-router.json

_tmp:
    mkdir -p tmp
