#!/usr/bin/env bash
# Gate principal para el stack Tauri + Svelte + Panda.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== claim =="
./scripts/check-worktree.sh --gate

echo "== docs =="
./scripts/check-docs.sh

echo "== records =="
./scripts/check-records.sh

echo "== biome =="
if command -v pnpm >/dev/null 2>&1 && [ -f package.json ]; then
  pnpm lint
else
  echo "skip: pnpm/package.json no disponible"
fi

echo "== svelte-check =="
if command -v pnpm >/dev/null 2>&1 && [ -f package.json ]; then
  pnpm check
else
  echo "skip: pnpm/package.json no disponible"
fi

echo "== rust (workspace check) =="
if [ -f Cargo.toml ]; then
  cargo check --workspace
else
  echo "skip: Cargo.toml no encontrado"
fi

echo "== rust (test src-tauri) =="
if [ -f src-tauri/Cargo.toml ]; then
  cargo test --manifest-path src-tauri/Cargo.toml
else
  echo "skip: src-tauri/Cargo.toml no encontrado"
fi

echo "== vitest =="
if command -v pnpm >/dev/null 2>&1 && [ -f package.json ]; then
  if node -e "process.exit(require('./package.json').scripts?.test ? 0 : 1)" >/dev/null 2>&1; then
    pnpm test -- --run 2>/dev/null || pnpm test
  else
    echo "skip: script test no definido en package.json"
  fi
else
  echo "skip: pnpm/package.json no disponible"
fi

echo "all gates green"
