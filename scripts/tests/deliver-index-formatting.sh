#!/usr/bin/env bash
# A delivery writes the index row in the table's own shape: padded on a Biome-formatted index, single
# spaced on the hand-written fixture, and widened when the value does not fit its column.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"

# The delivery's record check is not what this file judges, and the fixture has no Engram store: the
# stub keeps everything else `deliver-task.sh` does — the refusals, the atomic write, the claim
# release — real.
stub_records() {
  printf '#!/usr/bin/env bash\nexit 0\n' >"$MAIN/scripts/check-records.sh"
  chmod +x "$MAIN/scripts/check-records.sh"
}

# Every line but the delivered row has to come back byte for byte.
expect_only_row_changed() {
  local label=$1 index=$2 old=$3 row=$4
  if python3 - "$old" "$index" "$row" >"$WORK/out" 2>"$WORK/err" <<'PY'
import sys

old = open(sys.argv[1], encoding="utf-8").read().splitlines()
new = open(sys.argv[2], encoding="utf-8").read().splitlines()
row = sys.argv[3]
if len(old) != len(new):
    sys.exit(f"line counts differ: {len(old)} -> {len(new)}")
changed = [(a, b) for a, b in zip(old, new) if a != b]
if len(changed) != 1:
    sys.exit(f"{len(changed)} lines changed, expected only the delivered row")
before, after = changed[0]
if not before.startswith(row) or not after.startswith(row):
    sys.exit(f"a line other than {row} changed:\n  before: {before}\n  after:  {after}")
PY
  then pass "$label"; else
    fail "$label ($(cat "$WORK/err"))"
  fi
}

expect_unchanged() {
  local label=$1 index=$2 snapshot=$3
  if cmp -s "$snapshot" "$index"; then pass "$label"; else
    fail "$label (the index changed)"
  fi
}

build_repo
stub_records

if [ ! -f "$MAIN/scripts/deliver-task.sh" ]; then
  echo "skip  deliver-task.sh is not present"
  exit 0
fi

INDEX="$MAIN/odd/tasks/INDEX.md"
HEAD_SHORT=$(git -C "$MAIN" rev-parse --short HEAD)

echo "== the hand-written fixture keeps its single-space cells =="
run_in "$WORK/wt100" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 100 >/dev/null 2>&1
expect_ok "deliver 100 on the hand-written index" \
  run_in "$MAIN" ./scripts/deliver-task.sh 100 "$HEAD_SHORT"
expect_contains "the row keeps one space around each cell" "$INDEX" \
  "| 100 | [alpha](100-alpha.md) | delivered | 2026-11-12 | \`$HEAD_SHORT\` |"

echo "== a formatter-formatted index keeps its padding =="
format_index "$INDEX"
cp "$INDEX" "$WORK/index.formatted"
run_in "$WORK/wt101" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 101 >/dev/null 2>&1
expect_ok "deliver 101 on the formatted index" \
  run_in "$MAIN" ./scripts/deliver-task.sh 101 "$HEAD_SHORT"
expect_contains "the delivered row is padded to the column widths" "$INDEX" \
  "| 101 | [beta](101-beta.md)                 | delivered   | 2026-11-12 | \`$HEAD_SHORT\`     |"
expect_only_row_changed "the rest of the table is untouched" "$INDEX" "$WORK/index.formatted" "| 101 |"

echo "== a refused delivery still writes nothing =="
cp "$INDEX" "$WORK/index.before-refusals"
expect_fail "deliver refuses a third argument" \
  run_in "$MAIN" ./scripts/deliver-task.sh 102 "$HEAD_SHORT" extra
expect_unchanged "the refused third argument wrote nothing" "$INDEX" "$WORK/index.before-refusals"
expect_fail "deliver refuses a commit that does not exist" \
  run_in "$MAIN" ./scripts/deliver-task.sh 102 deadbee
expect_unchanged "the refused commit wrote nothing" "$INDEX" "$WORK/index.before-refusals"

