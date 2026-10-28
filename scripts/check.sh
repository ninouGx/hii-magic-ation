#!/usr/bin/env bash
# Local CI. The Rust tests need web/dist, so the front builds first.
set -euo pipefail
cd "$(dirname "$0")/.."

(cd web && npm ci --silent && npm run lint && npm test && npm run build)
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
