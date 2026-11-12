#!/usr/bin/env bash
# Claim one task for this worktree, by the file surfaces its document declares.
#
# The claim is what stops two agents from editing the same file in two worktrees. It is deliberately
# about *paths*, not crates: two tasks can live in `cam-cli` and never touch the same file, and a
# crate-level claim would refuse that pair while allowing two tasks that both edit
# `cam-geom/src/boolean.rs` under different crate tokens. A task that declares no `## Surfaces` block
# falls back to the crate tokens in its text, which is what the first version did, and says so.
set -euo pipefail
N="${1:-}"
if [ -z "$N" ]; then echo "Usage: claim-task.sh N" >&2; exit 1; fi

PWD_REAL=$(pwd -P)
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


def declared_surfaces() -> list[str]:
    """Paths and globs from the `## Surfaces` block, in document order."""
    inside, out = False, []
    for line in text.splitlines():
        if re.match(r"^##\s+Surfaces\s*$", line):
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
        item = item.rstrip("/")
        item = re.sub(r"/\*+$", "", item)  # `crates/x/**` names the same tree as `crates/x`
        if item and item not in out:
            out.append(item)
    return out


def legacy_crates() -> list[str]:
    return sorted(set(re.findall(r"crates/[A-Za-z0-9_-]+", text)))


def overlaps(one: str, other: str) -> bool:
    return (
        one == other
        or other.startswith(one + "/")
        or one.startswith(other + "/")
    )


surfaces = declared_surfaces()
crates = [] if surfaces else legacy_crates()
mine = surfaces + crates

for other in sorted(pathlib.Path(claimdir).glob("*.json")):
    if other.resolve() == pathlib.Path(claimpath).resolve():
        continue
    try:
        claim = json.loads(other.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        continue
    theirs = list(claim.get("surfaces") or []) + list(claim.get("crates") or [])
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
    "mode": "surfaces" if surfaces else "crates-fallback",
    "surfaces": surfaces,
    "crates": crates,
    "started_at": datetime.now().astimezone().isoformat(timespec="seconds"),
}

try:
    handle = open(claimpath, "x", encoding="utf-8")
except FileExistsError:
    print(f"ERROR: Task {n} already claimed:", file=sys.stderr)
    print(pathlib.Path(claimpath).read_text(encoding="utf-8"), file=sys.stderr)
    sys.exit(1)
with handle:
    handle.write(json.dumps(claim, indent=2) + "\n")

print(f"Claimed task {n} ({claim['mode']})")
print(json.dumps(claim, indent=2))
if not surfaces:
    print(
        "WARNING: the task document declares no `## Surfaces` block, so the claim fell back to the "
        "crate tokens in its text. Declare the paths it will edit.",
        file=sys.stderr,
    )
PY
