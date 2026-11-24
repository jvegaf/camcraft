# 060 — E1.7 Informe de importación Gerber/Excellon

E1.7: que un archivo Gerber con un constructo inválido importe **el resto** y reporte lo salteado con
su ubicación, sin que una capa con un trazo roto se pierda entera — y sin aflojar el rechazo entero
donde una feature salteada sería una placa mal taladrada.

## Surfaces

- `crates/cam-import/src/report.rs`
- `crates/cam-gerber/`
- `crates/cam-excellon/`

## Specs

- **S1 (pedido original, verbatim).** «me parece bien ese orden que sugieres, hazlo»
- **S2 (política por formato, verbatim, aceptada por el pedido de S1).** «**Gerber**: importar el resto
  y reportar `Skipped { construct, reason, location }` es defendible — perdés un trazo de una capa, lo
  sabés, y sabés dónde. — **Excellon**: no. Una feature salteada es un agujero que no se taladra: la
  placa llega a la máquina incompleta y nada la detiene. Ahí se sigue rechazando el archivo entero, como
  hoy.»
- **S3 (ROADMAP_CAMCRAFT.md E1.7, verbatim).** «| E1.7 | Informe de importación Gerber/Excellon sobre el
  patrón existente | **S** | Una placa con un arco inválido importa el resto y reporta `Skipped` con
  ubicación |»
- **S4 (estado real del patrón, verbatim del código).** `crates/cam-import/src/report.rs:7` ya declara
  `Skipped`, `:22` `ImportReport`, `:36` `pub skipped: Vec<Skipped>`, `:70` `fn skip(...)` y `:96`
  `fn render()`. Lo que falta no es la maquinaria: es que `cam-gerber` y `cam-excellon` la usen.
- **S5 (precedente que hay que igualar, verbatim del código).** El importador de carpeta ya reporta en
  vez de esconder: `crates/cam-import/src/folder.rs:67` documenta que se acumula «geometry this reader
  could not represent faithfully», y `:105` avisa `"{path}: could not be read: {error}"` dejando la capa
  fuera de `imported`. E1.7 baja ese mismo trato del nivel de **capa** al nivel de **constructo** dentro
  de un archivo, añadiendo la ubicación.
- **S6 (decisión que esta tarea NO revierte).** En Excellon, una feature que nombra una herramienta
  ausente de la tabla del archivo sigue rechazando el archivo entero, con el mensaje
  `line N: tool N is not in this file's table`. Decisión tomada y aprobada en el review de la
  049 (`odd/tasks/049-w2c-folder-import.md`): no se saltean features, porque un taladro que no se hace
  no se nota al mirar la pieza. S2 la confirma por formato; no la discute.
- **S7 (aceptación de tres partes, verbatim).** «La aceptación tiene que probar tres cosas: el reporte
  nombra constructo, razón y ubicación; el caso feliz no cambia ni un polígono; y el archivo con
  herramienta indefinida **sigue** siendo rechazo entero.»

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                          | Specs          | Ruta   | Estado | Commit                        |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------- | ------ | ------ | ----------------------------- |
| T1  | Inventariar cada `GerberError::Invalid` de `cam-gerber` y clasificarlo: constructo salteable (un trazo, un flash, un macro que no se puede barrer) o archivo irremediable (formato, unidades, puntero fuera de orden). El inventario queda en el Log                                                                                                                           | S2, S3         | inline | done   | `L5` + `L10`                  |
| T2  | Enhebrar `ImportReport` en el lector Gerber: los constructos salteables se reportan con `skip(construct, reason, location)` y el resto de la capa se evalúa; los irremediables siguen devolviendo `Err`                                                                                                                                                                        | S2, S3, S4, S5 | inline | done   | `c7df894` + `1005888`         |
| T3  | Excellon: dejar el rechazo entero como está y **probar que no se aflojó** (una feature con herramienta indefinida sigue siendo `Err`), más el reporte de lo que sí se puede contar                                                                                                                                                                                             | S6             | inline | done   | `1005888`                     |
| T4  | Aceptación S7 completa: (a) test de un Gerber con un arco inválido que devuelve geometría parcial **y** un `Skipped` con constructo/razón/ubicación; (b) test de que una capa válida produce exactamente los mismos polígonos que antes; (c) test de que el Excellon con herramienta indefinida sigue siendo rechazo. Documentar el cierre y actualizar la fila E1.7 del audit | S3, S7         | inline | done   | `c7df894` + `1005888` + `L10` |

## Log

- **L1 (pedido original, verbatim).** «me parece bien ese orden que sugieres, hazlo»
- **L2 (contexto y origen).** Sale del refresco del audit de la ola 4 (`dev` `766d931`), donde E1.7 quedó
  como el único `⬜` de F1 sin dueño ni ola. El encargo concreto fue decidir su destino; la recomendación
  aceptada fue: hacerla **antes** de la ola que abre F3/E6, porque no toca el kernel ni los goldens y por
  eso no depende de nada de `waves.md` §7. Tamaño declarado por el ROADMAP: **S**.
