# 097 — Galería del design system en `/ds`

Catálogo visual del Atomic Design dentro de la app, dev-only: ver los átomos, moléculas, organismos y templates que ya existen en `src/lib/components/`, con la matriz de props y estados de cada uno, más un tablero que convierte el "progreso del atomic design" en números derivados del repositorio en vez de una sensación.
Reemplaza la pregunta por Storybook: no se agrega un segundo pipeline de build ni dependencias nuevas.

## Surfaces

- `scripts/ds-inventory.mjs`
- `src/lib/ds/inventory.ts`
- `src/lib/ds/inventory.generated.ts`
- `src/lib/ds/catalog.ts`
- `src/routes/ds/+page.svelte`
- `src/routes/ds/+layout.ts`
- `src/routes/ds/[slug]/+page.svelte`
- `package.json`
- `vitest.config.ts`
- `docs/SYSTEM_DESIGN_CAMCRAFT.md`
- `odd/tasks/097-design-system-gallery.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «necesitamos un storybook poder visualizar el progreso del atomic design, ver los atomos, moleculas, organismos... que te parece https://storybook.js.org»
- **S2 (alternativa elegida por el owner, verbatim).** «Ruta /ds dev-only en la app (recomendado)» — «Galería por capa + matriz de props/estados + tablero de progreso, con los tokens reales de Panda. Cero dependencias nuevas, Risk: none.»
- **S3 (dev-only, verbatim).** «Ruta /ds dev-only en la app (recomendado)» — el catálogo no existe en el build de producción: `pnpm build` sigue verde y `/ds` responde 404 fuera de `pnpm dev`.
- **S4 (sin dependencias nuevas, verbatim).** «Cero dependencias nuevas» — no se instala Storybook, ni `@storybook/*`, ni un framework/build alternativo; la galería corre sobre Vite + SvelteKit + Panda ya presentes.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                            | Specs      | Ruta   | Commit |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | ------ |
| T1  | Inventario derivado del filesystem: `scripts/ds-inventory.mjs` + `src/lib/ds/inventory.ts` (función pura que clasifica capa, detecta test hermano y consumidores) y `src/lib/ds/inventory.generated.ts`; script `pnpm ds:inventory`; tests sobre un fixture de directorio                                                                 | S1, S2, S4 | worker |        |
| T2  | Manifest del catálogo (`src/lib/ds/catalog.ts`) y ficha por componente (`src/routes/ds/[slug]/+page.svelte`): matriz de variantes/tamaños/estados renderizada con los componentes reales y switch de tema sobre la condición `data-theme` de Panda                                                | S2, S4     | worker |        |
| T3  | Índice por capa atómica y tablero de progreso (`src/routes/ds/+page.svelte`, `src/routes/ds/+layout.ts`), con la guarda dev-only y su test                                                                                     | S1, S2, S3 | worker |        |
| T4  | Este documento, su fila en `odd/tasks/INDEX.md` y la nota de uso del catálogo en `docs/SYSTEM_DESIGN_CAMCRAFT.md`                                                                                                                                                                                                                 | S2, S4     | worker |        |

## Log

- **L1 (pedido original, verbatim).**

  ```
  necesitamos un storybook poder visualizar el progreso del atomic design, ver los atomos, moleculas, organismos... que te parece https://storybook.js.org
  ```

- **L2 (decisión del owner, verbatim).** Ante la pregunta «¿Cómo querés el catálogo visual del Atomic Design en Camcraft?» el owner eligió: «Ruta /ds dev-only en la app (recomendado)» — «Galería por capa + matriz de props/estados + tablero de progreso, con los tokens reales de Panda. Cero dependencias nuevas, Risk: none.»

- **L3 (diagnóstico del padre, evidencia medida).** Estado del repositorio al abrir la tarea: 26 archivos `.svelte`, de los cuales 13 están bajo `src/lib/components/{atoms,molecules,organisms,templates}` — el método ya está aplicado y falta el instrumento para verlo. Superficies visuales existentes: los artboards HTML de `design/camcraft-workbench-v2/` y los PNG canónicos de `fixtures/ui` (skill `ui-visual-review`); Storybook habría sido una tercera superficie de drift, además de exigir compatibilidad con Vite 8, glue propio con `styled-system` y un decorator para la condición `data-theme`. Descartado por eso, no por gusto.

- **L4 (forma técnica decidida por el padre).** El progreso no se lee del filesystem en runtime (el build es SPA con `adapter-static` y fallback): se genera en build-time a un módulo versionado, `src/lib/ds/inventory.generated.ts`, con `pnpm ds:inventory`. "Estados cubiertos" sale de cruzar el manifest con hechos del repositorio (archivo del componente, test hermano, consumidores), nunca de una declaración optimista.
