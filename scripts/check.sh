#!/usr/bin/env bash
# Every gate the project promises. Fails loudly instead of printing a count nobody reads.
set -euo pipefail
cd "$(dirname "$0")/.."

# First: the cheapest, and the one that stops an agent from working unclaimed in a worktree — or from
# working in the primary tree at all.
echo "== claim =="
./scripts/check-worktree.sh --gate

# Next because it is the cheapest and the one a rename breaks without saying so.
echo "== docs =="
./scripts/check-docs.sh

# The records come before the code: a task nobody wrote down is the one failure no test can catch.
echo "== records =="
./scripts/check-records.sh

echo "== fmt =="
cargo fmt --check

echo "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace

# The parity harness is deliberately outside the workspace (its cost stays out of `cargo test
# --workspace`), which means nothing above compiles it: `image()` and `tessellate()` changed their
# tolerance type and the only job that noticed was the one in CI. Building it here costs seconds and
# moves that discovery to the machine that made the change.
echo "== parity harness =="
# `--locked` and a stable target directory under `target/`: the measurer pins its own lock file, and
# a build artifact dropped inside `scripts/` would dirty the worktree the claim gate reads. Reusing
# the directory lets local hooks and checks avoid rebuilding it from scratch.
CARGO_TARGET_DIR="$PWD/target/parity-measure-check" \
  cargo build --quiet --locked --manifest-path scripts/parity/measure/Cargo.toml

echo "all gates green"
