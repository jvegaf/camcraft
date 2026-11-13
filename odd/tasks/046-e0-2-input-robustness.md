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
| T2 | Corpus/tests degenerados ejercitables (auto-intersección, bordes coincidentes, vértices de contacto) con expectativas explícitas | S1,S3 | inline/worker | — |
| T3 | Fixes de normalización/validación en `cam-geom` para los huecos encontrados (si aplica) | S1,S3 | inline/worker | — |
| T4 | Corregir referencia/criterio E0.2 en `docs/ROADMAP_CAMCRAFT.md` (y nota en GAPS si corresponde) para que el cierre sea verificable | S2 | inline | — |

## Log

- **L1 (pedido, verbatim).** «vamos ahora con el EO.2» / «b» (investigar si algún `refused`/`timeout` del corpus E0.5 es error de normalización y atacar desde ahí) / «1» (abrir/plantear E0.2 como tarea ODD: robustez de normalización en `cam-geom`, casos degenerados + criterio medible, corrigiendo la referencia del roadmap).
- **L2 (hallazgo previo).** Probe del measurer sobre `fixtures/corpus/**`: rechazos por soporte parser/macro/arcos-fuera-de-región/G85, `arrangement revisits a piece` y timeouts; ningún motivo explícito de normalización E0.2. El criterio de ROADMAP referencia “corpus de E0.3” (tolerancia) en lugar del corpus/ejercicio medible (E0.5/degenerados): requiere corrección documental para cierre demostrable.
