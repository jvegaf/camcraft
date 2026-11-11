#!/usr/bin/env bash
set -e
N="$1"
if [ -z "$N" ]; then echo "Usage: release-task.sh N"; exit 1; fi
ROOT=$(git rev-parse --show-toplevel)
CLAIM="$ROOT/odd/tasks/.claims/$N.json"
if [ -f "$CLAIM" ]; then
  rm "$CLAIM"
  echo "Released claim for task $N"
else
  echo "No claim found for task $N"
fi
