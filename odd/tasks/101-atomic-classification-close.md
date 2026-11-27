# 101 — Cerrar la clasificación atómica: los cinco sin clasificar

Los cinco componentes que el tablero cuenta como «sin clasificar» pasan a su capa real, y `WorkbenchShell` deja de ser un componente para volverse la página de su route. Al cerrar, el tablero tiene que mostrar cero sin clasificar porque los archivos están donde corresponde, no porque el badge los esconda.

## Surfaces

- `src/lib/components/atoms/Glyph.svelte`
- `src/lib/components/atoms/index.ts`
- `src/lib/components/settings/Glyph.svelte`
- `src/lib/components/settings/SettingsDialog.svelte`
- `src/lib/components/settings/SettingsDialog.test.ts`
- `src/lib/components/settings/index.ts`
- `src/lib/components/organisms/SettingsDialog.svelte`
- `src/lib/components/organisms/SettingsDialog.test.ts`
- `src/lib/components/organisms/BoardCanvas.svelte`
- `src/lib/components/organisms/StartScreen.svelte`
- `src/lib/components/organisms/index.ts`
- `src/lib/components/start-screen/StartScreen.svelte`
- `src/lib/components/start-screen/index.ts`
- `src/lib/components/workbench/BoardCanvas.svelte`
- `src/lib/components/workbench/WorkbenchShell.svelte`
- `src/lib/components/workbench/WorkbenchShell.test.ts`
- `src/lib/components/workbench/index.ts`
- `src/routes/workbench/+page.svelte`
- `src/routes/workbench/page.test.ts`
- `src/routes/+layout.svelte`
- `src/routes/+page.svelte`
- `src/routes/page.test.ts`
- `src/lib/ds/inventory.generated.ts`
- `src/lib/ds/catalog.ts`
- `src/lib/ds/catalog.test.ts`
- `src/lib/ds/inventory.test.ts`
- `docs/SYSTEM_DESIGN_CAMCRAFT.md`
- `odd/tasks/101-atomic-classification-close.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «ahora para la siguiente tarea vamos a terminar de clasificar los que pone que estan sin clasificar» — y la lista literal que acompañó el pedido: `src/lib/components/settings/Glyph.svelte`, `src/lib/components/settings/SettingsDialog.svelte`, `src/lib/components/start-screen/StartScreen.svelte`, `src/lib/components/workbench/BoardCanvas.svelte`, `src/lib/components/workbench/WorkbenchShell.svelte`.
- **S2 (decisión del owner sobre la página, verbatim).** «La página va al route (más fiel al skill)» — «WorkbenchShell se muda a src/routes/workbench/+page.svelte. Cuatro capas, cero sin clasificar, y el skill no se toca.»
- **S3 (la clasificación es estructural).** La capa se decide por la carpeta en la que vive el archivo, como ya lo hace `src/lib/ds/inventory.ts`. No se agrega un mapa de overrides por archivo ni se declara la capa en el catálogo: si el archivo no está en su carpeta, el tablero lo tiene que seguir mostrando como sin clasificar.
- **S4 (nada de cobertura inventada).** Los cuatro componentes que siguen siendo componentes ganan ficha en el catálogo; los que no se pueden renderizar en aislamiento (`SettingsDialog` escribe settings, `BoardCanvas` necesita canvas y store, `StartScreen` monta el flujo de documento nuevo si el writer confirma que necesita el host) se marcan `renderable: false` con su razón, como ya hace el resto del catálogo.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                   | Specs  | Ruta   | Commit |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------ | ------ |
| T1  | `Glyph` a `atoms/`, `SettingsDialog` y `BoardCanvas` a `organisms/`, `StartScreen` a `organisms/` (es un bloque de estado con props, no un template con slots): archivos, tests hermanos, barrels `index.ts` de cada capa y todos los importadores, incluidas las rutas | S1, S3 | worker |        |
| T2  | `WorkbenchShell` deja de ser componente: su cuerpo es `src/routes/workbench/+page.svelte` y su test pasa a `src/routes/workbench/page.test.ts` (nombre sin `+`, que SvelteKit reserva); el barrel `workbench/index.ts` deja de exportarlo y queda sólo el store         | S2, S3 | worker |        |
| T3  | Catálogo e inventario: fichas nuevas para los cuatro componentes que siguen siéndolo, con `renderable: false` y razón donde corresponda; `pnpm ds:inventory` regenerado; tests del tablero actualizados para exigir **cero sin clasificar** y las fichas nuevas         | S1, S4 | worker |        |
| T4  | Este documento, su fila en el índice y la nota de `docs/SYSTEM_DESIGN_CAMCRAFT.md` §18 si menciona conteos que cambian                                                                                                                                                  | S1, S2 | worker |        |

## Log

- **L1 (pedido original, verbatim).**

  ```
  ahora para la siguiente tarea vamos a terminar de clasificar los que pone que estan sin clasificar

  Sin clasificar (5)
  src/lib/components/settings/Glyph.svelte
  src/lib/components/settings/SettingsDialog.svelte
  src/lib/components/start-screen/StartScreen.svelte
  src/lib/components/workbench/BoardCanvas.svelte
  src/lib/components/workbench/WorkbenchShell.svelte
  ```

- **L2 (clasificación medida por el padre, con el gate del skill `atomic-design`).** `Glyph` (40 líneas) es una primitiva visual única → átomo; existe aparte sólo porque su tarea no podía editar `atoms/icons.ts`. `SettingsDialog` (523) es un bloque de feature con estado que lee y escribe settings y tema → organismo. `BoardCanvas` (382) lee y escribe el store `workbench.svelte` y monta three.js → organismo. `StartScreen` (302) recibe props y tiene `$state` local de dimensiones; la ruta `+page.svelte` es la que hace `invoke`, recents y settings, así que no llena slots: organismo, no template. `WorkbenchShell` (162) monta `WorkbenchTemplate` con snippets reales y navega con `goto('/')`: cuerpo de página, y el skill dice que las Pages viven en `src/routes/`.

- **L3 (decisión del owner).** Ante las tres alternativas —llevar la página al route, agregar una quinta capa `components/pages/`, o un mapa declarado de capas sin mover archivos— el owner eligió la primera: «La página va al route (más fiel al skill)». Consecuencia aceptada: `src/routes/workbench/+page.svelte` deja de ser un shim de dos líneas y pasa a contener la composición de la shell.
