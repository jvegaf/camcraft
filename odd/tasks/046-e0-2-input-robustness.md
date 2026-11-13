# e0-2-input-robustness

Robustez de entrada y normalización en `cam-geom` para geometría degenerada (E0.2): que contornos problemáticos produzcan resultados válidos o rechazos explícitos y diagnosticables, sin errores espurios de normalización.

## Surfaces

- `crates/cam-geom/src/`
- `crates/cam-geom/tests/`
- `docs/ROADMAP_CAMCRAFT.md`
- `docs/GAPS_CAMCRAFT.md`

## Specs

- **S1 (E0.2, verbatim ROADMAP).** «Robustez de entrada: normalización de contornos auto-intersectados, bordes coincidentes, vértices de contacto (`region.rs:162-165` hoy rechaza)»
- **S2 (criterio de salida, ROADMAP, con corrección de referencia).** El ROADMAP dice: «El corpus de E0.3 no produce errores de normalización». **Corrección necesaria:** E0.3 es política de tolerancia y el corpus de placas es E0.5; la referencia correcta es el corpus/ejercicio medible definido para E0.2 (degenerados) y, en la medida que aplique, el corpus E0.5 cuando el pipeline lo ejerza.
- **S3 (evidencia medible).** Existe un conjunto ejercitable (tests y/o corpus de casos degenerados) que cubre auto-intersección, bordes coincidentes y vértices de contacto, con resultado esperado (normalizado, vacío, o error nombrado) y sin fallos inesperados de normalización.
- **S4 (alineación con lo ya hecho).** 034 C1b abordó parte de esta robustez en boolean/arrangement; esta tarea no duplica lo ya probado: identifica huecos reales frente a S3 y los cierra o los deja explícitamente acotados.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Inventario medido: mapear casos E0.2 ya cubiertos por 034/cam-geom y huecos frente a S1/S3 | S1,S4 | inline | — |
| T2 | Probes deterministas para las clases de entrada abiertas (micro-tramos, vértice repetido no consecutivo, varias fronteras compartiendo arista/vértice, auto-cruce con arcos, contorno sin material), con el oráculo de E0.2: normaliza con identidad de área, o rechaza con variante nombrada; nunca área silenciosamente falsa, pánico, cuelgue ni `EditRefused` que no nombre el constructo (L3) | S1,S3 | worker | — |
| T3 | Fixes de normalización/validación en `cam-geom` para los huecos que T2 mida de verdad (si aplica) | S1,S3 | inline/worker | — |
| T4 | Corregir referencia/criterio E0.2 en `docs/ROADMAP_CAMCRAFT.md` (y nota en GAPS si corresponde) para que el cierre sea verificable | S2 | inline | — |

## Log

- **L1 (pedido, verbatim).** «vamos ahora con el EO.2» / «b» (investigar si algún `refused`/`timeout` del corpus E0.5 es error de normalización y atacar desde ahí) / «1» (abrir/plantear E0.2 como tarea ODD: robustez de normalización en `cam-geom`, casos degenerados + criterio medible, corrigiendo la referencia del roadmap).
- **L2 (hallazgo previo).** Probe del measurer sobre `fixtures/corpus/**`: rechazos por soporte parser/macro/arcos-fuera-de-región/G85, `arrangement revisits a piece` y timeouts; ningún motivo explícito de normalización E0.2. El criterio de ROADMAP referencia “corpus de E0.3” (tolerancia) en lugar del corpus/ejercicio medible (E0.5/degenerados): requiere corrección documental para cierre demostrable.
- **L3 (T1: el inventario, y el alcance que cambia).** Un `gentle-ai-explore` read-only recorrió `cam-geom` con evidencia `path:line`, y el resultado reescribe la tarea: **las tres cosas que E0.2 nombra ya están cubiertas** por 034/C1b/C1c, no son trabajo pendiente.
  - **(a) auto-intersección:** `normalize` corta cada frontera contra sí misma (`boolean.rs:133-174`); fijado en `boolean_corpus.rs:103-121,183-206` y `boolean_operations.rs:365-392`, y detectado por `validate::inspect` (`validate.rs:79-107`).
  - **(b) bordes coincidentes:** `push_fractions` re-corta en `Coincident` (`boolean.rs:214-224`) y `dedup_pieces` colapsa la pieza duplicada (`:607-647,714-763`); fijado en `boolean_corpus.rs` (`shared-edge`, `coincident`, `reversed`), `boolean_operations.rs:101-124,405-413` y `region.rs:403-439`.
  - **(c) vértices de contacto:** el toque entra como vértice del arrangement (`boolean.rs:203-212`), y el ordenamiento por curvatura separa las salidas tangentes (`:419-493`); fijado en `boolean_corpus.rs:166-179,281-316` (tangente interior, discos osculantes) y `boolean_operations.rs:327-363,394-403`.
  **Lo que queda abierto es la entrada previa al arrangement, y es otra cosa:**
  1. **Vértices consecutivos coincidentes** (contorno sin material): lo rechaza el constructor, no el motor — `contour.rs:89-93`, `DegenerateArc{"consecutive vertices coincide"}`; hoy pinneado *como rechazo* (`boolean_corpus.rs:208-229`). No hay ninguna ruta que lo normalice.
  2. **Contorno abierto como frontera:** `Region::new` sólo exige cerrado (`region.rs:91-97`); pinneado como rechazo (`tests/region.rs:192-200`). E0.2 no dice si un gap ≤ tolerancia debería cerrarse: **es una decisión, no un defecto**, y no se toma acá.
  3. **Tramos por debajo de la tolerancia:** `validate::inspect` los reporta (`validate.rs:73-76`) y nada los normaliza; no existe test que fije si el kernel los acepta o los rechaza. Es el candidato más probable a defecto real y por eso T2 lo mide primero.
  4. **`validate` ignora la tangencia por completo** (`validate.rs:100-101`) y no tiene clase "coincidente" propia: la reporta como punto de intersección (`:87-99`). Hueco de inspección, no de geometría; se registra y no se arregla en esta tarea.
  **Consecuencia sobre las tareas:** T2 no es "arreglar auto-intersección" — es fijar el oráculo de E0.2 sobre las clases abiertas; y T3 sólo existe si T2 mide un defecto. El criterio de T2 quedó reescrito con ese oráculo en la tabla de arriba.
