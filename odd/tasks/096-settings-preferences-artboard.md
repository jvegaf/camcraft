# 096 — Settings implementado desde el artboard de preferencias

Llevar el artboard `design/camcraft-workbench-v2/04-settings.html` al stack real (Tauri 2 + Svelte 5 + Panda CSS): la ventana de Preferencias con búsqueda en el titlebar, lista de páginas, filas label+hint/control y footer, construida sobre los átomos y moléculas del design system en lugar de copiar el HTML del artboard.
Vive en `src/lib/components/settings/` y es la pantalla que escribe settings, a través de `SettingsDto` (`src-tauri/src/lib.rs:93-104`).

## Surfaces

- `src/lib/components/settings/SettingsDialog.svelte`
- `src/lib/components/settings/SettingsDialog.test.ts`
- `src/lib/components/settings/Glyph.svelte`
- `design/camcraft-workbench-v2/handoffs.json`

## Specs

- **S1 (pedido original, verbatim).** «Implement the design artboard design/camcraft-workbench-v2/04-settings.html» — «Artboard: design/camcraft-workbench-v2/04-settings.html» — «Title: Settings» — «Tradeoff: Patrón SolidCraft de preferencias (page list, búsqueda, footer) adaptado a Camcraft; claro y escalable, pero hay que validar qué opciones son reales antes de implementarlas.»
- **S2 (stack real y mapeo, verbatim).** «Implement it in the project's real stack. Use the artboard as the visual spec: map its parts to the project's existing components and styles, and do not copy the artboard HTML verbatim.»
- **S3 (reporte del handoff, verbatim).** «Report progress with the gentle_design tool: action "handoff", target "camcraft-workbench-v2/04-settings.html", status "in_progress" when you start; status "implemented" when it is done, with previewUrl set to the running app's loopback URL (localhost, 127.0.0.1 or [::1]) when one is running, so the canvas can compare it with the artboard.»
- **S4 (tamaño ODD, verbatim).** «This design is not linked to an ODD feature document: size it under the project's ODD rules as usual.»
- **S5 (tokens, verbatim).** «Take colors, typography, spacing, radius and shadows from design/system.md, the project's token source.»
- **S6 (sin commits del handoff, verbatim).** «This handoff itself commits nothing.»
- **S7 (instrucción posterior del owner, verbatim; reemplaza la cláusula de S6 sobre commits).** «sincroniza odd y engram y comitea todo»

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | Specs      | Ruta   | Commit    |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | --------- |
| T1  | `SettingsDialog.svelte` reconstruido sobre átomos y moléculas: `CommandSearch` real que filtra filas por label/hint con estado explícito de sin resultados, `⌘F`/`Ctrl+F` que la enfoca con el diálogo abierto, lista de páginas renderizada desde `SETTINGS_GROUPS` con `aria-current="page"` sobre la recipe `panelRow`, control de tema segmentado con `aria-pressed` y tokens, footer con el átomo `Button` y la primaria sobre la señal de corte, Escape y backdrop cierran sin escribir, línea de error con `aria-live`, guarda de reduced-motion; CSS propio de botones/selects eliminado y el `css()` local queda en layout. `Glyph.svelte` nuevo para la cruz de cerrar porque `atoms/icons.ts` no la tiene y está fuera de alcance. Tests: 5 conservados (3 adaptados al control segmentado), 10 agregados | S1, S2, S5 | worker | `e5cde25` |
| T2  | Handoff de `gentle_design` reportado `in_progress` al empezar e `implemented` con `previewUrl` `http://127.0.0.1:5173/`, registrado en `design/camcraft-workbench-v2/handoffs.json`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | S3         | parent | `7defe77` |
| T3  | Este documento y su fila en `odd/tasks/INDEX.md`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | S4, S6, S7 | worker | `ccfe025` |

## Log

