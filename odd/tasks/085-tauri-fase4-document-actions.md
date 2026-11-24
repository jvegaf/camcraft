# 085 — Fase 4: acciones de documento en shell

Cablear acciones y estado de documento real en TopBar, ToolRail y Browser del workbench.

## Surfaces

- `src/lib/components/workbench/workbench.svelte.ts`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `src/lib/components/workbench/TopBar.svelte`
- `src/lib/components/workbench/ToolRail.svelte`
- `src/lib/components/workbench/BrowserPanel.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/085-tauri-fase4-document-actions.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Extender store con document status y refresh | S1 | inline | `e03a2da` |
| T2 | Cablear TopBar/ToolRail/Browser a acciones Tauri reales | S1 | inline | `e03a2da` |
| T3 | Verificar, RDD, ODD/Engram, commit y push | S1 | inline | `e03a2da` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 4 de `docs/MIGRATION_TAURI.md`: migración por paneles, flujos principales sin egui.
- **L3 (ejecución, 2026-11-24).** Store extendido con document status; WorkbenchShell carga estado y maneja open/save/save-as/settings/back; TopBar/ToolRail/Browser muestran datos y acciones reales. Verificado `pnpm check`.
- **L4 (cierre, 2026-11-24).** Commit `e03a2da`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
