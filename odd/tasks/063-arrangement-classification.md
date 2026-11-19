# 063 — Clasificación de piezas en el arreglo booleano (piezas que se descartan y abren el contorno)

El arreglo booleano corta las regiones en piezas y decide, por cada una, si se conserva, se invierte o se
descarta (`decide_indexed`). Cuando descarta una pieza que hacía falta, el contorno queda abierto: un
vértice queda sobre-conectado, otro queda con una sola arista, y el recorrido vuelve sobre una pieza ya
usada. El síntoma que llega al usuario es `edit refused: the arrangement revisits a piece, so its pieces
cannot be chained closed`.

Esta tarea existe separada de la 061 porque la 061 ya estableció que el problema no es la medición ni la
fase de corte: es la decisión de conservar o descartar, y merece su propio instrumento y su propia regla.

## Surfaces

- `crates/cam-geom/src/`
- `crates/cam-geom/tests/`
- `fixtures/corpus/`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «abre esa tarea».
- **S2 (estado medido que motiva la tarea, verbatim de la instrumentación).** En
  `kicad-2l-simple/top` quedan exactamente dos vértices no-manifold:
  `(148.699500,-87.652139294)` de **grado 3** y `(148.699500,-87.443774000)` de **grado 1**, separados por
  **0.208 mm** (`208365.29 eps`). La salida instrumentada es `non_manifold=2`, tras haber bajado de 234.
- **S3 (por qué no es soldadura, verbatim de la comparación).** Los huecos que sí eran de soldadura están a
  `8.60·ε = √74·ε` y `11.18·ε = √125·ε`; 0.208 mm es **208.365·ε**, tres órdenes de magnitud mayor. Es una
  longitud geométrica real: falta una pieza, no un vértice por unir.
- **S4 (lo que ya se falsificó y no debe repetirse, verbatim de `odd/tasks/061-e1-4-gerber.md` L23).**
  Soldar todos los vértices a 16·ε reduce los no-manifold de 234 a 2 pero rompe `substitute-eagle/outline`
  (pineado `parsed`); fusionar fracciones de corte a 16·ε es el culpable concreto de esa rotura; los snaps
  punta-a-punta a 16·ε, `dedup_pieces` a 16·ε y el umbral de contracción a 16·ε no tienen efecto; la
  guarda de `interior_fraction` a 16·ε rompe topología; y una reparación de sólo extremos abiertos
  (`join_open_ends`) deja la paridad verde pero no arregla `kicad-2l-simple/top`.
- **S5 (límites que esta tarea no mueve).** `Region` conserva su representación y su API pública
  (`contains`, `distance_to_boundary` con la misma semántica). No se sube ningún umbral para tapar el
  síntoma, no se retunea el límite de 30 s del arnés ni la tolerancia de comparación de 1e-3.
- **S6 (regla de honestidad de la evidencia).** Una corrida que termina por `timeout` no es un éxito:
  `parse ok` sin `image ok` significa que no se midió. Cualquier afirmación de "ya pasa" exige ver
  `image ok` o un estado JSON, no la ausencia de un mensaje de error.
- **S7 (casos que la tarea debe dejar medidos).** `kicad-2l-simple/top` y `substitute-easyeda/bottom`
  terminan hoy en `revisits a piece`; sus estados en `fixtures/corpus/<board>/parity.json` están pineados
  `timeout`, que **oculta el rechazo**, y eso también hay que corregir cuando se re-pinee.

## Tasks

- **T1** — Instrumentar `decide_indexed` sobre la pieza de 0.208 mm entre `(148.699500,-87.652139294)` y
  `(148.699500,-87.443774000)`: sus muestras de sonda, la decisión por intento y la decisión final, con
  los números de S2. Sin cambiar comportamiento. Specs: S2, S3, S6.
- **T2** — Decidir la regla con esa evidencia: si la mayoría de sondas que descarta una pieza necesaria es
  un error de muestreo (distancias que caen en bandas de frontera), de desempate, o de la propia noción de
  "lado del material" en esa zona. La decisión se escribe antes del código. Specs: S2, S3, S4.
