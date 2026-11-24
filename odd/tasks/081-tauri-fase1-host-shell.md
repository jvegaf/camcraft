# 081 — Fase 1: host + shell mínimo

Implementar Fase 1 de la migración Tauri: start screen real con DS, comandos recents/open/new y settings mínimas.

## Surfaces

- `src-tauri/src/lib.rs`
- `src-tauri/tauri.conf.json`
- `src/routes/+page.svelte`
- `src/routes/workspace/+page.svelte`
- `src/lib/components/start-screen/**`
- `src/lib/components/settings/**` (si se crea)
- `src/lib/stores/**` (si se crea)
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/081-tauri-fase1-host-shell.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID  | Tarea                                               | Specs | Ruta   | Commit |
| --- | --------------------------------------------------- | ----- | ------ | ------ |
| T1  | Wire start screen DS real a recents/open/new/create | S1    | inline |        |
| T2  | Agregar settings mínimas (backend Tauri + UI)       | S1    | inline |        |
| T3  | Verificar, RDD, ODD/Engram, commit y push           | S1    | inline |        |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 1 de `docs/MIGRATION_TAURI.md`: start screen real con DS HTML/CSS, comandos recents/open/new, settings mínimas. Ya existen comandos de documento/recents y rutas `/` `/workspace`.
- **L3 (ejecución, 2026-11-24).** Se cableó `StartScreen` DS a comandos Tauri reales (new/open/recent/settings). Se agregó backend de settings (`get_settings`/`update_settings`, JSON en app data) y ruta `/settings` mínima con theme. Verificados biome, svelte-check, cargo check --workspace, cargo test src-tauri, vitest.
