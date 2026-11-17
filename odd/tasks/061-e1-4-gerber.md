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
  Hecho (`b6f6643`): índice local `RegionIndex` (grilla uniforme) dentro de `boolean.rs` — corte por
  candidatos y pertenencia indexada en `combine_with_indexed`/`decide_indexed`, `Region` sin campos ni
  API nueva. `kicad-2l-simple/top` pasa de >300 s a **68.8 s** y falla con `command 1441: ... the
  arrangement revisits a piece, so its pieces cannot be chained closed` (T3), no por tiempo.
  `boolean_index` (11.352 comparaciones), fmt, clippy y `cargo test -p cam-geom` completos en verde.
  No alcanza 30 s ni el criterio: quedan T3 (correctitud) y T4 (evaluación por capa).
- **T3** — `cam-geom`: recorrido de caras / `revisits a piece`. Dos causas corregidas: (1) `decide_indexed`
  prefería la sonda cercana e inestable ante desacuerdo (`boolean_stroke_fill_overlap`); (2) la
  clasificación por sondeo ahora recoge todas las distancias definitivas, descarta outliers únicos y
  elige por mayoría (con desempate que preserva frontera y cercanía) en `decide_indexed`/`material_side`,
  evitando perder la pieza-puente del caso silk (`boolean_silk_union_probe`, `substitute-easyeda/silk-top`:
  559 polígonos, 22.7 s). Bajo 30 s pasan a `parsed` `substitute-easyeda/silk-top` (559), `kicad-large-pours/silk-top`
  (376), `substitute-altium/top` (643); a `timeout` `substitute-eagle/silk-bottom`, `substitute-easyeda/bottom`.
  **Sigue abierto**: `kicad-4l-pours/silk-top` (`revisits a piece`) y `kicad-2l-simple/top` (cmd 1441, lote
  falla y caída secuencial). Specs: S2, S3.
- **T4** — `cam-gerber`: estrategia de evaluación (un arreglo por capa / por tanda de polaridad en lugar de
  3900 `combine` secuenciales; la opción 2 elegida). Implementado en parte: los constructos oscuros
  consecutivos se unen en árbol balanceado y se aplican de una vez (`union_all` + `pending_dark`), con
  caída secuencial si el lote falla. Ganancia medida: `kicad-4l-pours/mask-top` 113 s → **2.55 s** (567
  polígonos). `kicad-2l-simple/top` cae a ~0.59 s al unir el lote pero ese lote falla en el kernel
  («a surviving piece begins and ends at the same point») y la caída secuencial vuelve a ~73 s con
  `revisits a piece` (cmd 1441). Correctitud/tiempo siguen abiertos en capas duras. Specs: S2, S3, S5.
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
- **L5 (T1 hecho, commit `420b2cb`).** `parse` deja de exigir apertura para un `D01`/`D02` dentro de
  `G36/G37` (`in_region` en el parser). Verde `cam-gerber` 52 tests, dos nuevos: la regla y la
  regresión de que un `D01` fuera de región sigue exigiéndola. Efecto sobre el corpus: los dos
  artefactos que morían en `parse` ahora mueren en `image` con la misma razón del kernel que las
  otras trece capas, así que **ningún número de paridad mejora todavía**: los 15 rechazos son, ahora
  sí todos, el mismo defecto del kernel.
- **L6 (medición de T2, y por qué no se cerró).** Tres experimentos, en este orden:
  1. **Unión balanceada contra secuencial** (200 cuadrados disjuntos, kernel real): secuencial
     **8.69 s**, balanceada **0.28 s** (31×). A 3900 constructos, balanceada sola sigue siendo
     ~100 s: no alcanza para el límite de 30 s de S5.
  2. **Asignaciones y caja envolvente en el arreglo** (aplanar `Contour::segments` una vez por
     operación + descartar pares cuyas cajas no se tocan): `kicad-2l-simple/top` pasó de 35.9 s a
     **34.9 s**. Nada. Revertido: un cambio sin ganancia medida es ruido para el review.
  3. **Costo de la pertenencia**: `Region::contains` escala lineal con las fronteras y cada paso es
     caro — 100 fronteras → 17.7 µs por sonda, 400 → 65.9 µs, 1600 → **245 µs**. Un `combine` pregunta
     dos veces por lado por cada pieza, así que un arreglo de 1600 fronteras cuesta ~2 s **por
     constructo**, y la imagen acumulada crece con cada constructo: el total es cuadrático en el
     tamaño de la capa. Ese es el muro, y está en `Region`, no en `cam-gerber`.
