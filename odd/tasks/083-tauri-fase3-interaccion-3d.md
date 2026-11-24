# 083 — Fase 3: interacción 3D

Agregar picking/hover/selección en viewport 3D y sincronizar selección con UI.

## Surfaces

- `src/lib/components/workbench/BoardCanvas.svelte`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `src/lib/components/workbench/StatusBar.svelte`
- `src/lib/components/workbench/InspectorPanel.svelte`
- `src-tauri/src/lib.rs`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/083-tauri-fase3-interaccion-3d.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Picking/hover en viewport 3D (raycast Three.js) | S1 | inline | `16199a5` |
| T2 | Selección sincronizada con UI (status/inspector) | S1 | inline | `16199a5` |
| T3 | Verificar, RDD, ODD/Engram, commit y push | S1 | inline | `16199a5` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 3 de `docs/MIGRATION_TAURI.md`: picking/selección/hover, overlays/gizmos mínimos, selección sincronizada con UI.
- **L3 (ejecución, 2026-11-24).** BoardCanvas agrega raycast Three.js para hover/click sobre surface mesh, overlay highlight (hover cian, selección naranja) y HUD con triángulo/profundidad media seleccionada. Verificado `pnpm check`.
- **L4 (cierre, 2026-11-24).** Commit `16199a5`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