- **L3 (por qué no es «conectar un struct»).** `ImportReport`/`Skipped` existen y no los usa nadie fuera
  de los importadores DXF/SVG (`grep -rn ImportReport crates/cam-gerber crates/cam-excellon` → 0). El
  trabajo real está en la política: hoy **todos** los sitios `GerberError::Invalid` de
  `crates/cam-gerber/src/lib.rs` (`:198`, `:250`, `:336`, `:342`, `:370`, `:377`, `:418`, …) rechazan el
  archivo entero. Clasificarlos (T1) es la tarea, no el cableado.
- **L4 (riesgo declarado).** `Risk: item 1` — geometría descartada en silencio es un fallo que no se ve en
  la pieza. Por eso S7 exige que el caso feliz quede idéntico y que el rechazo de Excellon siga siendo
  rechazo: la tarea agrega visibilidad, no tolerancia.
- **L5 (T1: inventario y clasificación, sobre `crates/cam-gerber/src/lib.rs` de `96a0899`).** La regla que
  decide no es «qué variante del error es» sino **qué cuesta el error**:
  - **Salteable**: la falla está confinada a la _forma de un constructo_ y el estado de dibujo queda
    determinado después de él (puntero, polaridad, apertura seleccionada, pertenencia a región). Se
    pierde esa geometría, se sabe cuál y dónde, y el resto de la capa sigue significando lo que dice.
  - **Irremediable**: la falla deja sin sentido lo que viene después, o el kernel rechazó el booleano
    sobre la imagen ya acumulada. No hay «resto» confiable que importar.

  **Salteables, seis grupos:**
  1. **Flash** (`:316`–`:440`): `:336` y `:342` (template C), `:370` y `:377` (R), `obround_region` para O,
     `:418` y `:426` (macro), más los `Unsupported` de parámetro faltante del mismo bloque («without a
     diameter», «without width and height»).
  2. **Trazo** (`stroke_of` `:835`, el camino de un `Line` fuera de región): `capsule` `:1045`/`:1049`,
     `rectangle_region` `:964`/`:968`, `swept_rectangle` `:1018`/`:1022`, `obround_stroke` `:918`,
     `macro_stroke` `:1136` con sus dos rechazos `:1263`/`:1267`, y `union_of` `:1284`/`:1290` cuando el
     booleano de un trazo falla.
  3. **Macro** (`macro_region` `:677`, `macro_primitive` `:704`): `:684`, `:723`, `:752`, `:758`, `:809`,
     `:815`, más `evaluate_parameter`/`lex_parameter` `:542`, `:565`, `:575`. Todos son alcanzables sólo
     desde un macro, que es la forma de un constructo.
  4. **Arco fuera de región** (`:294`): un trazo de arco, que este lector no barre. Se pierde ese arco y el
     archivo nombra el punto final, así que el puntero después no está en duda.
  5. **Arco dentro de región** (`arc_from_gerber` `:474`): el arco es una arista del polígono en
     construcción, así que **no se saltea el arco**: se envenena la región y se reporta entera en `G37`,
     con la ubicación del arco que la rompió. Cerrar el hueco devolvería un polígono plausible que no es
     el del archivo.
  6. **Región en `G37`** (`:250` `Region::new`, `close_boundary`): una región que no es una región válida,
     o un contorno de una o dos líneas que no cierra una forma.

  **Irremediables, comportamiento sin cambios:**
  - `:198` la imagen vacía inicial (no es un constructo del archivo).
  - `NoApertureSelected` y `UnknownAperture`: el archivo nombra algo que nunca definió (`:32`). Es la
    misma razón que S6 da para Excellon: una referencia indefinida es un archivo roto, no una forma rota.
  - Estructura de región: `RegionBegin` dentro de región (`:228`), `RegionEnd` fuera (`:239`), región que
    cierra sin ningún contorno (`:246`) — el archivo dijo «acá hay una forma» y no dibujó nada.
  - `Line` sin puntero (`:262`), línea de longitud cero en región (`:268`), `Arc` sin puntero (`:299`).
    Una coordenada que no se entiende deja el puntero indeterminado, y todo lo que sigue cae en otro lado.
  - `apply_region` (`:456`, `:461`) y `union_of` (`:1284`) cuando el kernel rechaza combinar: la imagen
    acumulada no es confiable.
  - **Todo el parseo** (`:1444`–`:1972`): `%FS`, `%MO`, orden («a coordinate before %FS»/«%MO»), bloque
    extendido sin cerrar, tabla de aperturas (`parse_add` `:1757`–`:1790`), sentencia de coordenada
    (`parse_coord_stmt` `:1896`–`:1952`) y `current_point` `:1968`/`:1972`. El parseo decide formato,
    unidades y tabla de aperturas: sin eso no hay «resto» que importar, sólo números sin escala.
