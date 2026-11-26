# 090 — Fase 4: settings con paridad DS

Alinear la pantalla de settings al DS `design/camcraft-workbench-v2/04-settings.html`, manteniendo solo settings reales soportadas.

## Surfaces

- `src/routes/settings/+page.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/090-tauri-fase4-settings-ds.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «si, dale palante papa»

## Tasks

| ID  | Tarea                                                  | Specs | Ruta   | Commit    |
| --- | ------------------------------------------------------ | ----- | ------ | --------- |
| T1  | Aplicar layout DS de settings (dialog/sidebar/footer)  | S1    | inline | `45a41fe` |
| T2  | Conectar settings reales (theme) sin inventar opciones | S1    | inline | `45a41fe` |
| T3  | Verificar, RDD, ODD/Engram, commit y push              | S1    | inline | `45a41fe` |

## Log

- **L1 (pedido original, verbatim).** «si, dale palante papa»
- **L2 (contexto).** Referencia DS: `design/camcraft-workbench-v2/04-settings.html`. Mantener solo settings reales (theme) soportadas por Tauri.
- **L3 (ejecución, 2026-11-24).** Settings rediseñada con layout DS (dialog/sidebar/footer/backdrop) y solo opción real soportada: theme. `pnpm check` OK.
- **L4 (cierre, 2026-11-24).** Commit `45a41fe`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
