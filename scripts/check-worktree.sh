#!/usr/bin/env bash
set -euo pipefail

PWD_REAL=$(pwd -P)
BRANCH=$(git rev-parse --abbrev-ref HEAD)
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
CLAIMDIR="$MAIN/odd/tasks/.claims"

# Never work in the primary tree (regardless of branch).
if [ "$PWD_REAL" = "$MAIN" ]; then
  echo "ERROR: Working in primary tree. Use a task worktree (just worktree N)." >&2
  exit 1
fi

# Must be a registered linked worktree.
if ! git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print}' | grep -Fxq "$PWD_REAL"; then
  echo "ERROR: Not in a registered git worktree ($PWD_REAL)." >&2
  exit 1
fi

# Require a claim for this worktree so parallel work is coordinated.
shopt -s nullglob
matches=()
for f in "$CLAIMDIR"/*.json; do
  wt=$(grep -oP '"worktree"\s*:\s*"\K[^"]+' "$f" 2>/dev/null || true)
  if [ -n "$wt" ] && [ "$(cd "$wt" 2>/dev/null && pwd -P || echo "$wt")" = "$PWD_REAL" ]; then
    matches+=("$f")
  fi
done
if [ ${#matches[@]} -eq 0 ]; then
  echo "ERROR: No task claim found for worktree $PWD_REAL. Run 'just claim N' from this worktree." >&2
  exit 1
fi

echo "OK: In claimed worktree $PWD_REAL ($BRANCH) claim $(basename "${matches[0]}")"
