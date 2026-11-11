#!/usr/bin/env bash
set -e
N="$1"
if [ -z "$N" ]; then echo "Usage: claim-task.sh N"; exit 1; fi
ROOT=$(git rev-parse --show-toplevel)
WT=$(pwd)
BRANCH=$(git rev-parse --abbrev-ref HEAD)
CLAIMDIR="$ROOT/odd/tasks/.claims"
mkdir -p "$CLAIMDIR"
CLAIM="$CLAIMDIR/$N.json"
if [ -f "$CLAIM" ]; then
  echo "Task $N already claimed:" >&2
  cat "$CLAIM" >&2
  exit 1
fi
TASKFILE=$(ls "$ROOT/odd/tasks/$N"-*.md 2>/dev/null | head -1)
CRATES=""
if [ -n "$TASKFILE" ]; then
  CRATES=$(grep -oP 'crates/[a-zA-Z0-9_-]+' "$TASKFILE" | sort -u | tr '\n' ' ')
fi
cat > "$CLAIM" << JSON
{
  "task": $N,
  "agent": "${PI_AGENT_ID:-unknown}",
  "session_id": "${PI_SESSION_ID:-unknown}",
  "branch": "$BRANCH",
  "worktree": "$WT",
  "crates": [$(echo $CRATES | sed 's/ /", "/g' | sed 's/^/"/;s/$/"/' | sed 's/""//g')],
  "started_at": "$(date -Iseconds)"
}
JSON
# Update INDEX to mark in progress if not already
if grep -q "| $N |" "$ROOT/odd/tasks/INDEX.md"; then
  sed -i "s/| $N | \([^|]*\)| [^|]*|/| $N | \1| in progress|/" "$ROOT/odd/tasks/INDEX.md" 2>/dev/null || true
fi
echo "Claimed task $N"
cat "$CLAIM"
