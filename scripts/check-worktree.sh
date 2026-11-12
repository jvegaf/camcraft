#!/usr/bin/env bash
# Validate that this tree may hold work: a linked worktree needs a claim, and the primary tree must be
# clean.
#
# Called with no argument it is the strict answer an agent asks for. Called with `--gate` it is the
# same answer made usable inside `scripts/check.sh`, where the primary tree is a legal place to validate
# a merge but an illegal place to work: a clean primary tree passes, one carrying any uncommitted change
# does not.
#
# The primary tree's condition is decided by whether `git status` reports anything at all, never by
# parsing its output: the paths that matter are all of them, and a parser that answers "clean" for a
# rename it did not expect, or for a `git status` that failed, is worse than no gate.
set -euo pipefail
GATE=0
case "${1:-}" in
  --gate) GATE=1 ;;
  "") ;;
  *)
    echo "Usage: check-worktree.sh [--gate]" >&2
    exit 2
    ;;
esac

PWD_REAL=$(pwd -P)
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
CLAIMDIR="$MAIN/odd/tasks/.claims"

if [ "$PWD_REAL" = "$MAIN" ]; then
  if [ "$GATE" -eq 0 ]; then
    echo "ERROR: Working in primary tree. Use a task worktree (just worktree N)." >&2
    exit 1
  fi
  # In the gate, a primary tree is fine as long as it is not doing the work itself.
  STATUS=$(git status --porcelain) || {
    echo "ERROR: git status failed in the primary tree, so the gate cannot call it clean." >&2
    exit 1
  }
  if [ -n "$STATUS" ]; then
    echo "ERROR: primary tree is not clean; that work belongs in a claimed worktree:" >&2
    printf '%s\n' "$STATUS" | sed 's/^/  /' >&2
    exit 1
  fi
  echo "OK: primary tree, clean"
  exit 0
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

echo "OK: In claimed worktree $PWD_REAL ($(git rev-parse --abbrev-ref HEAD)) claim $(basename "${matches[0]}")"
