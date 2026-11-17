# 061 — E1.4 Parser Gerber: que las seis placas del corpus midan

E1.4 quedó `🟡` en el roadmap porque el criterio de salida no se cumple: de los 54 artefactos del
corpus, 17 miden, **15 se rechazan y 22 se pasan del límite de 30 s del arnés**. El diagnóstico de
esta tarea dice que el parser ya lee 13 de esos 15 archivos enteros y que la muralla es el kernel
(`cam-geom`) y el costo de evaluar la capa constructo por constructo.

## Surfaces

- `crates/cam-gerber/`
- `crates/cam-geom/src/`
- `scripts/parity/`
- `fixtures/corpus/`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa E1.4»
- **S2 (E1.4 en `docs/ROADMAP_CAMCRAFT.md:93`, verbatim).** «| E1.4 | Parser Gerber RS-274X:
  aperturas estándar (C, R, O, P), macros, `G36/G37`, interpolación circular `G74/G75`, unidades y
  formato de coordenadas, `M02` | **L** | Las 6 placas del corpus producen geometría que pasa E0.4
  contra FlatCAM |»
- **S3 (estado medido, `odd/roadmap/index.md`, fila E1.4, verbatim).** «The exit criterion is *the six
  corpus boards produce geometry that passes E0.4 against FlatCAM*. The oracle half is answered — E0.6
  (task 059) pinned FlatCAM's own numbers for all six boards, and FlatCAM reads 51 of 54 artefacts —
  but the tree's half is not: of the 54 corpus artefacts, 17 parse, 15 are still refused, and **22
  exceed the harness' 20 s bound** and are pinned as `timeout`»
- **S4 (lo que el arnés NO prueba hoy, `scripts/parity/README.md` / `run.sh`, verbatim).** «It is not a
  claim of parity with FlatCAM. Comparing against an external oracle is the level D5 asks for and
  needs an oracle that runs here; what this harness compares against is this tree's own recorded
  output». El oráculo existe igual: `fixtures/baseline/<board>.json` ya lleva `status`, `polygons` y
  `contour_area_mm2` con la **misma forma** que el golden de paridad, así que comparar es leer dos
  archivos, no inventar un instrumento.
- **S5 (límite que esta tarea no mueve).** El límite del arnés es **30 s por artefacto** y está escrito
  en cada golden (`timeout_seconds`). Bajarlo o subirlo es un cambio de fixture con su propio review;
  esta tarea no lo retunea para que pase: hace que el árbol termine.
- **S6 (frontera que esta tarea no cruza).** El parser sigue rechazando el archivo entero por un
  error de formato (`%FS`, `%MO`, tabla de aperturas, orden de comandos): sin formato no hay «resto»
  que importar (S6 de la 060). Un constructo que no se puede construir sigue salteándose y
  reportándose (E1.7, tarea 060), no tumbando la capa.

## Tasks

- **T1** — `cam-gerber`: un `D01` dentro de `G36/G37` no necesita apertura seleccionada, y hoy el
  parser lo exige (`lib.rs:1615`). Desbloquea `kicad-large-pours/mask-bottom` y
  `substitute-eagle/mask-bottom`. Specs: S2. Route: inline.
- **T2** — `cam-geom`: medir dónde se va el tiempo al evaluar una capa de cobre del corpus y volver
  indexado el camino caliente del arreglo, de modo que `kicad-2l-simple/top` (3900 constructos,
  FlatCAM 4.97 s, área 482.21 mm²) termine dentro del límite de S5. Specs: S2, S3, S5.
- **T3** — `cam-geom`: el cierre del recorrido de caras. Trece capas del corpus mueren con
  `the arrangement revisits a piece, so its pieces cannot be chained closed`; el recorrido tiene que
  cerrarlas, o rechazar por una razón que nombre el constructo. Specs: S2, S3.
- **T4** — `cam-gerber`: estrategia de evaluación (partición por grilla / unión por lotes) sólo si
  T2+T3 dejan capas por encima del límite; la decisión la toma la medición, no la intuición.
  Specs: S2, S3, S5.
- **T5** — `scripts/parity/`: comparación contra el oráculo FlatCAM (`fixtures/baseline/`) por
  artefacto, re-pin de los goldens con lo medido, y actualización de la fila E1.4 del roadmap con los
  números reales. Specs: S2, S3, S4.

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa E1.4»
- **L2 (estado del árbol al empezar).** Rama `feat/E1.4` sobre `45fdb84` (merge de `feat/E1.1` en
  `dev`), sin claims activos en `odd/tasks/.claims`.
- **L3 (medición, no lectura).** Corrido `scripts/parity/measure` sobre los 15 artefactos rechazados
  y sobre un prefijo recortado de `kicad-2l-simple/top`. Resultado:
  - **13 de los 15 rechazos son del kernel, no del parser**: `parse` termina en milisegundos y
    `image_reporting` muere con `Invalid { what: "command N: polarity could not be applied: edit
    refused: the arrangement revisits a piece, so its pieces cannot be chained closed" }`
    (`crates/cam-geom/src/boolean.rs:776`).
  - **2 son del parser**: `NoApertureSelected { line: 62 }` y `{ line: 120 }`, ambos un `D01`/`G01`
    dentro de un `G36` antes de cualquier selección de apertura (`lib.rs:1615`).
  - **Los 22 `timeout` son costo, no cuelgue**: un prefijo de 264 líneas de
    `kicad-2l-simple/top` (137 polígonos de salida) tarda **35.9 s** en `image_reporting`; 527 líneas
    ya pasan los 120 s. El archivo entero tiene 3722 `D01` y 203 `D03`, y `image_reporting` une cada
    constructo contra la imagen acumulada (`apply_region`, `crates/cam-gerber/src/lib.rs`).
- **L4 (consecuencia de L3 para el roadmap).** La fila E0.1 dice `✅` con evidencia en
  `cam-geom/src/boolean.rs`, y es cierta sobre los tests sintéticos: ninguna prueba del kernel corre
  un pour real del corpus. E1.4 no se cierra sin tocar `cam-geom`, que es la espina serial del plan
  (`odd/roadmap/waves.md`: «Nadie más toca `cam-geom/src/` en esa ventana»). Esta tarea lo reclama
  entera y no se paraleliza con ningún otro agente.