- **L1 (pedido original, verbatim).**

  ```
  Implement the design artboard design/camcraft-workbench-v2/04-settings.html

  Artboard: design/camcraft-workbench-v2/04-settings.html
  Title: Settings
  Tradeoff: Patrón SolidCraft de preferencias (page list, búsqueda, footer) adaptado a Camcraft; claro y escalable, pero hay que validar qué opciones son reales antes de implementarlas.

  Implement it in the project's real stack. Use the artboard as the visual spec: map its parts to the project's existing components and styles, and do not copy the artboard HTML verbatim.
  Take colors, typography, spacing, radius and shadows from design/system.md, the project's token source.

  Report progress with the gentle_design tool: action "handoff", target "camcraft-workbench-v2/04-settings.html", status "in_progress" when you start; status "implemented" when it is done, with previewUrl set to the running app's loopback URL (localhost, 127.0.0.1 or [::1]) when one is running, so the canvas can compare it with the artboard.
  This design is not linked to an ODD feature document: size it under the project's ODD rules as usual.
  This handoff itself commits nothing.
  ```

- **L2 (instrucción posterior del owner, verbatim).** «sincroniza odd y engram y comitea todo»
- **L3 (decisión de tamaño, 2026-11-26).** Tarea **small** bajo las reglas ODD del proyecto: un componente y sus tests, el test de resume pasa, ningún ítem de alto riesgo (no toca comandos Tauri, migraciones, contratos de archivo ni el shell). Por eso **no se creó un documento de feature por adelantado** (S4): este registro se escribió después, por instrucción explícita del owner (S7).
- **L4 (restricción de producto y su fuente).** `SettingsDto` en `src-tauri/src/lib.rs:93-104` persiste exactamente un campo, `theme`; el resto de la estructura no existe en el backend. Las tareas anteriores ya fijaron esa línea: «Conectar settings reales (theme) sin inventar opciones» ([090-tauri-fase4-settings-ds.md](090-tauri-fase4-settings-ds.md), T2) y «Mantener solo settings reales (theme) soportadas por Tauri» ([091-tauri-fase4-settings-ds-fine.md](091-tauri-fase4-settings-ds-fine.md), L2). En consecuencia las demás páginas del artboard (Layout, Canvas, Manufactura, Librería de herramientas, Importar) y sus filas de select/checkbox **no se renderizan**: una entrada de navegación o un toggle que no guarda nada prometería lo que el backend no puede cumplir.
- **L5 (evidencia TDD, 2026-11-26, verbatim).** RED: `npx vitest run src/lib/components/settings/` → `Tests 12 failed | 3 passed (15)` antes de la implementación. GREEN: el mismo comando → `Test Files 1 passed (1)`, `Tests 15 passed (15)`. Suite completa: `npx vitest run` → `23 passed (23)` archivos, `125 passed (125)` tests. Tipos: `npx svelte-check --tsconfig ./tsconfig.json` → `0 errors and 0 warnings` (igual que la línea base). Lint: `npx biome check src/lib/components/settings/` → limpio. Extra: `npx vitest run src/routes/page.test.ts` → `5 passed (5)`. El cambio de página está implementado pero **no tiene test** porque el contrato produce una sola página; lo que lo fija es `renders one page per persisted setting: theme only`.
- **L6 (decisión).** El tema **no** se previsualiza: se aplica sólo cuando Aplicar lo persiste, igual que el contrato anterior. El control segmentado edita un borrador y el footer lo dice, en lugar de guardar una preferencia sin confirmar.
- **L7 (cierre: commit y atribución de rutas, 2026-11-26).** El commit que sostiene T3 es `ccfe025` («docs(odd): record task 096 settings preferences artboard»), ya en la rama. Las rutas de T1 y T2 no se deducen de los commits — los tres comparten el mismo autor, así que un commit no distingue quién lo ejecutó — sino de cómo ocurrió el trabajo: el diálogo de preferencias y sus tests los escribió un writer delegado (`worker`) y los reportes de handoff de `gentle_design` los hizo la sesión parent (`parent`). Este registro no afirma nada más allá de eso.
