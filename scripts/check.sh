#!/usr/bin/env bash
# Every gate M0 promises. Fails loudly instead of printing a count nobody reads.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== fmt =="
cargo fmt --check

echo "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace

echo "all gates green"
