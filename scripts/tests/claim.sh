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

echo "== a glob claims the prefix it can match =="
expect_fail "a glob surface collides with the file it matches" \
  run_in "$WORK/wt105" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 105
expect_contains "the glob collision names the claimed prefix" "$WORK/err" "crates/cam-geom/src"
expect_fail "a glob with no literal prefix collides with everything" \
  run_in "$WORK/wt107" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 107
expect_contains "the root-glob collision is named" "$WORK/err" "Surface collision"
expect_fail "a ./ surface collides with the same path" \
  run_in "$WORK/wt106" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 106
expect_contains "the ./ collision names the surface" "$WORK/err" "fixtures/corpus"

echo "== an older claim is normalized before it is compared =="
printf '{"task": 198, "surfaces": ["./crates/cam-gerber/src"], "crates": []}\n' \
  >"$MAIN/odd/tasks/.claims/198.json"
expect_fail "a raw ./ surface in an old claim still collides" \
  run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102
expect_contains "the old claim is named" "$WORK/err" "198.json"
rm -f "$MAIN/odd/tasks/.claims/198.json"
printf '{"task": 197, "surfaces": ["crates/cam-gerber/"], "crates": []}\n' \
  >"$MAIN/odd/tasks/.claims/197.json"
expect_fail "an old surface with a trailing slash still collides" \
  run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102
expect_contains "the trailing-slash claim is named" "$WORK/err" "197.json"
rm -f "$MAIN/odd/tasks/.claims/197.json"

echo "== an unreadable claim is a refusal, not an absence =="
printf 'not json at all\n' >"$MAIN/odd/tasks/.claims/199.json"
expect_fail "claiming refuses while a claim cannot be read" \
  run_in "$WORK/wt102" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 102
expect_contains "the refusal names the unreadable claim" "$WORK/err" "199.json"
rm -f "$MAIN/odd/tasks/.claims/199.json"

echo "== the tree is resolved, not the shell's directory =="
mkdir -p "$WORK/wt102/scripts/nested"
expect_ok "claiming works from a subdirectory of the worktree" \
  run_in "$WORK/wt102/scripts/nested" env PI_AGENT_ID=t PI_SESSION_ID=t \
  "$WORK/wt102/scripts/claim-task.sh" 102
rm -rf "$WORK/wt102/scripts/nested"
expect_contains "the claim records the enforced prefix" "$MAIN/odd/tasks/.claims/102.json" \
  "crates/cam-gerber"
expect_absent "no staging file is left behind" \
  "$(ls "$MAIN"/odd/tasks/.claims/.claim.* 2>/dev/null | head -1)"
MODE=$(stat -c %a "$MAIN/odd/tasks/.claims/102.json")
if [ "$MODE" = "600" ]; then
  pass "the claim is private to its owner"
else
  fail "the claim is mode $MODE; it carries a session id and belongs to its owner"
fi

echo "== documents without a surfaces block fall back to crates =="
expect_ok "legacy task claims by crate token" \
  run_in "$WORK/wt103" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 103
expect_contains "the legacy claim records the crate" "$MAIN/odd/tasks/.claims/103.json" "cam-excellon"
expect_contains "the legacy claim warns about the missing block" "$WORK/err" "Surfaces"
expect_fail "the second legacy task collides on the same crate" \
  run_in "$WORK/wt104" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 104
expect_contains "the crate collision names it" "$WORK/err" "crates/cam-excellon"

summary
