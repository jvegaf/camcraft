# 086 — Fase 4: endurecer flujo workbench

Endurecer flujo end-to-end de documento en workbench: estado dirty/path/error consistente y refresh tras acciones.

## Surfaces

- `src/lib/components/workbench/workbench.svelte.ts`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `src/lib/components/workbench/TopBar.svelte`
- `src/lib/components/workbench/StatusBar.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/086-tauri-fase4-workbench-flow-hardening.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID  | Tarea                                                       | Specs | Ruta   | Commit    |
| --- | ----------------------------------------------------------- | ----- | ------ | --------- |
| T1  | Estado de status con severidad y dirty/path visible         | S1    | inline | `b9036f0` |
| T2  | Refresh consistente tras open/save/save-as/new en workbench | S1    | inline | `b9036f0` |
| T3  | Verificar, RDD, ODD/Engram, commit y push                   | S1    | inline | `b9036f0` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 4: endurecer flujo end-to-end en workbench.
- **L3 (ejecución, 2026-11-24).** Store con statusKind ok/error/info; StatusBar colorea por severidad; TopBar muestra dirty y acción Nuevo; WorkbenchShell unifica setStatus tras open/save/save-as/new. Verificado `pnpm check`.
- **L4 (cierre, 2026-11-24).** Commit `b9036f0`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