- **T3** — Implementar la regla y su regresión mínima, que debe fallar sin el cambio. Specs: S2, S5, S6.
- **T4** — Re-medir el corpus: los dos casos de S7 deben pasar de rechazo a medición (dentro del límite de
  30 s o explícitamente lentos, nunca ocultos), y re-pinear sus estados junto con los de las capas que se
  muevan. Specs: S5, S6, S7.
- **T5** — Actualizar la fila E1.4 y la de esta tarea en el roadmap con los números reales. Specs: S5, S7.

## Log

- **L1 (pedido original, verbatim).** «abre esa tarea».
- **L2 (por qué separada de la 061).** La 061 agotó las hipótesis de medición, corte y soldadura (S4) y
  dejó el defecto localizado en una pieza descartada. Seguir dentro de la 061 habría mezclado una
  corrección de parser con una regla de clasificación del kernel, que es lo que S5 acota.
- **L3 (evidencia de apertura).** Instrumentación de `assemble` sobre `kicad-2l-simple/top`: tras soldar
  los huecos de pocos ε, quedan 2 vértices no-manifold, uno de grado 3 y otro de grado 1, a 0.208 mm, que
  es el patrón de una pieza faltante (S2, S3). El intento de reparación por umbral se abandonó porque rompe
  una capa pineada como `parsed` (S4).

- **L4 (T1, instrumento y evidencia, `CAMCRAFT_TRACE_063=1`, `fixtures/corpus/kicad-2l-simple/top.gbr`).**
  La pieza medida es `from_a=false path=0 len=0.208365294025 s=(148.699500,-87.443774000)
  e=(148.699500,-87.652139294) op=Union skip_a=None skip_b=Some(0)` — exactamente los dos extremos de S2.
  En las dos pasadas del `combine` (la segunda es el reintento con detección de cuasi-contactos, tras
  `revisits`) la clasificación es la misma:
  - intento 0, distancia 2.083653e-6: `left_a=true left_b=false right_a=true right_b=true`,
    `left_in=true right_in=true` => `Drop`.
  - intento 1, distancia 3.333845e-5: idem => `Drop`.
  - intento 2, distancia 5.334152e-4: `left_a=false left_b=false right_a=true right_b=true`,
    `left_in=false right_in=true` => `Reverse`.
  - intento 3, distancia 8.534642e-3: idem => `Reverse`.
  - intento 4: se detiene (`distance*2>len`).
  Muestras `[(Drop,0),(Drop,1),(Reverse,2),(Reverse,3)]`, filtradas iguales (ningún conteo es 1),
  `counts(K,R,D)=[0,2,2]`, decisión definitiva `Reverse` por prioridad ante el empate (`Reverse` > `Drop`,
  `Keep` no aparece), sin guarda de sliver (`adjacent_to_snap=false`). **La pieza no se descarta: se
  conserva invertida.** La traza completa está en `odd/tasks/evidence/063-t1-simple-top-gap.log`. Esto
  contradice la hipótesis de L3 ("la descartó `decide_indexed`"): la clasificación la conserva y el
  defecto —si sigue produciendo grado 3 y grado 1 en esos extremos— está después de `decide_indexed`
  (ensamblado, soldadura de vértices, deduplicación o contracción), no en la decisión misma.

