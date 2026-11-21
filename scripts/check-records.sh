#!/usr/bin/env bash
# Prove that a task is recorded before anyone calls it finished.
#
# A task is "done" when it is in three places: the document that says what was asked and what
# happened (`odd/tasks/NNN-*.md`), the row that says where it stands (`odd/tasks/INDEX.md`), and the
# memory that survives the session (`Engram`). The first two live in git and are checked here by
# reading them; the third lives outside the repository and is checked through the `engram` CLI.
#
# The rule this enforces is deliberately about *records*, not about code: whether the code is right
# is what tests and review are for. What this refuses is the other failure — work marked complete with
# nothing written down, which is indistinguishable from work that never happened.
#
# Usage:
#   scripts/check-records.sh                  # every row in the index
#   scripts/check-records.sh --task 53        # one task, as a delivery would
#   scripts/check-records.sh --engram-required
#
# The Engram half is skipped when neither the CLI nor its daemon is reachable, because CI has no
# memory store. A skip is printed, never silent; `--engram-required` turns it into a failure, and
# `scripts/deliver-task.sh` always passes that flag: a task cannot be marked delivered on a machine
# that cannot show the record exists.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"

task=""
engram_required=false
while [ "$#" -gt 0 ]; do
  case "$1" in
    --task)
      task="${2:-}"
      [ -n "$task" ] || { echo "ERROR: --task needs a number" >&2; exit 1; }
      shift 2
      ;;
    --engram-required)
      engram_required=true
      shift
      ;;
    *)
      echo "ERROR: unknown argument $1" >&2
      exit 1
      ;;
  esac
done

index="odd/tasks/INDEX.md"
[ -f "$index" ] || { echo "ERROR: no task index at $index" >&2; exit 1; }

# The index alone decides what is checked: a row that is not there is not a task, and a document
# without a row is not reachable by anyone reading the index.
rows=$(python3 - "$index" "$task" <<'PY'
import re, sys
from pathlib import Path

index, only = Path(sys.argv[1]), sys.argv[2]
for line in index.read_text(encoding="utf-8").splitlines():
    cells = line.split("|")
    if len(cells) != 7:
        continue
    number = cells[1].strip()
    if not number.isdigit():
        continue
    if only and number.lstrip("0") != only.lstrip("0"):
        continue
    link = re.search(r"\[([^\]]+)\]\(([^)]+)\)", cells[2])
    slug = link.group(1) if link else ""
    target = link.group(2) if link else ""
    state = cells[3].strip()
    since = cells[4].strip()
    commit = cells[5].strip().strip("`")
    print("\t".join([number, slug, target, state, since, commit]))
PY
)

if [ -z "$rows" ]; then
  echo "ERROR: no task row${task:+ for task $task} in $index" >&2
  exit 1
fi

failures=0
checked=0
skipped_commits=0
shallow=false
[ "$(git rev-parse --is-shallow-repository)" = "true" ] && shallow=true

fail() {
  echo "  ✗ $*"
  failures=$((failures + 1))
}

# The sections a task document must carry for its claim to be checkable by a human who was not there.
required_sections=("## Surfaces" "## Specs" "## Tasks" "## Log")

# The convention this checks was not retroactive: documents written before it have no `## Surfaces`
# and no `## Log`, and failing them would fail history. The structure is required from the cutover
# task on; earlier rows get the same findings as advisories.
structure_from=53
# Naming one task asks about that task, always: the bulk scan needs a cutover because history cannot
# be rewritten, but a delivery is a decision being taken now.
[ -n "$task" ] && structure_from=0

