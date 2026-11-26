# 095 — Floating Studio implementado con Atomic Design

Llevar el artboard `design/camcraft-workbench-v2/02-floating-studio.html` al stack real (SvelteKit 5 + Panda CSS + Tauri 2): marco flotante del shell, paridad de chrome y la IA de paneles por objetos CAM (Board, Setup, Operaciones, Salida). La UI se descompone y reorganiza según `.agents/skills/atomic-design/SKILL.md`.

## Surfaces

- `src/lib/components/atoms/` (nuevo)
- `src/lib/components/molecules/` (nuevo)
- `src/lib/components/organisms/` (nuevo)
- `src/lib/components/templates/` (nuevo)
- `src/lib/components/workbench/` (existente: se migra hacia arriba)
- `src/lib/components/start-screen/StartScreen.svelte`
- `src/lib/components/settings/SettingsDialog.svelte`
- `src/lib/ds/camcraft.preset.ts`
- `src/routes/+layout.svelte`, `src/routes/+page.svelte`, `src/routes/workbench/+page.svelte`, `src/routes/workspace/+page.svelte`
- `src/lib/components/**/*.test.ts`
- `odd/tasks/095-floating-studio-atomic.md`, `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «Implementar 02-floating-studio»
- **S2 (alcance elegido por el owner, verbatim).** «Corte 1 + IA de paneles»
- **S3 (pedido original, verbatim).** «ademas creo que deberiamos de desrollar mediante la metodologia de atomic design»
- **S4 (textos del artboard, verbatim de `design/camcraft-workbench-v2/02-floating-studio.html`).** El chrome dice exactamente: «Camcraft», «controller-board.cam», «Buscar comando, capa u operación…», «⌘K», «Floating», «layout», «Importar», «Capas», «Origen», «Generar plan», «2D Board», « · Grid 1 mm», «6 taladros seleccionados», «X 42.80», «Y 31.20», «Z 2.00», «mm · Grid 1 mm · GRBL», «● Sin errores».
- **S5 (contenido de paneles del artboard, verbatim).** Browser: «Browser», «Proyecto», secciones «Board», «Setup», «Operaciones», «Salida»; filas «Top Copper .gbr», «Bottom Copper .gbr», «Drill Hits .drl», «Board Outline .gbr», «Stock FR-4 1.6mm 100×80», «Origen bottom-left X0 Y0», «1 · Profile outline OK», «2 · Drill 0.8 / 1.0 seleccionada», «3 · Isolation top draft», «Post GRBL .nc». Inspector: «Inspector», «Drill 0.8 / 1.0», «Operación», «Tipo», «Drill cycle G81», «Herramienta», «T1 Flat 0.8 mm cambiar», «Z cut», «-1.8 mm», «Feed plunge», «120 mm/min», «Hits», «Herramientas», «T1, T2», «Tiempo», «0:42», «Estado», «Listo para simular. Toolpath válido, sin colisiones detectadas en preview.»
- **S6 (tokens).** Los colores, tipografía, espaciado, radios y sombras salen de `design/system.md`; donde el preset diverge (`camcraft.preset.ts:20-25` da radios 8/10/12 contra 6/8/10 del sistema), gana `design/system.md`.
- **S7 (metodología).** Toda pieza se clasifica antes de existir: átomo sin lógica ni estado, molécula sin Tauri/API, organismo con estado local o store, template con slots, page en `src/routes/`. No se crea una variante paralela de algo que ya existe; estilos en Panda CSS.
- **S8 (no regresión).** `/`, `/workspace` y `/workbench` siguen funcionando; `TopBar`/`StatusBar` son compartidos por la start screen y el shell, y las acciones de archivo (`new_document`, `open_document`, `save_document`, `list_recents`, `document_status`) siguen cableadas.

## Tasks

| ID  | Tarea                                                                                 | Specs      | Ruta   | Commit |
| --- | ------------------------------------------------------------------------------------- | ---------- | ------ | ------ |
| T1  | Átomos y moléculas del chrome (botón, icon-button, icono, dot, chip HUD, celda de status, hint de tecla, panel-section, panel-row, command-search) | S4, S6, S7 | worker | —      |
| T2  | Template `WorkbenchTemplate` con slots: marco con radio/borde/sombra y backdrop de grilla, canvas dentro; shell y rutas migran a él | S1, S4, S8 | worker | —      |
| T3  | Organismos de chrome a paridad: `TopBar` (acciones de archivo en iconos + botón de tema + command search), `ToolRail` (Importar/Capas/Origen/Generar plan primario), `ViewportHud` (2D Board · Grid 1 mm + selección), `StatusBar` (coordenadas mono + unidades/GRBL/health) | S4, S8 | worker | —      |
| T4  | Organismos de contenido CAM: browser con Board/Setup/Operaciones/Salida (roles, conteos, estados) e inspector de operación (Tipo/Herramienta/Z cut/Feed/Hits/Tiempo/Estado), con estado de presentación en `workbench.svelte.ts` derivado de lo que ya existe | S5, S7 | worker | —      |
| T5  | Convergencia de tokens, migración de rutas y cierre: `pnpm check`, `pnpm test`, `pnpm build`, verificación independiente y review | S6, S8 | inline | —      |

## Log

- **L1 (pedido original, verbatim).** «Implementar 02-floating-studio»
- **L2 (alcance, verbatim).** «Corte 1 + IA de paneles»
- **L3 (pedido original, verbatim).** «ademas creo que deberiamos de desrollar mediante la metodologia de atomic design»
- **L4 (mapa de partida, 2026-11-26).** Los paneles ya flotan: `FloatCard.svelte:54-60` es `position:absolute` con `z-index:20` y blur; el shell es un stack de absolutos (`WorkbenchShell.svelte:124-161`) sin radio, borde ni sombra. La suite actual (`workbench.svelte.test.ts:10-30`) sólo cubre `workbenchState`: ningún test prueba el render del shell, por eso el riesgo es alto en el ítem 6 y T1–T4 van test-first con render en jsdom.
- **L5 (acoplamientos observados).** `TopBar`/`StatusBar` se comparten con `src/routes/+page.svelte:106-123`; el shell se monta en `/workbench` y `/workspace`; los handlers Tauri viven en `WorkbenchShell.svelte:30-125` y deben sobrevivir a la migración al template. `BoardCanvas` mantiene su propio `ResizeObserver` (`:259-273`), así que insetar el viewport no rompe el encuadre.
- **L6 (decisión).** El «2D Board» del artboard no se construye: el viewport sigue siendo el canvas 3D real y su HUD adopta el rótulo del artboard. Queda fuera de este documento como trabajo futuro.
