#!/usr/bin/env bash
set -euo pipefail
N="${1:-}"
if [ -z "$N" ]; then echo "Usage: release-task.sh N" >&2; exit 1; fi

TOPLEVEL=$(git rev-parse --show-toplevel)
TOPLEVEL=$(cd "$TOPLEVEL" && pwd -P)
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
CLAIMDIR="$MAIN/odd/tasks/.claims"
CLAIM="$CLAIMDIR/$N.json"

# Claims live in the shared primary worktree, so releasing must prove this
# worktree owns the claim; otherwise one agent could drop another's claim.
if [ "$TOPLEVEL" = "$MAIN" ]; then
  echo "ERROR: Cannot release from primary tree. Run from the claiming task worktree." >&2
  exit 1
fi
if ! git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print}' | grep -Fxq "$TOPLEVEL"; then
  echo "ERROR: Not in a registered git worktree ($TOPLEVEL)." >&2
  exit 1
fi
if [ ! -f "$CLAIM" ]; then
  echo "No claim found for task $N"
  exit 0
fi

OWNER=$(grep -o '"worktree": *"[^"]*"' "$CLAIM" | head -1 | sed 's/.*"\([^"]*\)"$/\1/')
if [ "$OWNER" != "$TOPLEVEL" ]; then
  echo "ERROR: Task $N is claimed by another worktree (${OWNER:-unknown}), not $TOPLEVEL." >&2
  cat "$CLAIM" >&2
  exit 1
fi

rm -f "$CLAIM"
echo "Released claim for task $N"