- **L6 (T2/T3: qué cambió).** `cam-gerber` está **debajo** de `cam-import` (`cam-import` depende de él),
  así que no puede depender de `ImportReport`: inventar esa dependencia sería un ciclo. El lector reporta
  con un registro propio (`cam_gerber::Skipped { construct, reason, location }`) y el importador lo
  convierte. En `cam-gerber`: `Skipped`, `image_reporting(tolerance) -> Result<(Region, Vec<Skipped>)>`,
  `image()` **delega** en él (por eso el caso feliz no puede divergir, S7b) y `reason_of` quita el prefijo
  `line N:` que el pase de imagen siempre rellena con `0`. En `cam-import`: `parse_gerber` usa
  `image_reporting` y empuja cada salteo a `ImportReport::skip` con la ubicación `{archivo}: command N`
  (el reporte es de la carpeta entera, así que `command 7` solo no diría de quién), `parse_layer` cuenta
  contornos, y `FolderImport` gana `report: ImportReport`. Excellon no cambió: `parse` sigue devolviendo
  `Result` y nada parcial puede escapar por esa firma.
- **L7 (cambio de contrato visible, deliberado).** Dos tests existentes fijaban rechazo entero para
  constructos que S2/T1 nombran como salteables («un trazo, un flash, un macro que no se puede barrer»):
  `a_contour_with_fewer_than_three_segments_is_refused` →
  `a_contour_with_fewer_than_three_segments_takes_its_region_with_it`, y
  `a_macro_aperture_draw_with_curved_edges_is_refused_naming_the_macro` →
  `a_macro_aperture_draw_with_curved_edges_is_skipped_naming_the_macro`. Siguen fijando que no se
  aproxima nada: cambia el destino (salteo reportado en vez de archivo rechazado), no la tolerancia.
- **L8 (T4: aceptación S7, medida).** Los tres pedazos de S7 tienen test propio, y el caso feliz está
  garantizado por construcción (S7b): `image()` delega en `image_reporting()`, así que no puede devolver
  otra geometría. Tests: 50 en `cam-gerber` (4 nuevos de E1.7), 16 en `cam-excellon` (1 nuevo), 12 en
  `cam-import/tests/folder.rs` (1 nuevo, punta a punta sobre una carpeta). `just check` → _all gates green_.
  **El test de aceptación muerde de verdad**: simulé la alternativa peligrosa (no envenenar la región, o
  sea cerrar el hueco y armar igual el polígono) y el test falló; restaurado, volvió a verde. Eso es lo
  que importa: el test no distingue «se salteó la región» de «se armó una región plausible» por
  casualidad.
- **L9 (la paridad falló y se re-fijó con el procedimiento documentado).** `just parity` salió 1: el gate
  compara `fixtures/corpus/<slug>/parity.json` contra lo que el árbol mide hoy (`scripts/parity/compare.py`),
  y E1.7 cambia lo que mide. Re-fijado con `scripts/parity/run.sh --update` (procedimiento documentado en
  `scripts/parity/README.md`, «review the diff» antes de commitear) y el diff clasificado con la regla que
  el propio README fija (`README.md:138`–`142`):
  - **0 regresiones**: ningún artefacto que ya estaba `parsed` movió área, puntos ni polígonos.
  - **5 `refused` → `parsed` con geometría**: `kicad-2l-simple/outline` 10.26 mm²/8 polígonos,
    `kicad-4l-pours/outline` 28.52/7, `kicad-large-pours/outline` 35.82/1, `substitute-altium/outline`
    129.78/1, `substitute-eagle/outline` 21.28/21.
  - **4 `refused` → `timeout`**: `kicad-2l-simple` bottom/mask-bottom/silk-bottom y
    `substitute-eagle/bottom`. No es una mejora: el lector ahora acepta la capa y el kernel no termina
    dentro del límite. Es la misma clase que el README ya documenta («el pin se movió de _el lector dice
    no_ a _el kernel no termina_») y cuesta 30 s de CI por capa; queda registrado, no escondido.
  - Totales nuevos: **17 `parsed`, 15 `refused`, 22 `timeout`** de 54 (antes 12/24/18). `timeout_seconds`
    y `flatten_tolerance_mm` no cambiaron.
    El re-fijado fue una decisión explícita, no un `--update` a ciegas: se corrió, se clasificó cada entrada
    y recién entonces se commiteó (commit propio, separado del código).
- **L10 (el audit).** Fila E1.7: `⬜` → `✅` con la evidencia (nombres de test, `image_reporting`,
  `FolderImport.report`) y dos límites explícitos: la ubicación es índice de comando y no línea de origen,
  y el parseo sigue rechazando el archivo entero. Números del corpus corregidos en las filas E0.4 y E1.4,
  en `scripts/parity/README.md` y en «Next slice» (12/24/18 → 17/15/22). De paso corregí dos frases que ya
  eran falsas antes de esta tarea: «Only E1.2 is fully done of all F1 items» (E1.3, E1.5 y E1.6 ya estaban
  ✅ en el mismo archivo) y «the FlatCAM oracle does not exist (E0.6 ⬜)» en la fila E1.4 (E0.6 se entregó
  con la 059). No las introduje yo; las dejo dichas para que no parezcan parte del diff de E1.7.
