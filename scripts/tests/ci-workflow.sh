#!/usr/bin/env bash
# The workflow has one trunk: push and pull requests target `master`, and `parity`/`bench` only run
# when a human dispatches them. Branch protection names the three job names, so they stay.
set -uo pipefail
# shellcheck source=scripts/tests/lib/common.sh
. "$(dirname "$0")/lib/common.sh"

CI="$REPO_ROOT/.github/workflows/ci.yml"
PARSED="$WORK/ci.txt"

if [ ! -f "$CI" ]; then
  echo "FAIL  the workflow is missing: $CI" >&2
  echo
  echo "0 passed, 1 failed"
  exit 1
fi

# The workflow is parsed by indentation rather than with PyYAML: this test has to run on a bare shell
# with `grep`/`awk`/`sed`/`python3`, and only three things are read — the triggers, their branch
# lists, and each job's `name:`/`if:`. Both the flow list (`branches: [master]`) and the block list
# (`branches:` plus `- master`) are understood, so reformatting the file does not silently pass.
cat >"$WORK/parse.py" <<'PY'
import re
import sys
from pathlib import Path

items = []
for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    text = line.strip()
    if not text or text.startswith("#"):
        continue
    items.append((len(line) - len(line.lstrip(" ")), text))

KEY = re.compile(r"^([A-Za-z0-9_.\"'-]+)\s*:\s*(.*)$")


def key_of(text):
    match = KEY.match(text)
    if not match:
        return None, None
    return match.group(1).strip("\"'"), match.group(2).strip()


def top_index(name):
    for i, (indent, text) in enumerate(items):
        if indent == 0:
            key, _ = key_of(text)
            if key == name:
                return i
    return None


def nested(start):
    base = items[start][0]
    out = []
    for i in range(start + 1, len(items)):
        if items[i][0] <= base:
            break
        out.append(i)
    return out


def immediate(index):
    children = nested(index)
    if not children:
        return []
    indent = items[children[0]][0]
    return [i for i in children if items[i][0] == indent]


def child(index, name):
    for i in immediate(index):
        key, value = key_of(items[i][1])
        if key == name:
            return i, value
    return None


def flow_list(value):
    if not (value.startswith("[") and value.endswith("]")):
        return None
    return [part.strip().strip("\"'") for part in value[1:-1].split(",") if part.strip()]


def branches(index, value):
    if value:
        parsed = flow_list(value)
        if parsed is None:
            raise SystemExit(f"ERROR: unsupported branches form: {value!r}")
        return parsed
    out = []
    for i in nested(index):
        text = items[i][1]
        if not text.startswith("- "):
            raise SystemExit(f"ERROR: unsupported branches entry: {text!r}")
        out.append(text[2:].strip().strip("\"'"))
    return out


on_index = top_index("on")
if on_index is None:
    raise SystemExit("ERROR: no top-level `on:` block")

for i in immediate(on_index):
    key, value = key_of(items[i][1])
    if key is None:
        continue
    print(f"trigger_{key}={key}")
    if key in ("push", "pull_request"):
        found = child(i, "branches")
        if found is None:
            continue
        branches_index, branches_value = found
        print(f"{key}_branches=" + " ".join(branches(branches_index, branches_value)))

jobs_index = top_index("jobs")
if jobs_index is None:
    raise SystemExit("ERROR: no top-level `jobs:` block")

for i in immediate(jobs_index):
    key, value = key_of(items[i][1])
    if key is None or value:
        continue
    print(f"job={key}")
    for j in immediate(i):
        field, field_value = key_of(items[j][1])
        if field in ("name", "if"):
            print(f"{key}_{field}={field_value}")
PY

if ! python3 "$WORK/parse.py" "$CI" >"$PARSED" 2>"$WORK/parse-err"; then
  fail "the workflow could not be parsed"
  sed 's/^/      /' "$WORK/parse-err" >&2
  summary
  exit 1
fi

value() {
  sed -n "s/^$1=//p" "$PARSED"
}

expect_equal() {
  local label=$1 want=$2 got=$3
  if [ "$want" = "$got" ]; then
    pass "$label"
  else
    fail "$label (expected «$want», got «$got»)"
  fi
}

expect_no_dead_branch() {
  local trigger=$1 branch dead=""
  for branch in $(value "${trigger}_branches"); do
    case "$branch" in
      dev | feat/*) dead="$dead $branch" ;;
    esac
  done
  if [ -n "$dead" ]; then
    fail "$trigger must not name a dead branch, found:$dead"
  else
    pass "$trigger names no dead branch"
  fi
}

echo "== un solo tronco: push y PR apuntan a master =="
expect_equal "the push trigger targets master" "master" "$(value push_branches)"
expect_equal "the pull_request trigger targets master" "master" "$(value pull_request_branches)"

echo "== ninguna rama muerta recibe trabajo =="
expect_no_dead_branch push
expect_no_dead_branch pull_request

echo "== los nombres de job que la protección de rama nombra siguen ahí =="
for job in gate parity bench; do
  if grep -Fxq "job=$job" "$PARSED"; then
    pass "job $job exists"
  else
    fail "job $job is missing from the workflow"
  fi
  expect_equal "job $job keeps its name" "$job" "$(value "${job}_name")"
done

echo "== parity y bench sólo a pedido =="
expect_equal "the workflow can be dispatched by hand" "workflow_dispatch" \
  "$(value trigger_workflow_dispatch)"
for job in parity bench; do
  expect_equal "job $job runs only on workflow_dispatch" "github.event_name == 'workflow_dispatch'" \
    "$(value "${job}_if")"
done

summary
