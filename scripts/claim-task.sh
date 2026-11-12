#!/usr/bin/env bash
# Claim one task for this worktree, by the file surfaces its document declares.
#
# The claim is what stops two agents from editing the same file in two worktrees. It is deliberately
# about *paths*, not crates: two tasks can live in `cam-cli` and never touch the same file, and a
# crate-level claim would refuse that pair while allowing two tasks that both edit
# `cam-geom/src/boolean.rs` under different crate tokens. A task that declares no `## Surfaces` block
# falls back to the crate tokens in its text, which is what the first version did, and says so.
#
# A surface may be a glob. A glob is compared by the literal prefix it can match — `src/*.rs` claims
# `src`, and therefore every file under it — because refusing too much is recoverable and allowing two
# agents into one file is not.
set -euo pipefail
N="${1:-}"
if [ -z "$N" ]; then echo "Usage: claim-task.sh N" >&2; exit 1; fi

PWD_REAL=$(git rev-parse --show-toplevel)
PWD_REAL=$(cd "$PWD_REAL" && pwd -P)
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
CLAIMDIR="$MAIN/odd/tasks/.claims"
CLAIM="$CLAIMDIR/$N.json"
TASKFILE=$(ls "$MAIN"/odd/tasks/"$N"-*.md 2>/dev/null | head -1 || true)

# Must claim from a linked task worktree, never the primary tree.
if [ "$PWD_REAL" = "$MAIN" ]; then
  echo "ERROR: Cannot claim from primary tree. Run from a task worktree (just worktree $N)." >&2
  exit 1
fi
if ! git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print}' | grep -Fxq "$PWD_REAL"; then
  echo "ERROR: Not in a registered git worktree ($PWD_REAL)." >&2
  exit 1
fi
if [ -z "$TASKFILE" ]; then
  echo "ERROR: No task document for $N in $MAIN/odd/tasks/." >&2
  exit 1
fi

mkdir -p "$CLAIMDIR"
BRANCH=$(git rev-parse --abbrev-ref HEAD)

python3 - "$TASKFILE" "$CLAIMDIR" "$CLAIM" "$N" "$BRANCH" "$PWD_REAL" \
  "${PI_AGENT_ID:-unknown}" "${PI_SESSION_ID:-unknown}" <<'PY'
import json, os, pathlib, re, sys
from datetime import datetime

taskfile, claimdir, claimpath, n, branch, worktree = sys.argv[1:7]
agent, session = sys.argv[7], sys.argv[8]
doc = pathlib.Path(taskfile)
text = doc.read_text(encoding="utf-8")

SURFACES_HEADING = re.compile(r"^##\s+Surfaces\s*$")
GLOB_CHARS = "*?["


def declared_surfaces() -> list[str]:
    """Paths and globs from the `## Surfaces` block, in document order, without cosmetic noise."""
    inside, out = False, []
    for line in text.splitlines():
        if SURFACES_HEADING.match(line):
            inside = True
            continue
        if inside and line.startswith("## "):
            inside = False
        if not inside:
            continue
        item = line.strip()
        if not item or item[0] not in "-*":
            continue
        item = re.sub(r"^[-*]\s*", "", item).strip().strip("`").strip()
        while item.startswith("./"):
            item = item[2:]
        item = item.lstrip("/").rstrip("/")
        if item and item not in out:
            out.append(item)
    return out


def enforced_base(surface: str) -> str:
    """What a surface can touch: its literal prefix, cut at the first glob segment.

    A surface that starts with a glob (`*.rs`) has no literal prefix at all, and `""` is read as
    "everything" rather than as "nothing": the empty string is not a path anybody can write, and
    answering "nothing" there is what lets two tasks claim the same file.
    """
    while surface.startswith("./"):
        surface = surface[2:]
    kept = []
    for part in surface.split("/"):
        if any(char in part for char in GLOB_CHARS):
            break
        kept.append(part)
    return "/".join(kept)


def legacy_crates() -> list[str]:
    return sorted(set(re.findall(r"crates/[A-Za-z0-9_-]+", text)))


def overlaps(one: str, other: str) -> bool:
    if not one or not other:
        return True
    return one == other or other.startswith(one + "/") or one.startswith(other + "/")


declared = declared_surfaces()
surfaces = []
for surface in declared:
    base = enforced_base(surface)
    if base not in surfaces:
        surfaces.append(base)
crates = [] if declared else legacy_crates()
mine = surfaces + crates

for other in sorted(pathlib.Path(claimdir).glob("*.json")):
    if other.resolve() == pathlib.Path(claimpath).resolve():
        continue
    try:
        claim = json.loads(other.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        # A claim nobody can read is not an absent claim: skipping it silently is how two agents end
        # up in the same file.
        print(
            f"ERROR: claim {other} cannot be read as JSON. Remove it, or fix it, before claiming; "
            "an unreadable claim cannot be checked against this one.",
            file=sys.stderr,
        )
        sys.exit(1)
    # The other claim may predate this normalization, so it is normalized here too: comparing a
    # normalized surface against a raw one is how a new exact path misses an old './path' claim.
    theirs = [enforced_base(surface) for surface in (claim.get("surfaces") or [])]
    theirs += list(claim.get("crates") or [])
    for surface in mine:
        for taken in theirs:
            if overlaps(surface, taken):
                print(
                    f"ERROR: Surface collision: {surface} is claimed by task "
                    f"{claim.get('task', '?')} ({other.name}).",
                    file=sys.stderr,
                )
                print(other.read_text(encoding="utf-8"), file=sys.stderr)
                sys.exit(1)

claim = {
    "task": int(n) if n.isdigit() else n,
    "agent": agent,
    "session_id": session,
    "branch": branch,
    "worktree": worktree,
    "mode": "surfaces" if declared else "crates-fallback",
    "surfaces": surfaces,
    "declared_surfaces": declared,
    "crates": crates,
    "started_at": datetime.now().astimezone().isoformat(timespec="seconds"),
}
payload = json.dumps(claim, indent=2) + "\n"

# Write the whole claim somewhere else, then link it into place: `os.link` fails if the claim already
# exists, so exclusive creation and complete content are the same operation. A claim written straight
# into its final path can be read while half of it is on disk.
staging = pathlib.Path(claimdir) / f".claim.{n}.{os.getpid()}.tmp"
with open(staging, "w", encoding="utf-8") as handle:
    handle.write(payload)
    handle.flush()
    os.fsync(handle.fileno())
try:
    os.link(staging, claimpath)
except FileExistsError:
    staging.unlink()
    print(f"ERROR: Task {n} already claimed:", file=sys.stderr)
    print(pathlib.Path(claimpath).read_text(encoding="utf-8"), file=sys.stderr)
    sys.exit(1)
staging.unlink()

print(f"Claimed task {n} ({claim['mode']})")
print(payload, end="")
if not declared:
    print(
        "WARNING: the task document declares no `## Surfaces` block, so the claim fell back to the "
        "crate tokens in its text. Declare the paths it will edit.",
        file=sys.stderr,
    )
PY
