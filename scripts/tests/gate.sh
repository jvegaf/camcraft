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

# A fake harness, planted only where a case needs it. The real one would run the whole suite from
# inside the suite; the fixture's own copy list leaves it out, which is what keeps this terminating.
plant_tooling_stub() {
  local dir=$1
  cat >"$dir/scripts/test-worktree-tooling.sh" <<'STUB'
#!/usr/bin/env bash
echo "stub tooling harness ran"
: >"${TOOLING_STUB_MARK:-tooling-stub-ran}"
exit "${TOOLING_STUB_EXIT:-0}"
STUB
  chmod +x "$dir/scripts/test-worktree-tooling.sh"
}

# The real harness exports the guard, so a case that wants the step to run clears the inherited one.
echo "== tooling: the gate runs the harness when it exists =="
plant_tooling_stub "$WORK/wt102"
expect_ok "the gate runs the harness" \
  run_in "$WORK/wt102" env -u GENTLE_TOOLING_TESTS TOOLING_STUB_MARK="$WORK/tooling-ran" ./scripts/check.sh
expect_contains "the gate prints the tooling step" "$WORK/out" "== tooling =="
expect_contains "the harness actually ran" "$WORK/out" "stub tooling harness ran"

echo "== tooling: the guard skips the step =="
expect_ok "the gate skips the harness under the guard" \
  run_in "$WORK/wt102" env GENTLE_TOOLING_TESTS=0 TOOLING_STUB_MARK="$WORK/tooling-skipped" ./scripts/check.sh
expect_contains "the skip names its reason" "$WORK/out" "skip: GENTLE_TOOLING_TESTS=0"
expect_absent "the guarded harness did not run" "$WORK/tooling-skipped"

echo "== tooling: a failing harness fails the gate =="
expect_fail "the gate fails when the harness fails" \
  run_in "$WORK/wt102" env -u GENTLE_TOOLING_TESTS TOOLING_STUB_EXIT=1 ./scripts/check.sh
expect_contains "the failing harness was the one that ran" "$WORK/out" "stub tooling harness ran"

summary
