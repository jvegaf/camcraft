# 084 — Fase 4: paridad de shell (paneles)

Migrar paneles del shell a estado real compartido del workbench.

## Surfaces

- `src/lib/components/workbench/workbench.svelte.ts`
- `src/lib/components/workbench/BoardCanvas.svelte`
- `src/lib/components/workbench/ViewportHud.svelte`
- `src/lib/components/workbench/InspectorPanel.svelte`
- `src/lib/components/workbench/StatusBar.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/084-tauri-fase4-shell-panels.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID  | Tarea                                               | Specs | Ruta   | Commit    |
| --- | --------------------------------------------------- | ----- | ------ | --------- |
| T1  | Store compartido de workbench y wiring de selección | S1    | inline | `fae81d8` |
| T2  | Inspector/StatusBar/ViewportHud con estado real     | S1    | inline | `fae81d8` |
| T3  | Verificar, RDD, ODD/Engram, commit y push           | S1    | inline | `fae81d8` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 4 de `docs/MIGRATION_TAURI.md`: floating shell, paneles con estado real, migración por slices.
- **L3 (ejecución, 2026-11-24).** Se agregó store `workbench.svelte.ts`; BoardCanvas escribe selección; ViewportHud/InspectorPanel/StatusBar leen estado real. Verificado `pnpm check`.
- **L4 (cierre, 2026-11-24).** Commit `fae81d8`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
