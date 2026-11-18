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
