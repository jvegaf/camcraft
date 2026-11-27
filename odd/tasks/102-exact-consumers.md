# 102 — Consumidores exactos: resolver el binding nombrado a través del barrel

El inventario acredita como consumidor de un componente a cualquier archivo que importe su barrel, porque al resolver `import { Button } from '../atoms'` descarta qué binding se trajo y termina creditando a los seis átomos. Esta tarea hace que el nombre importado decida el destino, y que el tablero muestre uso real —incluidos los componentes que queden en cero.

## Surfaces

- `src/lib/ds/inventory.ts`
- `src/lib/ds/inventory.test.ts`
- `src/lib/ds/inventory.generated.ts`
- `scripts/ds-inventory.mjs`
- `src/lib/ds/catalog.test.ts`
- `src/routes/ds/index.test.ts`
- `src/routes/ds/[slug]/sheet.test.ts`
- `docs/SYSTEM_DESIGN_CAMCRAFT.md`
- `odd/tasks/102-exact-consumers.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «me parece bien afinalo», sobre la deuda reportada al cerrar la 101: «el inventario acredita consumidores **por barrel** — semántica preexistente que hace que exportar `Glyph` desde `atoms/index.ts` cuente a `ToolRail` y `TopBar` como sus consumidores. Afinar eso es otra tarea sobre `inventory.ts`, si te interesa.»
- **S2 (el uso se decide por el binding, no por el barrel).** Un archivo es consumidor de un componente sólo si el import que lo alcanza nombra ese componente, o si importa el módulo entero de forma que cualquiera podría usarse (`import * as`, `import './x.svelte'` de efecto, `export * from`). Importar un barrel sin nombrar el componente no acredita a todos sus miembros.
- **S3 (la verdad incómoda se muestra).** Si con la regla nueva un componente queda sin consumidores, el tablero lo dice: no se agrega un fallback, un «probablemente usado» ni un conteo por barrel para que los números queden lindos. El resultado es el dato, y lo que aparezca en cero se reporta como hallazgo.
- **S4 (la lógica sigue siendo pura).** La resolución de bindings vive en `src/lib/ds/inventory.ts` y se prueba con datos sintéticos, sin tocar el filesystem; `scripts/ds-inventory.mjs` sólo lee y entrega nombres.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                                                          | Specs      | Ruta   | Commit                                                            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ------ | ----------------------------------------------------------------- |
| T1  | `inventory.ts`: el modelo de import pasa de `string[]` de specifiers a entradas con `bindings: string[] \| null` (null = módulo entero), y un `BarrelFile` que mapea nombre exportado → specifier. La resolución acredita sólo los componentes nombrados; `import * as`, side-effect, default de un `.svelte` y `export * from` conservan la semántica actual. Los módulos de tipos (`export type ... from`) no acreditan nada | S1, S2, S4 | worker | `0703163`                                                         |
| T2  | `scripts/ds-inventory.mjs` extrae el binding de cada import y export, incluidos `export { default as X } from`, `export * from`, `import type` y alias (`import { Button as B }`), y pasa la estructura nueva. Inventario regenerado; determinismo verificado                                                                                                                                                                  | S1, S4     | worker | `0703163`                                                         |
| T3  | Tests: casos sintéticos de binding nombrado, alias, namespace, side-effect, `export *`, tipo y default, más los conteos derivados del repo real. Actualizar lo que pinneen `catalog.test.ts`, `ds/index.test.ts` y `sheet.test.ts`, y la regla en `docs/SYSTEM_DESIGN_CAMCRAFT.md` §18                                                                                                                                         | S1, S2, S3 | worker | `4e792ed`                                                         |
| T4  | Este documento, su fila en el índice, y el reporte de qué componentes quedaron con cero consumidores para el owner                                                                                                                                                                                                                                                                                                             | S1, S3     | worker | —, este documento (`git log -- odd/tasks/102-exact-consumers.md`) |

## Log

- **L1 (pedido original, verbatim).**

  ```
  me parece bien afinalo
  ```

  Responde al cierre de la 101, que dejó anotado que el inventario acredita consumidores por barrel.

- **L2 (causa raíz, medida en el código por el padre).** `resolveImportTargets` recibe el specifier pelado, y `scripts/ds-inventory.mjs:specifiersOf` extrae sólo el texto entre comillas de `from '...'`. El `BarrelFile` guarda `imports: string[]`, sin nombres. Así, al resolver `import { Button, Icon } from '../atoms'` el barrel devuelve sus seis miembros y el importador queda acreditado como consumidor de `Glyph`, `KeyHint` y `StatusDot` sin haberlos nombrado. El comportamiento está documentado como intencional en el comentario de `resolveImportTargets`, y es lo que esta tarea corrige.

- **L3 (modelo decidido durante la implementación).** El `BarrelFile` guarda `exports: { name, local, specifier, typeOnly }[]` en vez de un mapa nombre→specifier: conservar `local` es lo que hace exactos los barrels anidados (`export { Nested } from '../molecules'` resuelve ese nombre contra el barrel interno, no el módulo entero). Lo type-only es un campo propio y no `bindings: []`, para que `import { Button, type IconName }` descarte sólo el tipo. Un binding que el barrel no mapea cae al `export *` de ese barrel si existe; si no, no acredita nada.
- **L4 (hallazgo, y es el contrario del esperado).** Ningún componente queda en cero consumidores: la inflación del barrel hacía crecer las listas, no la veredicto de uso. Los 20 componentes siguen consumidos, con listas más chicas — `KeyHint`, `StatusDot` y `PanelRow` bajan de 4 a 1 (los nombra un solo archivo: `CommandSearch`, `PanelRow` y `StatusDot` respectivamente). Las rutas de renderizado en cero quedaron probadas igual, con una entrada puesta a cero a mano en el test y restaurada en `finally`, porque el repo real no tiene ningún caso.
- **L5 (límites del extractor, declarados en el script).** `import()` dinámico, `require`, sentencias cuyo keyword no arranca la línea y `export * as ns from` no se leen: sub-acreditan, nunca inventan un consumidor. La única sobre-acreditación que queda es un default import de un barrel `.ts`, que se reporta como módulo entero; esa forma no existe hoy en el repo.
- **L6 (evidencia medida).** RED semántico contra el `inventory.ts` de HEAD: nombrar sólo `A` a través del barrel seguía acreditando a `B`. GREEN: `inventory.test.ts` 19/19, suite completa 28 archivos / 173 tests (eran 160, ninguno perdido), `svelte-check` 0 errores, `build` verde, `lint` limpio, `pnpm ds:inventory` determinista (mismo sha256 en dos corridas). El inventario confirma el efecto de la 101: `WorkbenchTemplate` ahora tiene un solo consumidor, `src/routes/workbench/+page.svelte`.
