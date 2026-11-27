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
if [ -z "$N" ] || [ "$#" -gt 2 ]; then
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

# The index is read, rewritten and replaced as one step, and two deliveries at once would each read
# the index before the other wrote it: the second would silently drop the first's row. The lock is the
# claims directory itself, opened read-only: it creates no file, so there is no predictable name for
# somebody to pre-plant as a symlink, and it sits in an ignored directory, so waiting never dirties the
# tree.
mkdir -p "$CLAIMDIR"
exec 9<"$CLAIMDIR"
if ! flock -w 2 9; then
  echo "ERROR: another delivery holds the claims directory; nothing was written." >&2
  exit 1
fi
if ! grep -q "| $N |" "$INDEX"; then
  echo "ERROR: no row for task $N in $INDEX; add the task before delivering it." >&2
  exit 1
fi

# The commit the row will point at has to exist: a hash nobody can resolve makes the index a guess.
if [ -n "$COMMIT" ] && ! git cat-file -e "${COMMIT}^{commit}" 2>/dev/null; then
  echo "ERROR: commit $COMMIT does not exist in this repository; nothing was written." >&2
  exit 1
fi

# Before the index says "delivered": the document must be complete and the task must exist in
# memory. Delivering is the moment something is called finished, so it is the moment to prove the
# record exists — not the day after, when nobody remembers what the task even was.
if ! "$TOPLEVEL/scripts/check-records.sh" --task "$N" --engram-required; then
  echo "ERROR: task $N is not recorded yet (see above); nothing was written." >&2
  exit 1
fi

python3 - "$INDEX" "$N" "$COMMIT" <<'PY'
import os, sys
from pathlib import Path

index, n, commit = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
stamp = f"`{commit}`" if commit else "—"
lines = index.read_text(encoding="utf-8").splitlines()


def cells_of(line):
    return line.split("|")


def is_separator(cells):
    inner = cells[1:-1]
    return bool(inner) and all(c.strip() and set(c.strip()) == {"-"} for c in inner)


# The table is a block of pipe rows; the delivered row has to sit in the one whose separator row says
# how wide each column is, because that row is the only place the table states its own shape.
blocks, current = [], []
for i, line in enumerate(lines):
    if line.startswith("|"):
        current.append(i)
    elif current:
        blocks.append(current)
        current = []
if current:
    blocks.append(current)

target = next(
    (
        i
        for block in blocks
        for i in block
        if len(cells_of(lines[i])) == 7
        and cells_of(lines[i])[1].strip() == n
        and not is_separator(cells_of(lines[i]))
    ),
    None,
)
if target is None:
    sys.exit(f"ERROR: row for task {n} is not a well-formed index row")

block = next(b for b in blocks if target in b)
sep_at = next((i for i in block if is_separator(cells_of(lines[i]))), None)
if sep_at is None:
    sys.exit(f"ERROR: the table holding task {n} has no separator row")
separator = cells_of(lines[sep_at])
columns = len(separator) - 2
rows = [i for i in block if i != sep_at and len(cells_of(lines[i])) == columns + 2]

# Biome pads every cell of a column to the widest cell plus two spaces, and pads the separator row to
# the same width. It also floors the width at five characters, because a separator needs three dashes,
# so a column whose cells are all three characters or shorter still comes out five wide. A table
# written by hand has no such widths: its separator is `| --- |` whatever the content, and its cells are
# `| value |`. Reading that difference off the separator is what lets one delivery find the table's own
# style instead of imposing one.
def formatted_width(widest_cell):
    return max(5, widest_cell + 2)


widths = [len(separator[c + 1]) for c in range(columns)]
widest = [max(len(cells_of(lines[i])[c + 1].strip()) for i in rows) for c in range(columns)]
formatted = [widths[c] == formatted_width(widest[c]) for c in range(columns)]

# `| # | tarea | estado | desde | último commit |`: estado is cell 3, último commit is cell 5.
out = list(lines)
for column, text in ((2, "delivered"), (4, stamp)):
    if formatted[column]:
        width = widths[column]
        if len(text) > width - 2:
            # The padding is uniform across the table, so a value that does not fit widens the whole
            # column rather than overflowing one row: every cell is re-padded and the separator row
            # carries the new width, floored at the same five characters as the detection above.
            width = formatted_width(len(text))
            for i in rows:
                cells = cells_of(out[i])
                cells[column + 1] = " " + cells[column + 1].strip().ljust(width - 1)
                out[i] = "|".join(cells)
            cells = cells_of(out[sep_at])
            cells[column + 1] = " " + "-" * (width - 2) + " "
            out[sep_at] = "|".join(cells)
        cells = cells_of(out[target])
        cells[column + 1] = " " + text.ljust(width - 1)
        out[target] = "|".join(cells)
    else:
        cells = cells_of(out[target])
        cells[column + 1] = f" {text} "
        out[target] = "|".join(cells)

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
