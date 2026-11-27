#!/usr/bin/env bash
# Lanzamiento único del catálogo del design system: libera el puerto, prepara el árbol y arranca
# Vite. Lo usan tanto `just ds` como `pnpm ds`; no depende del cwd desde donde se invoque.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
ROOT=$(cd "$SCRIPT_DIR/.." && pwd -P)
cd "$ROOT"

echo "== puerto 1420 =="
"$SCRIPT_DIR/free-port.sh" 1420

echo "== codegen de Panda =="
pnpm codegen

echo "== inventario del design system =="
node scripts/ds-inventory.mjs

echo "Catálogo del design system: http://127.0.0.1:1420/ds"
exec pnpm dev
