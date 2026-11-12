#!/usr/bin/env bash
# Record a delivered task in the shared index and release its claim.
#
# Run this from the primary tree *after* the merge, never from the claiming worktree: the index is
# shared state, and a claim that writes it from a linked worktree edits a file that is in no branch —
# it drifts, nobody commits it, and the next agent reads a lie. This script is the one writer, and it
# enforces that from here rather than by asking nicely in its own comment.
set -euo pipefail
N="${1:-}"
COMMIT="${2:-}"
if [ -z "$N" ]; then
  echo "Usage: deliver-task.sh N [commit]" >&2
  exit 1
fi

TOPLEVEL=$(git rev-parse --show-toplevel)
TOPLEVEL=$(cd "$TOPLEVEL" && pwd -P)
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
INDEX="$MAIN/odd/tasks/INDEX.md"
CLAIMDIR="$MAIN/odd/tasks/.claims"
CLAIM="$CLAIMDIR/$N.json"

# Delivering from a worktree would mark a task delivered before its branch is merged. The tree is
# asked for its root, never the shell's directory: a subdirectory of the primary tree is still the
# primary tree.
if [ "$TOPLEVEL" != "$MAIN" ]; then
  echo "ERROR: Cannot deliver from $TOPLEVEL. Run it from the primary tree, after the merge." >&2
  exit 1
fi

if [ ! -f "$INDEX" ]; then
  echo "ERROR: no task index at $INDEX." >&2
  exit 1
fi
if ! grep -q "| $N |" "$INDEX"; then
  echo "ERROR: no row for task $N in $INDEX; add the task before delivering it." >&2
  exit 1
fi

python3 - "$INDEX" "$N" "$COMMIT" <<'PY'
import os, sys
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

# Rewrite through a sibling file and rename it into place: `os.replace` is atomic, so a crash leaves
# the old index intact instead of a half-written one.
staging = index.with_name(f".{index.name}.{os.getpid()}.tmp")
with open(staging, "w", encoding="utf-8") as handle:
    handle.write("\n".join(out) + "\n")
    handle.flush()
    os.fsync(handle.fileno())
os.replace(staging, index)
PY

rm -f "$CLAIM"
echo "Delivered task $N${COMMIT:+ at $COMMIT}; claim released."
echo "Commit the index change: git add odd/tasks/INDEX.md"
