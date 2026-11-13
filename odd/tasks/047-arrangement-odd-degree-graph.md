# arrangement-odd-degree-graph

El motor de conjuntos de `cam-geom` rechaza geometría legal con `"the arrangement revisits a piece"` cuando un contorno lleva un tramo corto entre tramos largos. El rechazo no es un empate de orden: los supervivientes dejan un grafo de **grado impar**, así que la caminata no puede cerrar. Esta tarea arregla la clasificación que lo produce.

## Surfaces

- `crates/cam-geom/src/boolean.rs`
- `crates/cam-geom/tests/boolean_corpus.rs`

## Specs

- **S1 (el oráculo de E0.2, verbatim de 046 L4).** «cada caso degenerado o normaliza con la identidad de área, o se rechaza con una variante de error que nombra el constructo — nunca un área silenciosamente falsa».
- **S2 (el defecto medido, verbatim de 046 L5).** «los supervivientes dejan vértices de **grado impar** — grado 3 en `(10,10)` con tres salidas de ángulos distintos (+π/2, 0, π), grado 1 en `(5,10)` — así que la caminata revisite una pieza porque el grafo **no es encadenable**, y eso es inconsistencia de clasificación (`decide`/`material_side`), no un desempate de orden».
- **S3 (el alcance del arreglo, verbatim de 046 L5).** «arreglarlo necesita una reescritura acotada de cómo `combine` clasifica y retiene piezas cerca de uniones casi coincidentes, validada contra el corpus».
- **S4 (cómo se demuestra, verbatim del `## Backlog` de 046).** «Merece tarea propia contra G0.1/G0.2, con RED→GREEN en la identidad de área».
- **S5 (nada se aplana en silencio).** Contrato heredado de 034 S1 / I2: la corrección no puede aplanar arcos ni relajar la tolerancia para hacer pasar el caso. Un aplanado implícito es un bug, no una optimización.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Diagnóstico: nombrar **qué decisión de clasificación** deja el extremo colgado — qué pieza se conserva de más o de menos en `(10,10)` y `(5,10)`, con la instrumentación temporal retirada antes de reportar | S2 | worker | — |
| T2 | Arreglar la causa en `combine`/`decide`, no el síntoma: prohibido subir la tolerancia, atrapar el error y devolver algo degradado, aplanar arcos o special-casear `1e-6` | S1,S3,S5 | worker | — |
| T3 | Convertir `micro_edge_one_micron_combine_pins_current_behavior` en la aserción de identidad de área (`\|A∪B\| + \|A∩B\| = \|A\| + \|B\|` sobre las regiones curadas), RED observado antes del fix y GREEN después, más a lo sumo un test de borde si el fix tiene uno | S1,S4 | worker | — |
| T4 | Regresión completa: la suite del workspace, y el camino que atraviesa `cam-gerber` (la imagen de una capa usa `combine`) sin cambios de resultado en el corpus de paridad | S1,S3 | inline | — |

## Log

- **L1 (pedido, verbatim).** «vamos con la 047».
- **L2 (la evidencia heredada, para no volver a medirla).** De 046 L5: el contorno mínimo es `(0,0),(10,0),(10,10),(10,10+1e-6),(0,10)` —un rectángulo de 10×10 mm con un lado de 1 µm, mil veces `TOLERANCE = 1e-9`— y `combine(micro, rect(5,0,15,10), Union, 1e-9)` se rechaza con `EditRefused { reason: "the arrangement revisits a piece, so its pieces cannot be chained closed" }`. El vértice de 1 µm inclina el borde superior 1e-6 sobre 10 mm, que a la altura de `x = 5` pasa a 5e-7 de `(5,10)`: por encima de la fusión de vértices (1e-9), así que no se funde, pero lo bastante cerca para que las decisiones de conservar/descartar/invertir dejen un extremo sin pareja. **No hay empates angulares en los dos vértices imparciales**, que es lo que separa este caso del que arregló 034 L6.
