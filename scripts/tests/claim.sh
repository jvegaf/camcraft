#!/usr/bin/env bash
# The claim is by the file surfaces a task declares, and it says who holds what when they collide.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"
build_repo

echo "== a worktree is required, and the primary tree is refused =="
expect_fail "claim from the primary tree is refused" \
  run_in "$MAIN" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 100
expect_contains "the refusal names the worktree rule" "$WORK/err" "primary tree"

echo "== surfaces, not crates =="
INDEX_BEFORE=$(git -C "$MAIN" hash-object "$MAIN/odd/tasks/INDEX.md")
expect_ok "claim 100 from its worktree" \
  run_in "$WORK/wt100" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 100
expect_contains "the claim records the declared surface" "$MAIN/odd/tasks/.claims/100.json" \
  "crates/cam-geom/src/boolean.rs"
expect_contains "the claim records the second surface" "$MAIN/odd/tasks/.claims/100.json" \
  "fixtures/corpus"
expect_contains "the claim says it was made by surfaces" "$MAIN/odd/tasks/.claims/100.json" "surfaces"
INDEX_AFTER=$(git -C "$MAIN" hash-object "$MAIN/odd/tasks/INDEX.md")
if [ "$INDEX_BEFORE" = "$INDEX_AFTER" ]; then
  pass "claiming does not write the shared index"
else
  fail "claiming wrote the shared index; the index belongs to the merge"
fi

expect_fail "a task whose surface overlaps is refused" \
  run_in "$WORK/wt101" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 101
expect_contains "the collision names the surface" "$WORK/err" "crates/cam-geom/src/boolean.rs"
expect_contains "the collision names the owner" "$WORK/err" "100"
expect_fail "the refused task left no claim behind" test -f "$MAIN/odd/tasks/.claims/101.json"

expect_ok "a disjoint task claims fine" \
  run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102

echo "== documents without a surfaces block fall back to crates =="
expect_ok "legacy task claims by crate token" \
  run_in "$WORK/wt103" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 103
expect_contains "the legacy claim records the crate" "$MAIN/odd/tasks/.claims/103.json" "cam-excellon"
expect_contains "the legacy claim warns about the missing block" "$WORK/err" "Surfaces"
expect_fail "the second legacy task collides on the same crate" \
  run_in "$WORK/wt104" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 104
expect_contains "the crate collision names it" "$WORK/err" "crates/cam-excellon"

summary