- **L7 (la decisión que queda abierta, con las tres salidas medidas).** Ninguna es un ajuste:
  1. **Indexar `Region`** (grilla o árbol dentro de la estructura): mata el costo por sonda, pero
     `Region` se construye con literal `Region { boundaries }` en todo el workspace, así que el
     índice obliga a tocar cada sitio de construcción — la mayor ganancia y la mayor superficie.
  2. **Arreglo k-ario en el kernel**: generalizar `combine` a una sola pasada sobre todos los
     constructos de una misma polaridad (`combine_all`), de modo que una capa sea **un** arreglo en
     vez de 3900. Sin índice el arreglo único sigue siendo cuadrático pero **una sola vez**: con
     ~4000 segmentos son ~0.5 s de intersecciones y ~2 s de pertenencia, dentro del límite de S5.
     Es la salida más chica que alcanza, y no cambia la representación de `Region`.
  3. **Reordenar el roadmap**: dejar E1.4 en 🟡 y hacer F2 (taladrado y cutout) primero, que ya
     tiene G-code real. El corpus queda como está: 17 medidos, 22 sin terminar.
- **L8 (por qué no se sigue sin decisión).** Las tres salidas cambian `cam-geom`, la espina serial que
  `odd/roadmap/waves.md` prohíbe compartir y que la fila E0.1 da por cerrada. Elegir cuál cambia el
  orden de las olas y lo que el roadmap puede seguir afirmando, y eso lo decide el humano, no la
  medición. T3 (el cierre del recorrido de caras) queda pendiente y es independiente de cuál se elija.
- **L9 (decisión: opción 2, y T2 en árbol, commit `b6f6643`).** El humano eligió la opción 2. T2 entregó
  un índice local `RegionIndex` en `crates/cam-geom/src/boolean.rs` sin tocar la representación ni la
  API de `Region`. `kicad-2l-simple/top.gbr` pasa de >300 s a **68.8 s** y falla con `revisits a piece`
  (cmd 1441). fmt, clippy y `cargo test -p cam-geom` completos en verde.
- **L10 (T3, sonda inestable, commit `8ab4fd5`).** Repro `/tmp/masktop_fail.gbr`: ante sondas definitivas
  en desacuerdo se conservaba la cercana (inestable) y se descartaba la lejana (estable), eliminando una
  pieza de borde. Fix: preferir el resultado definitivo más lejano. Regresión `boolean_stroke_fill_overlap`.
  Re-pines bajo 30 s: `refused`→`parsed` `kicad-2l-simple/mask-top`, `kicad-large-pours/mask-bottom`,
  `kicad-large-pours/mask-top`, `substitute-eagle/mask-top`; `refused`→`timeout` `kicad-4l-pours/mask-top`,
  `kicad-4l-pours/silk-bottom`, `kicad-4l-pours/top`, `substitute-altium/bottom`; varios `parsed`
  re-medidos por la clasificación más estable. T3 seguía abierto.
- **L11 (T4 batching parcial, commit `d4c2aed`).** Uniones oscuras consecutivas en árbol balanceado con
  caída secuencial si el lote falla (`cam-gerber`). `kicad-4l-pours/mask-top` 113 s→2.55 s (parsed). El
  lote de `kicad-2l-simple/top` falla («a surviving piece begins and ends at the same point») y la caída
  vuelve a ~73 s con `revisits a piece` (cmd 1441).
- **L12 (T3, hueco por sondeo, silk, commit `346723a`).** Repro `/tmp/silk24.gbr`: pieza-puente mal
  clasificada por desacuerdo transitorio de sondas dejaba dos vértices de grado 1. Fix: reunir
  distancias definitivas, descartar outliers únicos y elegir por mayoría con desempate que preserva
  frontera/cercanía (`boolean_silk_union_probe`). Bajo 30 s: `refused`→`parsed` `substitute-easyeda/silk-top`,
  `kicad-large-pours/silk-top`, `substitute-altium/top`; `refused`→`timeout` `substitute-eagle/silk-bottom`,
  `substitute-easyeda/bottom`.
- **L13 (T3, near-miss punta a punta, silk-top de kicad-4l-pours).** Repro aislado: unir la imagen de
  las 370 primeras líneas de `kicad-4l-pours/silk-top.gbr` con el trazo D10 de la línea 371 (cápsula
  horizontal 7 mm, y=-126.7). El extremo de una pieza existente y el inicio de la cápsula quedaban a
  ~1.42 µm (> ε=1 µm), fuera de soldadura por tolerancia y sin snap de near-miss (proyección no
  interior), rompiendo el grafo (`revisits a piece`). Fix: en la detección de near-miss (reintento)
  registrar snaps punta-a-punta entre extremos de segmento dentro de 2·ε. El repro aislado y el prefijo
  fallido ahora combinan; la capa completa ya no es `refused` pero es muy lenta (>180 s / 44 s a 20k
  líneas) y queda pineada `timeout` bajo 30 s. `cargo test -p cam-geom` completo, fmt, clippy y
  `--test parity` (565 s) en verde. Corpus: **25 parsed, 29 timeout, 0 refused**. Persisten T3 en
  `kicad-2l-simple/top` (cmd 1441) y T4/rendimiento (timeouts).
