#!/usr/bin/env bash
# Compare the cam-ops toolpath benches against the committed Criterion baseline.
#
# Criterion exits 0 even when it prints "Performance has regressed", so this script reads the
# comparison output and fails on its own. The committed baseline is copied into the workspace
# target directory (where cargo actually reads it), never the other way round.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CRATE="cam-ops"
BENCH="toolpaths"
# Repo root: benches -> cam-ops -> crates -> src-tauri -> repo.
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
BASE_SRC="$SCRIPT_DIR/baselines/criterion"
BASE_DST="$WORKSPACE_ROOT/target/criterion"
BENCH_SRC="$SCRIPT_DIR/$BENCH.rs"
# 10 % relative slowdown is a regression. Well above ordinary run-to-run noise on this host and
# far below the 3.4x NCC regression this gate has to catch. The decision is relative: it reads
# Criterion's own "change: [low mid high]" line and rejects a measured (mid) slowdown above this.
THRESHOLD_PCT=10

if [ ! -d "$BASE_SRC" ]; then
  echo "Missing committed baseline at $BASE_SRC" >&2
  exit 1
fi

mapfile -t BENCHES < <(
  grep -oE 'bench_function\("[^"]+"' "$BENCH_SRC" | sed -E 's/.*"([^"]+)"$/\1/'
)
if [ "${#BENCHES[@]}" -eq 0 ]; then
  echo "No bench_function declarations found in $BENCH_SRC" >&2
  exit 1
fi

missing=0
for bench_id in "${BENCHES[@]}"; do
  dir="${bench_id//\//_}"
  if [ ! -f "$BASE_SRC/$dir/committed/estimates.json" ]; then
    echo "Missing committed baseline for '$bench_id' at $BASE_SRC/$dir/committed" >&2
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  exit 1
fi

mkdir -p "$BASE_DST"
# Restore committed criterion baseline data so --baseline committed can compare against it.
cp -a "$BASE_SRC"/. "$BASE_DST"/

echo "Comparing cam-ops/$BENCH against committed baseline:"
echo "  baseline: $BASE_SRC"
echo "  target:   $BASE_DST"
echo "  benches:  ${BENCHES[*]}"
echo "  threshold: +${THRESHOLD_PCT}% relative slowdown (parsed from the Criterion change interval)"

LOG="$(mktemp "${TMPDIR:-/tmp}/cam-ops-bench-regression.XXXXXX.log")"
trap 'rm -f "$LOG"' EXIT

cd "$WORKSPACE_ROOT"
set +e
cargo bench -p "$CRATE" --bench "$BENCH" -- \
  --baseline committed \
  --noplot \
  "$@" 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}
set -e

if [ "$status" -ne 0 ]; then
  echo "cargo bench failed with exit code $status" >&2
  exit "$status"
fi

parse_status=0
CHANGES="$(awk -v thr="$THRESHOLD_PCT" '
  /^[A-Za-z0-9_]+\/[A-Za-z0-9_]+( |$)/ { id = $1 }
  /change: \[/ {
    s = $0; sub(/.*change: \[/, "", s); sub(/\].*/, "", s)
    n = split(s, b, " "); mid = b[2]; sub(/%$/, "", mid)
    if (mid + 0 > thr) { bad = 1; printf "  %s change: [%s]\n", id, s }
  }
  END { if (!n) { print "  no Criterion change line found in " FILENAME; exit 1 }; exit (bad ? 1 : 0) }
' "$LOG")" || parse_status=$?

if [ "${parse_status:-0}" -ne 0 ]; then
  echo "" >&2
  echo "REGRESSION (or unparseable bench output) vs $BASE_SRC, threshold +${THRESHOLD_PCT}%:" >&2
  printf '%s\n' "$CHANGES" >&2
  exit 1
fi

echo ""
echo "OK: ${#BENCHES[@]} cam-ops benches compared against $BASE_SRC, none regressed."
