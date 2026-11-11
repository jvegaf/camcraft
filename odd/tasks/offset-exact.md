# offset-exact

Make the offset keep its arcs when a contour self-intersects. Feature G3.2 of
`docs/GAPS_CAMCRAFT.md` — the second hard blocker after G0.1, on the critical path to
isolation and NCC. Without it, every operation that offsets copper near a tight
clearance either loses exactness or refuses.

## Specs

**S1 — El offset actual no soporta isolation confiable.** De `docs/GAPS_CAMCRAFT.md:160-167`,
verbatim:

"- **G3.2 — El offset actual no soporta isolation confiable.**
  `offset_contour` (`cam-geom/src/offset.rs:69`) hace offset por curva y resuelve vértices
  intersectando curvas soporte, con fallback a puente recto + `OffsetWarning::MiterLimited`
  (`offset.rs:246-279`); el cleanup de self-intersection **tessela, parte y se queda con el loop más
  grande, reportando `Flattened`** (`offset.rs:403-491`); y `Region::offset` puede rechazar el
  resultado si el offset atraviesa la forma (`region_offset.rs:120-185,244-344`).
  **Impacto:** en cobre denso y concavo, aclaraciones finas de PCB, este camino o pierde exactitud
  (aplana arcos) o rechaza. Es el segundo bloqueante duro después de G0.1."

**S2 — Una regla no está fijada si sólo se probó en la geometría fácil.** El defecto vive en el camino
que se activa **sólo cuando el contorno se auto-intersecta**: en la geometría simple el arco sobrevive y
el resultado sale `flattened: false`. Un test que no fuerce la auto-intersección sobre un contorno con
arcos no prueba nada de esto, y es la misma clase de punto ciego que dejó pasar el defecto de
`dedup_pieces` en el motor de conjuntos.

**S3 — El motor de conjuntos ya sabe hacer esto sin aplanar.** `normalize` y `combine`
(`crates/cam-geom/src/boolean.rs`) preservan `Segment::Arc` y está fijado por tests. Si el cleanup puede
apoyarse en el arrangement en vez de en una polilínea muestreada, la pieza existe y G3.2 es reemplazar un
camino por otro, no inventar geometría.

## Tasks

| ID | Task | Specs | Route | Commit |
| -- | ---- | ----- | ----- | ------ |
| T1 | Mapear el pipeline del offset y decidir si el arrangement reemplaza al tessellated cleanup | S1, S3 | explorer | L3 |
| T2 | El cleanup de auto-interseccion preserva arcos | S1, S3 | inline | L4 |
| T3 | Tests con arcos que **fuerzan** la auto-interseccion | S2 | inline | L4 |
| T4 | G3.2 corregida en `docs/GAPS_CAMCRAFT.md` | S1 | inline | L4 |

**Fuera de alcance, declarado:** `Region::offset` rechazando el resultado cuando el offset atraviesa la
forma (`region_offset.rs`) es el otro extremo del mismo bloqueante y merece su propia tarea cuando T2
cierre; `OffsetWarning::MiterLimited` no cambia acá.

## Log

**L1** (owner, autorizacion): la recomendacion fue "agarrar **G3.2** antes que nada cosmetico. El offset
que aplana arcos es el proximo defecto de la misma familia que el de hoy —silencioso, geometrico, y en el
camino critico de la primera operacion que de verdad corta." El owner respondio: "perfecto, dale".

**L2** (owner, correccion de proceso): "no veo las tasks en odd". Tenia razon: los dialectos de Excellon
recien entregados vivian solo como entradas de Log en `c3-model.md`, sin fila en su tabla de tareas, y esta
feature no tenia documento. Se agregaron las filas E1/E2 con su spec S6 y se creo este documento antes de
escribir una linea de codigo de G3.2.

**L3** (reconocimiento, y la decision que salio de el): el aplanado vive en un solo lugar,
`offset.rs:160-170`: cuando el candidato se auto-intersecta, se tessela, se parte en dos loops y se
reconstruye con `Contour::from_points`, que es sólo segmentos de recta. El resto del pipeline **sí**
preserva curvas: cada segmento se desplaza como curva y el arco conserva centro y radio.

La pieza que decidia el arreglo: `normalize` corta un contorno contra sí mismo sin aplanar, pero su
contrato **no es el mismo** que el del camino muestreado. `normalize` devuelve un arrangement por
paridad, que puede ser cero, una o muchas fronteras; el camino muestreado se queda con el loop de mayor
área. Reemplazarlo sin más habría cambiado qué geometría sobrevive, en silencio. La decisión fue
normalizar y después elegir la frontera de mayor `|signed_area|`, que reproduce la elección vieja
apoyada en geometría exacta, con el camino muestreado como fallback cuando el arrangement aplana o no
devuelve nada.

**L4** (T2, T3 y T4): el arreglo es de veinticinco líneas. Lo que costó fue el fixture, no el código.
Los 32 tests de offset que había —15 de `offset_contour`, 17 de `Region::offset`— no alcanzaban el
camino que aplana: hay tests con círculos y tests con rectángulos, pero **ninguno con arcos unidos a
rectas**, y ninguno fuerza la auto-intersección, que es la única condición que activa esa rama. Es
exactamente el mismo punto ciego que dejó pasar el defecto de `dedup_pieces`: la geometría que el
camino difícil necesita no aparecía en ningún fixture.

El caso salió de un test que ya existía en otra suite: `a_narrow_neck_flattens_the_ring_and_says_so`
usa un soporte en U de rectas que, encogido 8 mm, cruza las paredes de un brazo. Le agregué un arco
superficial en la base —bulge 0,05— y quedó el caso que hacía falta: arcos **y** auto-intersección.
Contra el código viejo el test falla con `[Flattened]`, que es el ROJO honesto; con el arreglo pasa y
`has_arcs()` sigue siendo verdadero. Sacando la rama del arrangement, el test vuelve a morir.

**Estado:** workspace en **1317 passed, 0 failed, 3 ignored**. G3.2 queda con el aplanado cerrado; el
rechazo de `Region::offset` cuando el offset atraviesa la forma sigue abierto y se anota como tarea
aparte, no como resuelto.
