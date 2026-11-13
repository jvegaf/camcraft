#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
CRATE_DIR="$ROOT/crates/cam-ops"
BASE_SRC="$CRATE_DIR/benches/baselines/criterion"
BASE_DST="$ROOT/target/criterion"
if [ ! -d "$BASE_SRC" ]; then
  echo "Missing committed baseline at $BASE_SRC" >&2
  exit 1
fi
mkdir -p "$BASE_DST"
cp -a "$BASE_SRC"/. "$BASE_DST"/
cd "$ROOT"
exec cargo bench -p cam-ops --bench toolpaths -- --baseline committed "$@"