- **L4 (T2 y T4: el oráculo fijado, y el defecto que sólo el probe de 1 µm encontró).** `fb182ea` (probes) y `00af97f` (criterio). Diez probes en `crates/cam-geom/tests/boolean_corpus.rs`, 15 tests verdes en ese target, `cargo test -p cam-geom` en verde (226 tests), clippy y fmt limpios.
  **Lo que ya estaba bien, y ahora está fijado:** un micro-tramo de 1e-10 (bajo la tolerancia) normaliza y combina con identidad de área; un vértice repetido **no consecutivo** (pinch) normaliza; dos fronteras de la **misma** región compartiendo una arista completa dan área 200 y tres aristas encontradas en un vértice dan 300; un contorno auto-cruzado **con arco** conserva el arco o declara `flattened` (nunca aplana en silencio); el contorno de ida y vuelta se rechaza **en el constructor** con `consecutive vertices coincide`, que nombra el constructo; y cada caso aceptado es determinista (dos corridas, región idéntica). Cubren S1 y S3 sin duplicar lo que 034 ya fijó (S4).
  **El único defecto real que apareció, y apareció donde no estaba el foco:** un rectángulo con un lado de **1 µm** —mil veces la tolerancia, geometría legal y representable— normaliza bien, pero `combine` contra un rectángulo solapado se rechaza con `EditRefused { reason: "the arrangement revisits a piece, so its pieces cannot be chained closed" }` (`boolean.rs` en `assemble`). Es exactamente la clase que 034 L6 arregló para la tangencia y los discos osculantes, con otra entrada: un tramo cortísimo entre tramos largos produce dos ángulos de salida que la comparación casi no separa. T3 lo ataca; hasta que T3 cierre, ese caso queda pinneado como **rechazo actual** (`micro_edge_one_micron_combine_pins_current_behavior`), que es pin de contrato y no de virtud.
  **T4, el criterio.** El ROADMAP decía «El corpus de `E0.3` no produce errores de normalización», y `E0.3` es la política de tolerancia: un criterio que no se puede verificar contra nada. Ahora nombra el corpus degenerado que sí existe (`boolean_corpus.rs`) con el oráculo de E0.2 —normaliza con identidad de área, o rechaza con una variante que **nombra el constructo**— y deja el corpus de `E0.5` como la segunda mitad de la misma exigencia, en la medida en que el kernel lo ejerza. De paso se corrigió el mismo error de numeración en el párrafo de dependencias: el arnés de paridad es `E0.4` y los benchmarks `E0.7`, no `E0.3`/`E0.4`.
  **Dos decisiones que quedan abiertas y no se toman acá:** (1) si un micro-tramo **por encima** de la tolerancia debe preservarse como material —lo actual— o colapsarse al normalizar; se recomienda preservarlo, porque colapsar cambia el material en silencio; (2) si los rechazos deben propagar qué constructo los disparó en vez de una razón genérica de ensamblado. La (2) es la que E0.2 realmente pide, y T3 la puede dejar a medias sin mentir: se registra acá si así fuere.
