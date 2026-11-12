#!/usr/bin/env bash
set -euo pipefail
N="${1:-}"
if [ -z "$N" ]; then echo "Usage: claim-task.sh N" >&2; exit 1; fi

PWD_REAL=$(pwd -P)
BRANCH=$(git rev-parse --abbrev-ref HEAD)
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

mkdir -p "$CLAIMDIR"

CRATES=""
if [ -n "${TASKFILE:-}" ]; then
  CRATES=$(grep -oP 'crates/[a-zA-Z0-9_-]+' "$TASKFILE" | sort -u || true)
fi

# Detect task/crate collisions across shared claims before creating ours.
if [ -f "$CLAIM" ]; then
  echo "ERROR: Task $N already claimed:" >&2
  cat "$CLAIM" >&2
  exit 1
fi
shopt -s nullglob
for f in "$CLAIMDIR"/*.json; do
  [ "$(basename "$f")" = "$N.json" ] && continue
  if [ -n "$CRATES" ]; then
    for c in $CRATES; do
      if grep -q "\"$c\"" "$f" 2>/dev/null; then
        echo "ERROR: Crate collision: $c already claimed in $(basename "$f")" >&2
        cat "$f" >&2
        exit 1
      fi
    done
  fi
done

# Atomic claim creation: fail if file appears between checks.
tmp=$(mktemp "$CLAIMDIR/.claim.$N.XXXXXX")
crates_json=""
if [ -n "$CRATES" ]; then
  crates_json=$(printf '%s\n' $CRATES | sed 's/.*/"&"/' | paste -sd, -)
fi
cat > "$tmp" << JSON
{
  "task": $N,
  "agent": "${PI_AGENT_ID:-unknown}",
  "session_id": "${PI_SESSION_ID:-unknown}",
  "branch": "$BRANCH",
  "worktree": "$PWD_REAL",
  "crates": [${crates_json}],
  "started_at": "$(date -Iseconds)"
}
JSON
if ! ( set -o noclobber; : > "$CLAIM" ) 2>/dev/null; then
  rm -f "$tmp"
  echo "ERROR: Task $N already claimed (concurrent):" >&2
  cat "$CLAIM" >&2 || true
  exit 1
fi
mv -f "$tmp" "$CLAIM"

# Best-effort: mark in progress in shared INDEX (primary tree).
if [ -f "$MAIN/odd/tasks/INDEX.md" ] && grep -q "| $N |" "$MAIN/odd/tasks/INDEX.md"; then
  sed -i "s/| $N | \([^|]*\)| [^|]*|/| $N | \1| in progress |/" "$MAIN/odd/tasks/INDEX.md" 2>/dev/null || true
fi
echo "Claimed task $N"
cat "$CLAIM"