- **L5 (investigación posterior a T1, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr` = primera `G36/G37` de
  `kicad-2l-simple/top.gbr`).** La pieza de S2 **no se pierde en la clasificación ni en la poda**: llega a
  ensamblado como `seg=(148.699500,-87.652139294)->(148.699500,-87.443774000)` (invertida), sobrevive a
  `dedup_pieces` y a la contracción en las dos pasadas. En ensamblado se le asignan `start=1114` en el
  vértice superior (grado 3: piezas 1112, 3216, 3217) y `end=3223` en el inferior (grado 1: sólo 3216),
  pero **existe otro vértice en las mismas coordenadas del inferior**, `VERTEX 820 (148.699500,-87.443774000)`
  con grado 2 (piezas 817, 818). El extremo inferior de la pieza quedó en un vértice distinto al de los
  otros tramos que llegan a ese punto, por lo que el contorno no cierra. **Conclusión:** el defecto no
  está en `decide_indexed` (T1 lo falsifica) sino en la identidad/fusión de vértices en ensamblado: un
  mismo punto geométrico termina representado por dos vértices. T2 debe partir de esto, no de la regla de
  sondas. Se añadió una guarda defensiva en `vertex_index` (segunda búsqueda por cercanía espacial antes
  de crear), que no elimina el duplicado observado; la causa del fallo de fusión sigue abierta.

- **L6 (T2, traza correlacionada por llamada a `assemble`, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr`,
  evidencia `odd/tasks/evidence/063-t2-vid-top-r1.log`).** La lectura de L5 estaba mal correlacionada entre
  pasadas: no hay dos vértices en el mismo punto dentro de la misma `assemble`. Lo que ocurre es:
  - **asm 4131** (2099 piezas, 55 snaps) y **4132** (99 snaps): un único vértice 820 en
    `(148.6995,-87.443774)` con grado 2 (pieza 817 `b0 -> 148.712003,-87.380915`, pieza 818
    `148.6995,-88.032744 -> b0`). Topología local correcta.
  - **asm 5025** (4497 piezas, 55 snaps, la pasada que contiene la pieza 3216) y **5026** (4527, 99): un
    único vértice 3223/3253 en `b0` con **grado 1** (sólo pieza 3216/3246 `top -> b0`), y el vértice
    1114/1133 en `top = (148.6995,-87.652139294)` con **grado 3** (piezas 1112, 3216, 3217 /
    `148.6995,-88.032744 -> top`). No hay otro vértice en `b0` y nada más llega a `b0`.
  Comparando 4131 con 5025, el corte cambió: donde 4131 tenía `(-88.033)->b0->(destino alto)`, 5025 tiene
  `(-88.033)->top` (3217) y `top->b0` (3216), dejando `b0` abierto y `top` como unión en T. **La causa
  real es un defecto topológico del conjunto de piezas que sobrevive a ese combine** (un extremo sin pareja
  en `b0` y una T en `top`), no un fallo de fusión de vértices ni un descarte de la pieza 3216. Falta
  determinar qué pieza que debería cerrar en `b0` falta o quedó mal cortada en asm 5025.

- **L7 (T2, piezas de entrada a `assemble` en las pasadas clave, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr`,
  evidencia `odd/tasks/evidence/063-t2-pieces-asm5025.log`).** En **asm 4131** (2099, 55) `b0` es unión
  cerrada: pieza 817 `b0=(148.6995,-87.443774) -> (148.712003,-87.380915)` y pieza 818
  `(148.6995,-88.032744) -> b0`. En **asm 5025** (4497, 55) no hay ninguna pieza que empiece o termine en
  `b0` salvo 3216 `(top=148.6995,-87.652139 -> b0)`; la continuación que en 4131 salía de `b0` hacia arriba
  se desvía por `x=148.7` (`top -> 1112 -> (148.7,-87.638) -> 1113 -> (148.7,-87.441) -> 3215 ->
  (148.711543,-87.383227)`), dejando `b0` de grado 1 y `top` de grado 3. **Decisión T2 (evidencia, sin
  código):** la rotura no es fusión de vértices (L6) ni clasificación de 3216 (L4/T1); es el **conjunto de
  piezas supervivientes** de asm 5025: topológicamente no encadenable (extremo abierto en `b0`, unión en T
  en `top`). La corrección debe venir de por qué ese combine produce esa partición/corte (qué pieza que
  cerraría en `b0` falta o se recortó hacia el desvío `x=148.7`) —ya no de sondas ni de soldadura— y hay
  que localizarla comparando el grafo de piezas de 4131 frente al de 5025 alrededor de `b0/top`.

- **L8 (T2, vecindad de `b0/top` en asm 5025, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr`, evidencia
  `odd/tasks/evidence/063-t2-pieces-near.log`).** En 5025 el borde no pasa por `b0`: sube por `x=148.6995`
  hasta `top` (3217), se parte en `top` hacia `b0` por 3216 (hoja muerta, grado 1) y hacia
  `(148.7,-87.638)` por 1112, sigue por `x=148.7` (1113) y vuelve a la cadena alta por 3215 →
  `(148.711543,-87.383227)` (≈ el punto al que 817 llegaba en 4131). Se forma así una cinta abierta de
  ~0.0005 mm de ancho entre `x=148.6995` y `x=148.7` (`3216,1112,1113,3215`) con `b0` colgando; en 4131
  ese tramo era el vértice simple `818→b0→817`. El conjunto superviviente de 5025 es, por tanto, no
  manifold en esa zona (T en `top`, hoja en `b0`) y la pregunta de T3 pasa a ser qué decisión de
  clasificación/corte de ese combine produjo la cinta y dejó sin pareja el extremo en `b0`, comparando las
  piezas fuente (origen `from_a/path`) de 4131 y 5025.

