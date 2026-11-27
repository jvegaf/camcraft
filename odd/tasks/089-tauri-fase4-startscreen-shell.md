# 089 — Fase 4: start screen dentro del shell principal

Mostrar start screen con TopBar y StatusBar visibles en la ventana principal.

## Surfaces

- `src/routes/+page.svelte`
- `src/lib/components/start-screen/StartScreen.svelte`
- `odd/tasks/089-tauri-fase4-startscreen-shell.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «es que la start-screen es solo los dos componentes, que se debe mostrar junto con la main window cuando arranca la app, no se si me explico»
- **S2 (pedido original, verbatim).** «solo se ve el baground, junto con los componentees de la start-screen, pero no se ven ni la AppBar ni tampoco la statusbar»

## Tasks

| ID  | Tarea                                                   | Specs  | Ruta   | Commit    |
| --- | ------------------------------------------------------- | ------ | ------ | --------- |
| T1  | Envolver start screen con TopBar/StatusBar en ruta raíz | S1, S2 | inline | `c02988d` |
| T2  | Ajustar StartScreen para embebido en shell              | S1, S2 | inline | `c02988d` |
| T3  | Verificar, RDD, ODD/Engram, commit y push               | S1, S2 | inline | `c02988d` |

## Log

- **L1 (pedido original, verbatim).** «es que la start-screen es solo los dos componentes, que se debe mostrar junto con la main window cuando arranca la app, no se si me explico»
- **L2 (pedido original, verbatim).** «solo se ve el baground, junto con los componentees de la start-screen, pero no se ven ni la AppBar ni tampoco la statusbar»
- **L3 (ejecución, 2026-11-24).** Ruta `/` ahora renderiza TopBar + contenido StartScreen + StatusBar; StartScreen ajustado para ocupar el área de contenido sin borde/radio de shell standalone. `pnpm check` OK.
- **L4 (cierre, 2026-11-24).** Commit `c02988d`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
