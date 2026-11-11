#!/usr/bin/env bash
# Every gate the project promises. Fails loudly instead of printing a count nobody reads.
set -euo pipefail
cd "$(dirname "$0")/.."

# First because it is the cheapest and the one a rename breaks without saying so.
echo "== docs =="
./scripts/check-docs.sh

echo "== fmt =="
cargo fmt --check

echo "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace

echo "all gates green"
