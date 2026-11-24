# 091 — Fase 4: ajuste fino settings DS

Ajustar detalles visuales/componentes de settings para mayor paridad con `04-settings.html`.

## Surfaces

- `src/routes/settings/+page.svelte`
- `odd/tasks/091-tauri-fase4-settings-ds-fine.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «todavia le falta ajustar estilos y componentes para hacer paridad con @design/camcraft-workbench-v2/04-settings.html»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Ajustar titlebar/search/sidebar/footer a DS | S1 | inline | `a96532b` |
| T2 | Verificar, RDD, ODD/Engram, commit y push | S1 | inline | `a96532b` |

## Log

- **L1 (pedido original, verbatim).** «todavia le falta ajustar estilos y componentes para hacer paridad con @design/camcraft-workbench-v2/04-settings.html»
- **L2 (ejecución, 2026-11-24).** Agregados search visual ⌘F, icono en sidebar activo, footer con Restaurar valores/Cancelar/Aplicar, estructura más cercana al DS. Solo setting real: theme. `pnpm check` OK.
- **L3 (cierre, 2026-11-24).** Commit `a96532b`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
