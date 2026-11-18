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
- **L13 (T3, near-miss punta a punta, commit `55f5fd7`).** Repro aislado (prefijo de
  `kicad-4l-pours/silk-top` + trazo línea 371): extremos a ~1.42 µm (> ε) sin snap interior rompían el
  grafo. Fix: en el reintento de near-miss registrar snaps punta-a-punta entre extremos dentro de 2·ε.
  La capa ya no es `refused` pero es lenta (>180 s) y queda `timeout`. Corpus: **25 parsed, 29 timeout,
  0 refused**.
- **L14 (T3, `kicad-2l-simple/top`, commits `84de02f` y `ae1965e`).** Repro: primer `G36/G37`
  (`/tmp/top_r1.gbr`) fallaba por vértices de grado 3 (aristas casi duplicadas) y luego por
  `the arrangement is finer than the tolerance can resolve` en una pieza de ~2.8 µm. `dedup_pieces`
  compara equivalencia geométrica orientada dentro de tolerancia; `interior_fraction` rechaza cortes
  que dejarían piezas ≤ 3·ε de un extremo. **Corrección de L13/L14:** la capa se había declarado
  «ya no rechaza» porque la sonda se cortaba por timeout antes del error. Corriendo a terminación
  completa sigue terminando en `refused` (`revisits a piece`) a ~107 s; lo que se ganó es que falle
  más rápido. T3 no está cerrado para esa capa (ver L15).
- **L15 (T4, perfil y aceleración, worker).** Perfil por fases (timers temporales, `perf` no disponible):
  para `kicad-2l-simple/top` **decide 60.3%**, assemble 23.6%, dedup 11.8%, cut 4.2%; para
  `kicad-4l-pours/silk-top` decide 52.1%, assemble 35.8%; para `substitute-eagle/silk-bottom`
  decide 46.4%, assemble 43.2%. Aceleraciones semánticamente neutras en `cam-geom/src/boolean.rs`:
  grilla de fronteras candidatas y pre-chequeo por caja en `classify_boundary`/`contains`/`enclosed_by`,
  búsqueda espacial de vértices en `assemble`/`vertex_index` (match determinista de índice mínimo) y
  índice de extremos en `dedup_pieces` (elimina el O(n²)). Regresión `boolean_acceleration_regression`
  (unión de 128 fronteras disjuntas y resta de 64 agujeros con pertenencia y área exactas).
  `kicad-2l-simple/top` 249.6 s→107.7 s (**2.3x**) pero sigue `refused`; `kicad-4l-pours/silk-top` y
  `substitute-eagle/silk-bottom` siguen >300 s. `cargo test -p cam-geom` (287), `-p cam-gerber` (52),
  fmt y clippy verdes. **Conclusión del perfil: el costo dominante es la clasificación por sondas
  sobre arreglos que crecen, no el índice; bajarlo más requiere cambiar la estrategia de evaluación**
  (no micro-tuning).
- **L16 (estado real de correctitud, no visible en dorados).** Corriendo a terminación, además de
  `kicad-2l-simple/top` (`revisits a piece`) también `substitute-easyeda/bottom` termina `refused`
  (`a surviving piece begins and ends at the same point`). Ambos están pineados `timeout` en el arnés,
  que oculta el rechazo. T3 se reabre para esos dos casos.
- **L17 (T5 adelantado: el árbol no coincide con el oráculo).** `fixtures/baseline/` **es** el oráculo
  FlatCAM (lo escribe `scripts/baseline/pin.py` desde una corrida de FlatCAM-Plus; sus 3 `refused`
  cierran con «FlatCAM lee 51 de 54»), así que la comparación de T5 se puede hacer sin esperar T4:
  es leer dos JSON. Comparados los 24 artefactos con medición en ambos lados (`status`, `polygons`,
  `contour_area_mm2`): **1 de 24 coincide** en área a `rel 1e-9`; 18 de 24 coinciden en polígonos.
  Se agrupan en tres familias:
  1. **`outline` (3 placas, `rel` ≈ 0.99).** FlatCAM mide el *área encerrada* por el contorno
     (`kicad-2l-simple` 1210.58, `kicad-4l-pours` 16790.50, `substitute-eagle` 13410.66 mm²); el árbol
     mide la **línea dibujada** (10.26, 35.35, 29.92 mm²; 8, 8 y 20 polígonos frente a 2). No es un
     bug: son dos definiciones distintas de «outline». Hay que decidir cuál es la regla comparable
     antes de re-pinear, no ajustar el número.
  2. **Máscaras, deriva chica (`rel` 5.6e-5 … 8.96e-5).** `kicad-2l-simple/mask-top`,
     `kicad-4l-pours/mask-bottom`, `kicad-large-pours/mask-top/bottom`, `substitute-eagle/mask-top`,
     `substitute-easyeda/mask-top/bottom`: polígonos idénticos y área que difiere muy poco. Es
     consistente con el aplanado de arcos de Shapely frente a los arcos nativos del árbol (la nota de
     tolerancia de arcos del roadmap), pero **`1e-9` relativo no es la regla adecuada para arcos** y
     hoy hace fallar la comparación. Falta declarar la tolerancia por artefacto.
  3. **Diferencias reales de geometría.** `kicad-large-pours/silk-top` (395 vs 376 polígonos,
     8712.90 vs 5066.31 mm², `rel` 0.42), `substitute-easyeda/silk-top` (586 vs 559, `rel` 0.25),
     `kicad-4l-pours/mask-top` (mismos 567 polígonos pero `rel` 0.022), `substitute-altium/top`
     (634 vs 643, `rel` 7.8e-4), `kicad-large-pours/silk-bottom` (`rel` 2.9e-5). Estas sí son
     divergencias que el árbol tiene que explicar o corregir.
  Conclusión para el orden del trabajo: **T4 (tiempo) no es la brecha principal**. Los 29 `timeout`
  no tienen medición que comparar, pero los que **sí** miden ya discrepan del oráculo, así que acelerar
  antes de cerrar la paridad optimiza capas cuyo resultado todavía no es el correcto.