- **L9 (T3, origen y decisión de la cinta en asm 5025, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr`,
  evidencias `odd/tasks/evidence/063-t3-decide-combines-part-1.log`, `odd/tasks/evidence/063-t3-decide-combines-part-2.log`, `odd/tasks/evidence/063-t3-decide-combines-part-3.log`, `odd/tasks/evidence/063-t3-decide-combines-part-4.log`, `odd/tasks/evidence/063-t3-decide-combines-part-5.log`, `odd/tasks/evidence/063-t3-decide-combines-part-6.log`,
  `odd/tasks/evidence/063-t3-dropped-connector-trace.log`).** En `combine 5025` (from_a=false, path=0) el
  tramo `817` de 4131 (`(148.712003,-87.380915) -> b0`) queda partido y decidido así:
  `(148.711543,-87.383227) -> (148.7,-87.441260)` **Reverse** (3215),
  `(148.7,-87.441260) -> b0` len 0.002563 mm **Drop**,
  `b0 -> top(148.6995,-87.652139)` len 0.208365 mm **Reverse** (3216),
  `top -> (148.6995,-88.032744)` **Reverse** (3217); de path=67 se conservan
  `top -> (148.7,-87.638006)` **Keep** (1112), `(148.7,-87.638006) -> (148.7,-87.441260)` **Keep** (1113),
  `(148.711543,-87.383227) -> (148.719411,-87.369828)` **Keep** (1114). La pieza **descartada** que cerraría
  la cinta es `(148.7,-87.441260) -> b0` (2.563 µm): sus 4 sondas alcanzables dan
  `left_a=true,left_b=false,right_a=true,right_b=true` (`left_in=true,right_in=true`) ⇒ `Drop` unánime
  (no alcanza un radio con lados distintos, `distance*2>len` en el intento 4). La pieza 3216 (`b0->top`)
  da `Drop,Drop,Reverse,Reverse` ⇒ `Reverse`. La cinta abierta (hoja en `b0`, T en `top`, desvío por
  `x=148.7`) es, por tanto, consecuencia directa de descartar ese conector de 2.563 µm: sin él, 3216 no
  tiene pareja en `b0`. T3 debe decidir si ese `Drop` unánime a escala sub-longitud es geométricamente
  correcto para el union o un falso interior, y fijar la regla/regresión mínima.

- **L10 (T3, experimento controlado, `CAMCRAFT_TRACE_063=1`, `/tmp/top_r1.gbr`, evidencia
  `odd/tasks/evidence/063-t3-force-drop-3216.log`).** Forzando a `Drop` la pieza 3216
  (`b0->top`, 208 µm, `from_a=false path=0`) en asm 5025/5026, el vértice en `b0` desaparece y `top` pasa
  a grado 2 (`3217↔1112`), o sea la cinta se cierra por `1112,1113,3215`. El archivo completo sigue
  fallando con `revisits a piece`, así que esta no es la única rotura, pero el defecto local queda
  resuelto. Lectura geométrica: el conector de 2.563 µm es interior en todo su rango alcanzable (Drop
  correcto); 3216 sólo se vuelve `Reverse` en sondas lejanas (~0.53–8.5 mm, decenas de veces su longitud)
  mientras a rango cercano es `Drop` — las sondas lejanas cruzan otra geometría y contradicen lo local.
  Esto empuja T3 hacia una regla que no permita que sondas lejanas conviertan en frontera una pieza cuyos
  vecinos inmediatos y sondas cercanas la declaran interior, pero hay que acotarla para no degradar casos
  donde la sonda lejana sí es la correcta.

- **L11 (T3, heurística candidata probada y revertida).** Se probó `decide_indexed` con “si las dos
  sondas definitivas más cercanas coinciden, usarlas; si no, mayoría”: en `/tmp/top_r1.gbr` clasifica 3216
  como `Drop` (coincide con sondas cercanas y con el conector interior) y corrige la topología local
  `b0/top` igual que L10; el archivo completo aún falla (`revisits a piece`), por lo que hay más roturas.
  Pasaron `boolean_index`, `boolean_silk_union_probe`, `boolean_stroke_fill_overlap`,
  `boolean_acceleration_regression`, `boolean_operations` (16). No se corrió la suite completa de
  `cam-geom` ni paridad. La regla es coherente con la evidencia (las sondas lejanas de 3216 cruzan otra
  geometría) pero falta validar contra `boolean_properties/corpus/parity` y contra los casos que
  motivaron la preferencia por sondas lejanas en 346723a antes de adoptarla.

- **L12 (T3, heurística cercana descartada).** La regla “si las dos sondas definitivas más cercanas
  coinciden, usarlas; si no, mayoría” corrige 3216 en `/tmp/top_r1.gbr` y pasa los booleanos dirigidos
  (índice, silk_union_probe, stroke_fill_overlap, acceleration_regression, operations), pero **rompe**
  `fixtures/corpus/substitute-easyeda/silk-top.gbr` en 1.5 s con `revisits a piece` (caso que 346723a
  dejó parseado, 559 polígonos). Revertida. La regla no puede adoptarse tal cual: hay piezas donde el
  par cercano concuerda y aun así la clasificación correcta es la lejana/mayoritaria. Sigue abierto cómo
  distinguir “sondas lejanas espurias por cruzar otra frontera” (3216) de “sondas cercanas engañosas”
  (silk-top) sin una señal geométrica adicional (p. ej. adyacencia a arista interior descartada que deja
  un extremo sin pareja, o coherencia con el grado del vértice resultante).

- **L13 (T3, reparación acotada en reintento).** En el reintento por arreglo no encadenable
  (`detect_near_misses=true`), `combine_with_indexed` descarta piezas frontera *far-flip*: las dos
  sondas definitivas más cercanas concuerdan en `Drop` pero sondas más lejanas (que pueden cruzar
  otra frontera) invirtieron la clasificación a `Keep/Reverse`. Se intenta ensamblar el conjunto
  reparado antes de fallar. En `/tmp/top_r1.gbr` esto identifica y descarta exactamente 3216
  (`b0->top`, 0.208 mm) en el reintento, pero el ensamblado reparado sigue en `revisits a piece`
  (4526 segmentos, mismo resultado que L10 al forzar 3216): la capa completa tiene otros defectos no
  cubiertos por este patrón. `substitute-easyeda/silk-top.gbr` sigue parseando (la reparación no se
  activa sin reintento no encadenable). Booleanos dirigidos verdes: `boolean_index`,
  `boolean_silk_union_probe`, `boolean_stroke_fill_overlap`, `boolean_acceleration_regression`,
  `boolean_operations`. Falta validar suite completa de `cam-geom`, paridad y los dos rechazos
  completos (S3) antes de cerrar T3.

- **L14 (T3, cierre de extremos de grado 1 en reintento).** Tras la reparación far-flip, el reintento
  de `/tmp/top_r1.gbr` (asm 5026) queda con 110 vértices de grado 1 y 0 de grado 3: pares de extremos
  colgantes a ~1.4 µm (√2·ε) sin arista de unión (p. ej. `seg2613` termina en `(154.798049,-108.156033)`
  y `seg2612` empieza en `(154.798050,-108.156034)` yendo en sentido opuesto). Acotado al mismo camino
  de recuperación, cuando el grafo es sólo grado 1/2 se sueldan pares de extremos grado-1 dentro de
  4·ε al punto medio y se reensambla. Resultado: `/tmp/top_r1.gbr` produce imagen
  (`image ok in ~118 s: polygons=111, skipped=83`; persiste un trazo rechazado en comando 704
  `revisits a piece` y arcos no soportados). Booleanos dirigidos verdes (`boolean_index`,
  `boolean_silk_union_probe`, `boolean_stroke_fill_overlap`, `boolean_acceleration_regression`,
  `boolean_operations`). Pendiente: capa completa `kicad-2l-simple/top.gbr`,
  `substitute-easyeda/bottom.gbr` (S3), suite completa/paridad y verificar que la soldadura acotada no
  altere geometría fuera de estos reintentos.

- **L15 (T4 medición, capas completas S3).** Con `bedb58e` (reparación far-flip + soldadura grado-1↔grado-1
  ≤ 4·ε en reintento), probe de las dos capas objetivo:
  - `fixtures/corpus/kicad-2l-simple/top.gbr`: `parse ok in 2.52 ms`, `IMAGE-ERR after 117.69 s` —
    `edit refused: the arrangement revisits a piece, so its pieces cannot be chained closed`. El prefijo
    `/tmp/top_r1.gbr` (primera región G36/G37) sí produce imagen (L14), por lo que el rechazo completo
    proviene de otra región/combinar de la capa o de un defecto no cubierto.
  - `fixtures/corpus/substitute-easyeda/bottom.gbr`: `parse ok in 7.52 ms`, `IMAGE-ERR after 32.02 s` —
    mismo rechazo. Tiempo distinto al observado antes (~356 s), pero el resultado sigue siendo fallo a
    terminación (sin `image ok`).
  S3 sigue sin cumplirse para ambas capas. Próximo paso: localizar en qué región/`combine` falla cada
  capa completa (traza/instrumentación acotada por capa) para distinguir si comparten el patrón far-flip
  + extremos abiertos ya reparado o si son causas distintas.

- **L16 (T4, diagnóstico bottom).** Con traza `CAMCRAFT_TRACE_FAIL` y refactor de reparación (adoptar el
  conjunto sin piezas far-flip como base aunque falle el ensamblado, antes del intento final), los fallos
  finales de `substitute-easyeda/bottom.gbr` muestran:
  - `a_bounds=1 b_bounds=1 op=Union`: estados finales observados con T-junctions
    (`segs=10 vertices=8 deg1=0 deg2=4 deg3=4 deg4+=0 far_flips=2`) y con extremos abiertos
    (`segs=42 vertices=44 deg1=4 deg2=40 deg3=0 deg4+=0 far_flips=0`).
  - `a_bounds=113 b_bounds=1 op=Union`: `segs=2274 vertices=2276 deg1=4 deg2=40?` no, `deg1=4 deg2=2272
    deg3=0 deg4+=0 far_flips=0` (repetido). La soldadura de pares grado-1 no aplica en grafos con
    grado 3 y, cuando aplica, no basta o no se alcanza el caso acotado actual. La reparación far-flip sola
    no elimina las T-junctions del primer caso. S3 sigue abierto; hay que localizar los `combine`
    concretos (región/comando) y caracterizar causas (T-junction vs extremos abiertos no emparejables
    dentro de 4·ε o con topología distinta).

- **L17 (T4, diagnóstico corregido bottom).** Con traza refinada (intento near-miss, a/b, reintento explícito)
  y prueba de tolerancia de emparejado:
  - `a_bounds=1 b_bounds=1, near_miss=false, segs=10, d3=4, far_flips=2` falla en primer intento pero el
    reintento de ese combine tiene éxito (no produce fallo final); el reintento/rescate del camino nm=true
    lo resuelve.
  - `a_bounds=1 b_bounds=1`: ambos intentos terminan en `segs=42, d1=4, d2=40, d3=0, far_flips=0`.
  - `a_bounds=113 b_bounds=1`: ambos intentos terminan en `segs=2274, d1=4, d2=2272, d3=0, far_flips=0`.
  En los dos casos de extremos abiertos hay dos pares de hojas: uno a 1.414·ε y otro a
  4.000000004·ε (4·ε + ruido de coma flotante) — mismo punto crítico observado en L22
  (`93.129430/-88.588638` vs `93.129426/-88.588638`). El límite `4·ε` estricto no lo emparejaba; con
  margen mínimo (`4·ε + 1e-12`) el combine ya no falla rápido en ~34 s, pero la capa completa pasa a
  correr >300 s (en curso al registrar), lo que sugiere que ese combine se repara y el procesamiento
  continúa a etapas posteriores (pendiente de medir a terminación o nuevo fallo).

- **L18 (T4, bottom con margen de emparejado).** `fixtures/corpus/substitute-easyeda/bottom.gbr` con
  `9d98491` (margen `4·ε + 1e-12`): supera el fallo rápido previo (~34 s) pero agota `timeout 600` sin
  `image ok` ni `IMAGE-ERR` (`parse ok`, `EXIT=124` del timeout). No es éxito (S6); indica que los
  combines de 4 hojas se reparan y el procesamiento avanza, pero queda un cuello de rendimiento o un caso
  patológico posterior. S3 sigue sin cumplirse para bottom.

- **L19 (T4, kicad-2l-simple/top completo).** Con `9d98491`, `fixtures/corpus/kicad-2l-simple/top.gbr`
  produce imagen: `image ok in 21.01 s: polygons=107, skipped=83` (persiste el trazo rechazado de
  apertura 28 en comando 704 `revisits a piece` y arcos de trazo no soportados). La capa ya no falla a
  terminación por el arreglo de regiones (antes `IMAGE-ERR ~117.7 s`, L15); el prefijo `top_r1` ya
  imagenaba (L14). S3 queda satisfecho para `kicad-2l-simple/top` en cuanto a parseo/imagen a terminación
  (con omisiones preexistentes/no soportadas), sujeto a verificación de geometría/paridad (S4, T5).
  `substitute-easyeda/bottom.gbr` sigue sin completar (timeout 600 s, L18).

- **L20 (T4, medición kicad-2l-simple/top).** Medición del artefacto con el measurer de paridad:
  `status=parsed, contour_area_mm2=477.73819038600953, points=8556, polygons=107` (21.01 s en probe;
  <30 s). Golden actual: `status=timeout`. Baseline FlatCAM: `status=parsed,
  contour_area_mm2=482.2125158364497, polygons=36, vertices=78680`. Diferencia de área relativa
  ~0.00928 (>0.001) y polígonos/estructura distintos (107 vs 36; 83 omisiones en probe, incl. trazo
  comando 704 rechazado por `revisits a piece` y arcos no soportados). Estado: pasa a completar e
  iguala status con FlatCAM, pero el área no está dentro de tolerancia 1e-3; no se re-pinea golden en
  este paso (requiere validación S4/S6, decisión sobre divergencia y no mezclar con `--assume-known-timeouts`).
