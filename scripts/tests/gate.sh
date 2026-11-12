#!/usr/bin/env bash
# The gate answers "may this tree hold work": a linked worktree needs a claim, and the primary tree
# must not be carrying the work itself.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"
build_repo

echo "== check-worktree: one claim per worktree =="
run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102 >/dev/null 2>&1
expect_ok "a claimed worktree passes" run_in "$WORK/wt102" ./scripts/check-worktree.sh
expect_fail "the primary tree is refused" run_in "$MAIN" ./scripts/check-worktree.sh
expect_fail "an unclaimed worktree is refused" run_in "$WORK/wt101" ./scripts/check-worktree.sh
expect_contains "the refusal says how to claim" "$WORK/err" "claim"

echo "== --gate: usable inside check.sh =="
expect_ok "--gate passes on a clean primary tree" run_in "$MAIN" ./scripts/check-worktree.sh --gate
expect_fail "--gate still refuses the plain call's primary-tree answer" \
  run_in "$MAIN" ./scripts/check-worktree.sh
expect_ok "--gate passes in a claimed worktree" run_in "$WORK/wt102" ./scripts/check-worktree.sh --gate
expect_fail "--gate fails in an unclaimed worktree" run_in "$WORK/wt101" ./scripts/check-worktree.sh --gate

echo "== the gate runs the claim check =="
export CARGO_STUB_LOG="$WORK/cargo.log"
expect_fail "the gate fails in an unclaimed worktree" run_in "$WORK/wt101" ./scripts/check.sh
expect_contains "the gate's refusal is the claim check" "$WORK/err" "claim"
expect_ok "the gate passes in a claimed worktree" run_in "$WORK/wt102" ./scripts/check.sh
expect_ok "the gate passes on a clean primary tree" run_in "$MAIN" ./scripts/check.sh

mkdir -p "$MAIN/crates"
printf 'fn a() {}\n' >"$MAIN/crates/probe.rs"
expect_fail "the gate fails on a primary tree holding a new source file" run_in "$MAIN" ./scripts/check.sh
expect_contains "the refusal names the primary tree" "$WORK/err" "primary tree"
rm -rf "$MAIN/crates"

printf '\n# uncommitted edit\n' >>"$MAIN/justfile"
expect_fail "the gate fails on a primary tree holding a justfile edit" run_in "$MAIN" ./scripts/check.sh
expect_contains "the justfile edit is named" "$WORK/err" "justfile"
git -C "$MAIN" checkout -q -- justfile

git -C "$MAIN" mv odd/tasks/102-gamma.md odd/tasks/102-gamma-renamed.md
expect_fail "the gate fails on a primary tree holding a staged rename" run_in "$MAIN" ./scripts/check.sh
expect_contains "the rename is reported" "$WORK/err" "102-gamma"
git -C "$MAIN" mv odd/tasks/102-gamma-renamed.md odd/tasks/102-gamma.md
expect_ok "the gate passes again once the tree is clean" run_in "$MAIN" ./scripts/check.sh

summary
