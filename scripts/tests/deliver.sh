#!/usr/bin/env bash
# Delivering a task is the merge step: the shared index gets its row, and the claim goes away.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"
build_repo

if [ ! -f "$MAIN/scripts/deliver-task.sh" ]; then
  echo "skip  deliver-task.sh is not present"
  exit 0
fi

run_in "$WORK/wt100" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 100 >/dev/null 2>&1

echo "== deliver belongs to the primary tree =="
expect_fail "deliver is refused from a linked worktree" \
  run_in "$WORK/wt100" ./scripts/deliver-task.sh 100 abc1234
expect_contains "the refusal names the worktree" "$WORK/err" "primary tree"
expect_contains "the claim survives the refusal" "$MAIN/odd/tasks/.claims/100.json" "crates/cam-geom/src/boolean.rs"

echo "== deliver writes the index =="
expect_fail "deliver refuses a task with no index row" run_in "$MAIN" ./scripts/deliver-task.sh 999
expect_ok "deliver 100 with its commit" run_in "$MAIN" ./scripts/deliver-task.sh 100 abc1234
expect_contains "the row is delivered" "$MAIN/odd/tasks/INDEX.md" \
  "| 100 | [alpha](100-alpha.md) | delivered |"
expect_contains "the row records the commit" "$MAIN/odd/tasks/INDEX.md" '`abc1234`'
expect_absent "the claim is released" "$MAIN/odd/tasks/.claims/100.json"

echo "== deliver without a commit still delivers =="
run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102 >/dev/null 2>&1
expect_ok "deliver 102 without a commit" run_in "$MAIN" ./scripts/deliver-task.sh 102
expect_contains "the row is delivered with no commit" "$MAIN/odd/tasks/INDEX.md" \
  "| 102 | [gamma](102-gamma.md) | delivered |"
expect_absent "the second claim is released" "$MAIN/odd/tasks/.claims/102.json"

expect_absent "the index rewrite leaves no staging file" \
  "$(ls "$MAIN/odd/tasks/."INDEX.md.* 2>/dev/null | head -1)"

echo "== deliver leaves the other rows alone =="
expect_contains "an untouched row keeps its state" "$MAIN/odd/tasks/INDEX.md" \
  "| 103 | [legacy](103-legacy.md) | in progress |"

summary
