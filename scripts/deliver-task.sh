#!/usr/bin/env bash
# Record a delivered task in the shared index and release its claim.
#
# Run this from the primary tree *after* the merge, never from the claiming worktree: the index is
# shared state, and a claim that writes it from a linked worktree edits a file that is in no branch —
# it drifts, nobody commits it, and the next agent reads a lie. This script is the one writer.
set -euo pipefail
N="${1:-}"
COMMIT="${2:-}"
if [ -z "$N" ]; then
  echo "Usage: deliver-task.sh N [commit]" >&2
  exit 1
fi

MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
INDEX="$MAIN/odd/tasks/INDEX.md"
CLAIMDIR="$MAIN/odd/tasks/.claims"
CLAIM="$CLAIMDIR/$N.json"

if [ ! -f "$INDEX" ]; then
  echo "ERROR: no task index at $INDEX." >&2
  exit 1
fi
if ! grep -q "| $N |" "$INDEX"; then
  echo "ERROR: no row for task $N in $INDEX; add the task before delivering it." >&2
  exit 1
fi

python3 - "$INDEX" "$N" "$COMMIT" <<'PY'
import sys
from pathlib import Path

index, n, commit = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
stamp = f"`{commit}`" if commit else "—"
out, hit = [], False
for line in index.read_text(encoding="utf-8").splitlines():
    cells = line.split("|")
    # `| # | tarea | estado | desde | último commit |` — cells[0] and cells[-1] are the empty edges.
    if len(cells) == 7 and cells[1].strip() == n:
        cells[3] = " delivered "
        cells[5] = f" {stamp} "
        line = "|".join(cells)
        hit = True
    out.append(line)
if not hit:
    sys.exit(f"ERROR: row for task {n} is not a well-formed index row")
index.write_text("\n".join(out) + "\n", encoding="utf-8")
PY

rm -f "$CLAIM"
echo "Delivered task $N${COMMIT:+ at $COMMIT}; claim released."
echo "Commit the index change: git add odd/tasks/INDEX.md"