- **L18 (decisiones del humano sobre T5).** Dos decisiones que fijan la regla de paridad: (1) el
  `outline` se mide **como lo mide FlatCAM**, es decir el **área encerrada** por el contorno, no el área
  del trazo dibujado; (2) **se acepta tolerancia** de área por artefacto/rol en vez del `1e-9` relativo
  actual. La tolerancia se ancla en lo medido: la deriva por aplanado de arcos es ≤ 9.0e-5 y las
  divergencias reales del corpus son ≥ 2.2e-3, así que `1e-3` relativo separa unas de otras; los
  conteos (`polygons`) no se comparan como fallo, sólo se reportan, igual que `compare.py` ya hace con
  `vertices` frente a `points`. Se delega el instrumento (`scripts/parity/`, `fixtures/corpus/`),
  disjunto del worker de T3 que sigue en `crates/`.
- **L19 (alcance de T5 recortado por decisión).** El humano decidió que la **reconstrucción del contorno
  de placa** no se hace dentro de esta tarea: pasa a la [062](062-board-outline-contour.md). Motivo
  medido: en `kicad-2l-simple`, `kicad-4l-pours` y `substitute-eagle` el `outline` son segmentos sueltos
  con huecos de hasta 23.4 mm, así que cerrarlos no es una tolerancia de unión sino derivar un polígono
  nuevo; las otras tres placas cierran con hueco 0.0 mm y **ya coinciden exactas** con FlatCAM, lo que
  prueba que la medición es comparable cuando el contorno cierra. Con eso, T5 en esta tarea queda
  limitada a las divergencias de **cobre, máscara y silk**, que sí son su alcance: `kicad-large-pours/silk-top`
  (0.419), `substitute-easyeda/silk-top` (0.249), `kicad-4l-pours/mask-top` (0.0224) — dos de ellas
  justamente capas que dejaron de rechazar por el fix de mayoría de sondas, así que hay que verificar si
  ese fix cambió geometría para «dejar de fallar» en vez de para acertar.
- **L20 (T3, `substitute-easyeda/bottom`: causa raíz hallada, arreglo no aceptado).** El worker aisló la
  causa con números: una pieza superviviente tenía sus dos extremos a ~1.0·ε de un **mismo vértice**
  existente (p. ej. `(152.368635,-124.837519)`) aunque los extremos entre sí distan ~1.414·ε, así que
  `vertex_index` los soldaba al mismo vértice, `start == end` y `assemble` rechazaba con «a surviving
  piece begins and ends at the same point». Eso explica el síntoma; el diagnóstico se acepta.
  **El arreglo no.** Dos variantes, mismo resultado:
  1. worker: `filter_valid_snaps` con tolerancia + en `assemble` reasignar el extremo a otro vértice o
     **descartar el sliver** y seguir; el repro pasó de «begins and ends at the same point» a
     **`revisits a piece`** (~277 s), o sea el rechazo se movió, no se resolvió; además la regresión sólo
     afirmaba la *ausencia de un mensaje* (acepta cualquier otro error) y tardaba 288 s en release y
     ~1400 s en debug, y la capa pasó de 78 s a 279 s y `kicad-2l-simple/top` de 107 s a 170 s.
  2. alternativa propia, en la capa correcta: descartar piezas con longitud ≤ 2·ε entre los
     supervivientes y antes de `assemble` (`is_tolerance_sliver`). Verificado con el mismo repro
     `/tmp/bot_12797.gbr`: **también** termina en `revisits a piece` (~139 s).
  Conclusión medida: **el sliver es portante**. Sacarlo abre el arreglo y el recorrido revisita, así que
  el problema no está en el ensamblado sino en el **corte**: se está creando un trozo de ~2·ε cuyos dos
  extremos caen dentro de ε de un vértice ajeno. El arreglo correcto es que el corte no genere ese trozo
  (fundir la fracción de corte con el vértice existente que está a menos de ε), no descartarlo después.
  Queda como el próximo paso de T3, junto con `kicad-2l-simple/top`.
