# 087 — Fase 4: recents en shell y smoke checks

Integrar recientes en el workbench y ejecutar smoke checks no-GUI de la migración.

## Surfaces

- `src/lib/components/workbench/workbench.svelte.ts`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `src/lib/components/workbench/BrowserPanel.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/087-tauri-fase4-recents-smoke.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID  | Tarea                                                 | Specs | Ruta   | Commit    |
| --- | ----------------------------------------------------- | ----- | ------ | --------- |
| T1  | Recents en store/BrowserPanel y refresh tras acciones | S1    | inline | `3083342` |
| T2  | Smoke checks no-GUI y cierre ODD/Engram/push          | S1    | inline | `3083342` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 4: integrar recents/settings en shell y hacer smoke test guiado.
- **L3 (ejecución, 2026-11-24).** Store/WorkbenchShell/BrowserPanel muestran recents y refrescan tras open/save/new/openRecent. Smoke checks no-GUI: `pnpm check` OK, `cargo check --manifest-path src-tauri/Cargo.toml` OK, `cargo test --manifest-path src-tauri/Cargo.toml` 6 passed.
- **L4 (cierre, 2026-11-24).** Commit `3083342`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
