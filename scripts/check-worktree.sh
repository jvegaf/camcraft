#!/usr/bin/env bash
set -e
ROOT=$(git rev-parse --show-toplevel)
WT=$(git rev-parse --show-toplevel)
PWD_REAL=$(pwd)
BRANCH=$(git rev-parse --abbrev-ref HEAD)
MAIN=$(git worktree list --porcelain | grep '^worktree ' | head -1 | cut -d' ' -f2-)
if [ "$PWD_REAL" = "$MAIN" ] && [ "$BRANCH" = "master" ] || [ "$BRANCH" = "main" ]; then
  echo "ERROR: Working in primary tree on master/main. Use a task worktree (just worktree N)" >&2
  exit 1
fi
# Check if in a registered worktree
if git worktree list | grep -q "$PWD_REAL"; then
  echo "OK: In worktree $PWD_REAL ($BRANCH)"
else
  echo "WARNING: Not in a registered git worktree" >&2
  exit 1
fi