while IFS=$'\t' read -r number slug target state since commit; do
  checked=$((checked + 1))
  echo "task $number ($slug): $state"
  required=false
  [ "$number" -ge "$structure_from" ] && required=true

  # The index links are relative to `odd/tasks/`, which is where the document lives.
  doc="odd/tasks/$target"
  if [ ! -f "$doc" ]; then
    fail "the index points at $target, which does not exist under odd/tasks/"
    continue
  fi

  for section in "${required_sections[@]}"; do
    if ! grep -qxF "$section" "$doc"; then
      if $required; then
        fail "$doc has no '$section' section"
      else
        echo "  · $doc predates the convention and has no '$section' section"
      fi
    fi
  done

  if ! grep -qE '^\- \*\*L[0-9]+' "$doc"; then
    if $required; then
      fail "$doc has no Log entry (expected a line starting '- **L1…')"
    else
      echo "  · $doc predates the convention and has no Log entry"
    fi
  fi

  if [ "$commit" != "—" ] && [ -n "$commit" ]; then
    if $shallow; then
      skipped_commits=$((skipped_commits + 1))
      echo "  · commit $commit not verified: the clone is shallow"
    elif git cat-file -e "${commit}^{commit}" 2>/dev/null; then
      git merge-base --is-ancestor "$commit" HEAD 2>/dev/null ||
        echo "  · commit $commit is not an ancestor of HEAD (a branch not merged here yet)"
    else
      fail "commit $commit does not exist in this repository"
    fi
  fi
done <<<"$rows"

# The memory half. The rule starts at a task number, not at a date: demanding a memory for the fifty
# tasks that predate it would fail for history rather than for the rule. Anything delivered before the
# cutover is listed as an advisory, so the gap stays visible instead of being silently tolerated.
cutover=53
if [ -n "$task" ]; then
  memory_rows=$(awk -F'\t' '{ print $1 "\t" $2 }' <<<"$rows")
else
  memory_rows=$(awk -F'\t' -v cut="$cutover" '$4 == "delivered" && $1 + 0 >= cut { print $1 "\t" $2 }' <<<"$rows")
fi
history_rows=$(awk -F'\t' -v cut="$cutover" '$4 == "delivered" && $1 + 0 < cut { print $1 "\t" $2 }' <<<"$rows")

if [ -z "$memory_rows" ]; then
  echo "engram: nothing to check (no task delivered on or after $cutover)"
else
  if ! command -v engram >/dev/null 2>&1; then
    if $engram_required; then
      fail "the engram CLI is not on PATH, so no memory can be shown for: $(cut -f1 <<<"$memory_rows" | tr '\n' ' ')"
    else
      echo "engram: SKIPPED — the CLI is not on PATH, so no memory was checked"
      memory_rows=""
    fi
  elif ! engram ps 2>/dev/null | grep -q "running"; then
    if $engram_required; then
      fail "the engram daemon is not running, so no memory can be shown for: $(cut -f1 <<<"$memory_rows" | tr '\n' ' ')"
    else
      echo "engram: SKIPPED — the daemon is not running, so no memory was checked"
      memory_rows=""
    fi
  fi
fi

while IFS=$'\t' read -r number slug; do
  [ -n "${number:-}" ] || continue
  echo "task $number: memory for '$slug'"
  found=$(engram search "$slug" --all --limit 1 2>/dev/null | head -1 || true)
  if echo "$found" | grep -qE "Found [1-9][0-9]* memor"; then
    echo "  ✓ $found"
  else
    fail "no Engram memory mentions '$slug' (save one with topic_key 'camcraft/$slug' before delivering)"
  fi
done <<<"$memory_rows"

# Delivered before the rule: reported, never failed, so the honest list is visible.
if [ -n "${history_rows:-}" ] && command -v engram >/dev/null 2>&1 && engram ps 2>/dev/null | grep -q "running"; then
  while IFS=$'\t' read -r number slug; do
    [ -n "${number:-}" ] || continue
    found=$(engram search "$slug" --all --limit 1 2>/dev/null | head -1 || true)
    if ! echo "$found" | grep -qE "Found [1-9][0-9]* memor"; then
      echo "engram: ADVISORY — task $number ($slug) was delivered with no memory; backfill it when touched"
    fi
  done <<<"$history_rows"
fi

if [ "$failures" -gt 0 ]; then
  echo "records: $failures problem(s) in $checked task(s)" >&2
  exit 1
fi

echo "records: $checked task(s) checked${skipped_commits:+, $skipped_commits commit(s) unverified in a shallow clone}"
