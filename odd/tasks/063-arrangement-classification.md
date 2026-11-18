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
