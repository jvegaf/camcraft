# 088 — Fase 4: paridad DS Floating Studio

Ajustar shell/workbench y start screen a paridad de estilos/componentes con `design/camcraft-workbench-v2/02-floating-studio.html`.

## Surfaces

- `src/lib/components/start-screen/StartScreen.svelte`
- `src/lib/components/workbench/TopBar.svelte`
- `src/lib/components/workbench/ToolRail.svelte`
- `src/lib/components/workbench/ViewportHud.svelte`
- `src/lib/components/workbench/StatusBar.svelte`
- `src/lib/components/workbench/BrowserPanel.svelte`
- `src/lib/components/workbench/InspectorPanel.svelte`
- `src/lib/components/workbench/FloatCard.svelte`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/088-tauri-fase4-ds-parity.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «el problema que teniamos en egui es que no estaba actualizada la ui con los nuevos requerimientos que teniamos, por lo que no le vamos a dar mas coba. tenemos que centrarnos en la paridad de estilos y componentes con el ui system design de @design/camcraft-workbench-v2/02-floating-studio.html»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Auditar y ajustar TopBar/ToolRail/ViewportHud/StatusBar a DS | S1 | inline | `212d4b3` |
| T2 | Ajustar Browser/Inspector/FloatCard y StartScreen a DS | S1 | inline | `906ec14` |
| T3 | Verificar, RDD, ODD/Engram, commit y push | S1 | inline | `906ec14` |

## Log

- **L1 (pedido original, verbatim).** «el problema que teniamos en egui es que no estaba actualizada la ui con los nuevos requerimientos que teniamos, por lo que no le vamos a dar mas coba. tenemos que centrarnos en la paridad de estilos y componentes con el ui system design de @design/camcraft-workbench-v2/02-floating-studio.html»
- **L2 (contexto).** Referencia DS: `design/camcraft-workbench-v2/02-floating-studio.html`. Priorizar paridad visual/estructural, manteniendo acciones reales ya cableadas.
- **L3 (ejecución, 2026-11-24).** TopBar/ToolRail/ViewportHud/StatusBar ajustados a DS (kbd ⌘K, layout pill, iconos, estado/severidad, labels 3D). StartScreen ya alineado en textos clave. Verificado `pnpm check`.
- **L4 (cierre parcial, 2026-11-24).** Commit `212d4b3`. RDD assess: not-due (under_budget, medium). Queda T2/T3: afinar Browser/Inspector/FloatCard contra DS y cierre final.
- **L5 (ejecución, 2026-11-24).** FloatCard soporta `headAction`; BrowserPanel agrega botón de búsqueda en header como DS. Inspector/StartScreen verificados cercanos al DS. `pnpm check` OK.
- **L6 (cierre, 2026-11-24).** Commits `212d4b3`, `906ec14`. RDD assess not-due (under_budget/passive). ODD/Engram sincronizados; push a `origin/feat/tauri`.
