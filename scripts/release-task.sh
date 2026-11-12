#!/usr/bin/env bash
set -euo pipefail
N="${1:-}"
if [ -z "$N" ]; then echo "Usage: release-task.sh N" >&2; exit 1; fi
MAIN=$(git worktree list --porcelain | awk '/^worktree /{sub(/^worktree /,""); print; exit}')
MAIN=$(cd "$MAIN" && pwd -P)
CLAIM="$MAIN/odd/tasks/.claims/$N.json"
if [ -f "$CLAIM" ]; then
  rm -f "$CLAIM"
  echo "Released claim for task $N"
else
  echo "No claim found for task $N"
fi