echo "== a value wider than its column widens the column =="
build_repo 2
stub_records
NARROW="$MAIN/odd/tasks/INDEX.md"
cat >"$NARROW" <<'MD'
# Índice de tareas

| # | tarea | estado | desde | último commit |
| --- | --- | --- | --- | --- |
| 100 | [alpha](100-alpha.md) | wip | 2026-11-12 | — |
| 101 | [beta](101-beta.md) | wip | 2026-11-12 | — |
| 102 | [gamma](102-gamma.md) | wip | 2026-11-12 | — |
MD
format_index "$NARROW"
NARROW_SHORT=$(git -C "$MAIN" rev-parse --short HEAD)
run_in "$WORK/wt1012" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 101 >/dev/null 2>&1
expect_ok "deliver 101 with a status wider than its column" \
  run_in "$MAIN" ./scripts/deliver-task.sh 101 "$NARROW_SHORT"
expect_contains "the separator row is widened to match" "$NARROW" \
  "| --- | --------------------- | --------- | ---------- | ------------- |"
expect_contains "the header row is re-padded to the same width" "$NARROW" \
  "| #   | tarea                 | estado    | desde      | último commit |"
expect_contains "the delivered row fills the widened column" "$NARROW" \
  "| 101 | [beta](101-beta.md)   | delivered | 2026-11-12 | \`$NARROW_SHORT\`     |"
expect_contains "the row above is re-padded to the same width" "$NARROW" \
  "| 100 | [alpha](100-alpha.md) | wip       | 2026-11-12 | —             |"
expect_contains "the row below is re-padded to the same width" "$NARROW" \
  "| 102 | [gamma](102-gamma.md) | wip       | 2026-11-12 | —             |"

echo "== a column at the formatter's five-character floor is still formatted =="
# Biome floors a column at five characters (a three-dash separator), so a column whose widest cell is
# one character is five wide while its content plus two is only three. That is a formatted table, not a
# hand-written one, and a value that does not fit it has to widen the column like any other.
build_repo 3
stub_records
FLOOR="$MAIN/odd/tasks/INDEX.md"
cat >"$FLOOR" <<'MD'
# Índice de tareas

| # | tarea | e | desde | último commit |
| --- | --- | --- | --- | --- |
| 100 | [alpha](100-alpha.md) | w | 2026-11-12 | — |
| 101 | [beta](101-beta.md) | w | 2026-11-12 | — |
| 102 | [gamma](102-gamma.md) | w | 2026-11-12 | — |
MD
format_index "$FLOOR"
expect_contains "the fixture starts at the five-character floor" "$FLOOR" \
  "| --- | --------------------- | --- | ---------- | ------------- |"
FLOOR_SHORT=$(git -C "$MAIN" rev-parse --short HEAD)
run_in "$WORK/wt1013" env PI_AGENT_ID=t PI_SESSION_ID=t ./scripts/claim-task.sh 101 >/dev/null 2>&1
expect_ok "deliver 101 into a five-wide column" \
  run_in "$MAIN" ./scripts/deliver-task.sh 101 "$FLOOR_SHORT"
expect_contains "the floored separator is widened to match" "$FLOOR" \
  "| --- | --------------------- | --------- | ---------- | ------------- |"
expect_contains "the header row is re-padded to the same width" "$FLOOR" \
  "| #   | tarea                 | e         | desde      | último commit |"
expect_contains "the delivered row fills the widened column" "$FLOOR" \
  "| 101 | [beta](101-beta.md)   | delivered | 2026-11-12 | \`$FLOOR_SHORT\`     |"
expect_contains "the row above is re-padded to the same width" "$FLOOR" \
  "| 100 | [alpha](100-alpha.md) | w         | 2026-11-12 | —             |"
expect_contains "the row below is re-padded to the same width" "$FLOOR" \
  "| 102 | [gamma](102-gamma.md) | w         | 2026-11-12 | —             |"

summary
