# 094 — Deuda de advisories de review (feat/tauri)

Lista consolidada de hallazgos **informativos/no-bloqueantes** (advisories) de las reviews RDD
por slices de `feat/tauri`. No incluyen hallazgos corregidos en el mismo instante ni CRITICAL
refutados. Se cierra al final de la migración con una pasada de corrección dedicada.

## Surfaces

- `odd/tasks/094-review-advisory-debt.md`
- `odd/tasks/INDEX.md`
- Código afectado según cada advisory al momento de cerrar (R# correspondiente)

## Specs

- **S1 (acuerdo con el usuario).** Los advisories WARNING/informativos de las reviews por batch
  se acumulan en esta lista y se corrigen al final de la migración, salvo que uno se corrija
  en el mismo momento en que aparece.
- **S2.** Cada entrada registra: id, lente, ubicación, batch/lineage de origen y qué falta.

## Tasks

| ID  | Tarea                                                  | Specs | Ruta   | Commit |
| --- | ------------------------------------------------------ | ----- | ------ | ------ |
| T1  | Crear la lista y sembrar los advisories de batches 1-7 | S1,S2 | inline | (este) |
| T2  | Corregir cada advisory pendiente y marcar estado       | S1    | inline | —      |

## Log

- **L1 (pedido original, verbatim).** «si arma la lista de deuda y le vamos agregando los que vayan saliendo siempre y cuando no sea para corregirlo en ese mismo instante»

### Batch 1 — `origin/master..4b53b4f` (review-2173b4080566240c)

| ID                           | Lente       | Ubicación                                     | Resumen                                                                                                                        |
| ---------------------------- | ----------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| R1-001                       | risk        | `crates/cam-cli/src/commands/export.rs:72-84` | Export escribe a path temporal predecible con `fs::write` (sigue symlinks); symlink pre-creado permite sobrescribir el target. |
| R3-records-partial-parse     | reliability | `scripts/check-records.sh:51-74`              | Fallo de python3 tras output parcial puede dejar pasar el gate con filas incompletas.                                          |
| R3-timeout-doc-inconsistency | reliability | `scripts/parity/README.md:86-88`              | README dice 20s, código usa 30s (`collect.py:52`); roadmap repite la inconsistencia.                                           |
| R4-001                       | resilience  | `.github/workflows/ci.yml:82`                 | `git diff` sobre `github.event.before` sin fallback ante push forzado; job de parity falla en vez de degradar a medir.         |
| R4-002                       | resilience  | `scripts/check-records.sh:181,193`            | `engram search` sin guardar exit code bajo `set -euo pipefail`; falla closed pero pierde el summary.                           |

### Batch 2 — `4b53b4f..c5acd4f` (review-575701d5908b897b)

| ID                               | Lente       | Ubicación                                      | Resumen                                                                                                                                                           |
| -------------------------------- | ----------- | ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R1-drill-nan-depth-validation    | risk        | `crates/cam-cli/src/commands/drill.rs:185-189` | `parse_number` acepta NaN/Inf para `--cut-depth/--peck-depth/--start-depth`; NaN burla guards y produce G-code malformado.                                        |
| R1-drill-non-atomic-write        | risk        | `crates/cam-cli/src/commands/drill.rs:167-169` | `drill --out` usa `std::fs::write` directo (no atómico); un `.nc` truncado puede ejecutarse como programa completo. Cutout en el mismo cambio sí usa temp+rename. |
| R3-001                           | reliability | `crates/cam-ops/src/excellon_drill.rs:241-258` | `route_slots` no rechaza `RetractMode::Amount` no finito o negativo; retract Z inseguro/no finito.                                                                |
| R3-002                           | reliability | `crates/cam-cli/src/commands/drill.rs:168-172` | Mismo write no atómico de `--out` (duplicado de R1-drill-non-atomic-write).                                                                                       |
| R3-003                           | reliability | `crates/cam-cli/src/commands/drill.rs:185-189` | `parse_number` acepta no-finitos; fluyen a parámetros de mecanizado. (mismo tema que R1-drill-nan-depth-validation)                                               |
| R4-drill-non-atomic-output-write | resilience  | `crates/cam-cli/src/commands/drill.rs:169-171` | Write no atómico puede dejar G-code truncado tras error (sin footer M5/M30). (mismo tema)                                                                         |

### Batch 4 — `1f61cfa..3e2eae1` (review-110b47af8d9c69f1)

| ID     | Lente       | Ubicación                             | Resumen                                                                                                                                 |
| ------ | ----------- | ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| R3-001 | reliability | `scripts/baseline/pin.py:77-86`       | `replace_all` deja temp file huérfano en el directorio baseline si falla un paso intermedio (staged solo se agrega tras escribir todo). |
| R4-001 | resilience  | `scripts/baseline/collect.py:152-157` | Timeout mata solo el hijo directo, no el process group; workers de FlatCAM quedan huérfanos consumiendo CPU/memoria.                    |

### Batch 5 — `3e2eae1..4ed8d8e` (review-f9b99613922e7e9b)

| ID                   | Lente       | Ubicación                              | Resumen                                                                                                                                                                        |
| -------------------- | ----------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| R3-001               | reliability | `crates/cam-gerber/src/lib.rs:311`     | Recuperación de región rota no guarda el rechazo de línea de largo cero dentro de región: región ya marcada rota + Line degenerada aborta el archivo entero en vez de ignorar. |
| R4-image-silent-skip | resilience  | `crates/cam-gerber/src/lib.rs:204-205` | `Gerber::image()` devuelve geometría parcial descartando constructs saltados; caller no puede detectar geometría omitida sin diagnóstico.                                      |

### Batch 6a — `4ed8d8e..4714f49` (review-a35c137faa633ad3)

Sin advisories.

### Batch 6b — `4714f49..420b2cb` (review-a794aa6be01f9c65)

| ID                      | Lente       | Ubicación                                 | Resumen                                                                                                                                      |
| ----------------------- | ----------- | ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| R3-001                  | reliability | `crates/cam-gerber/src/lib.rs:1621-1623`  | Exención de región aplica a D01-D03: flash D03 dentro de G36/G37 sin aperture se acepta y descarta en silencio en vez de NoApertureSelected. |
| R3-002                  | reliability | `crates/cam-model/src/lib.rs:272-287`     | Nuevos Drill/Slot sin validación de coordenadas finitas ni diámetro/width positivo; NaN/degenerados alcanzan bounds/transform.               |
| R3-003                  | reliability | `crates/cam-app/src/command.rs:1189-1195` | Drill/Slot en transform/array-copy/align sin tests directos a nivel command.                                                                 |
| R3-004                  | reliability | `crates/cam-export/src/dxf.rs:189-196`    | Export DXF/SVG de Drill/Slot sin tests directos.                                                                                             |
| R4-SVG-SLOT-SILENT-DROP | resilience  | `crates/cam-export/src/svg.rs:386-389`    | SVG export descarta un slot en silencio si falla la construcción stadium; retorna Ok sin geometría ni aviso.                                 |

### Batch 7 — `420b2cb..9e9d865` (review-d63ad080c1bf1aa9)

| ID                            | Lente       | Ubicación                                | Resumen                                                                                                                                                                             |
| ----------------------------- | ----------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R3-flush-batch-merge-fallback | reliability | `crates/cam-gerber/src/lib.rs:686`       | Si la unión batched en árbol logra unir pero el merge a imagen falla, `flush_pending_unions` propaga error sin fallback secuencial por región.                                      |
| R3-tip-snap-first-pass-guard  | reliability | `crates/cam-geom/src/boolean.rs:358-366` | `push_fractions` registra snaps endpoint near-miss en rango (eps, 2*eps] también en el primer combine pass, donde `detect_near_misses` es false; cambia identificación de vértices. |
| R4-001                        | resilience  | `crates/cam-gerber/src/lib.rs:657-719`   | Batching de dark constructs pierde el índice de comando Gerber: errores reportan línea 0 (Invalid) en vez del comando origen.                                                       |

### Batch 8/9/10 — docs+evidence y geom chico

Not-due (`under_budget`), sin review.

### Batch 11a — `085d55d..957692c` (review-620213087430eba3)

| ID     | Lente       | Ubicación                                  | Resumen                                                                                                                                                           |
| ------ | ----------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R3-001 | reliability | `crates/cam-geom/src/boolean.rs:2014-2019` | Near-miss retry: far-flip removal corre siempre que exista far-flip y adopta el set reducido como baseline; caso assemblable original puede volverse unchainable. |
| R4-001 | resilience  | `scripts/parity/collect.py:176-181`        | Skip incondicional de artefactos con golden pinned skipped; crashes/parse errors/regresiones en esos 30 artefactos no se detectan hasta un-skip manual.           |

### Batch 11c — `dc3196e..8ff8d50` (review-9b3e4a593289c812, ESCALADO)

Review terminada en stop terminal (`native_stop_required`): el refuter corroboró R3-001,
R4-001 y R4-002 (CRITICAL, deterministas) y R1-001 quedó con causalidad desconocida. Sin ruta
de corrección nativa; se corrigieron por fuera del lineage en `7714fc1`:

| ID     | Lente       | Ubicación                                   | Resolución                                                                                                        |
| ------ | ----------- | ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| R3-001 | reliability | `cam-ops/src/isolation.rs` (`unite_copper`) | Corregido en `7714fc1`: polaridad por profundidad de anidamiento, no por winding.                                 |
| R4-001 | resilience  | `cam-ops/src/isolation.rs:43`               | Corregido en `7714fc1` (misma raíz: clasificación additive/subtractive por signed_area).                          |
| R4-002 | resilience  | `cam-ops/src/isolation.rs` (keep_pours)     | Corregido en `7714fc1`: la omisión total de paths interiores por keep_pours ya no se reporta como collapse falso. |
| R1-001 | risk        | (no verificado)                             | Causalidad desconocida; pendiente de diagnóstico manual.                                                          |

Hallazgos adicionales del writer (deuda, ver `094`):

- Hole-collapse recovery es código muerto: `is_hole_collapse_error` y `is_clearance_exhausted`
  matchean strings que ningún error de cam-geom emite; todo el fix 066/S6 nunca corre.
- El predicado de pours es tan amplio que `keep_pours` descarta todo path interior en cualquier
  board con clearance anidada (probe corpus: 38/38 descartados). Cumple el contrato documentado,
  pero el comportamiento merece decisión de diseño.
- Paths de fixtures stale (`../..`) en `cam-ops/tests/ncc.rs`, `excellon_drill.rs` y
  `cam-geom/tests/parity.rs` (fuera de superficies del writer).

### Batch 12 — `8ff8d50..a949ebe` (NCC + export writers, TRES generaciones)

Review en tres generaciones con dos rondas de corrección (200 líneas cada una, al tope):

- Gen 1 `review-7807a3bb21e8599d`: 6 hallazgos corregidos en el worktree de revisión —
  band area real en `outline_region_to_board` (R1-001/R3-001/R4-001), Lines con spans abiertos
  bypassing Region::normalize (R3-002/R4-002), seed inseteado por el radio (R3-003).
- Gen 2 `review-2f0218adf3d619cc`: 6 CRITICAL nuevos corregidos — fill_component propaga errores
  de offset no-exhaustion (R1-001/R4-002), doble inset en Lines eliminado (R3-001/R4-003),
  containment check en stroke-ring (R3-002), contrato de error de `Region::offset` restaurado (R4-001).
- Gen 3 `review-7b4a2d46920b56e7`: ESCALADA terminal — R3-001 con causalidad desconocida
  (claim no retransmitido; pendiente de diagnóstico manual). Riesgo/resilience admitieron limpios.

Pendiente registrado: diagnosticar R3-001 del gen 3 (el diff corregido está en
`/tmp/camcraft-rdd-slice` commits `97506aa`/`e728db4` → replicar en `feat/tauri`).

### Batch 13 — `a949ebe..bb90f0c` (review-d995330573914023)

| ID     | Lente       | Ubicación                            | Resumen                                                                                                                    |
| ------ | ----------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| R3-002 | reliability | `cam-cli/src/commands/cutout.rs:335` | drill_moves cuenta detours de exc-areas como drill (cutout_moves pre-routing).                                             |
| R3-003 | reliability | `cam-post/src/emit.rs:377`           | Toolchange por estilo escribe fuera del path de numeración: líneas sin número con LINE_NUMBER_*.                           |
| R3-004 | reliability | `cam-ops/src/exc_travel.rs:71-80`    | Detours de Retract emitidos como Move::Retract con X/Y; posts RETRACT_MOVE solo emiten Z → el XY del detour no se ejecuta. |
| R4-003 | resilience  | `cam-cli/src/commands/cutout.rs:335` | Duplicado de R3-002.                                                                                                       |

Refutados: R3-001, R4-001 (MarlinM6/RepetierPause spindle), R4-002 (muestreo 64).

### Batch 14 — `bb90f0c..c4ce3a1` (review-fabe6e03b93ed641, approved tras 2 correcciones)

Corregido en el worktree de revisión (2 rondas): inspector overlay al colapsar (R3-001 gen 2),
sweep sampling acotado (R3-001/R4-SWEEP-UNBOUNDED gen 1; R4-SWEEP-STEP gen 2). Pendiente de
replicar en `feat/tauri`: sweep step clamp + inspector early-return.

| ID     | Lente       | Ubicación                      | Resumen                                                                                                                                                                                        |
| ------ | ----------- | ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R3-002 | reliability | `cam-sim/src/sweep.rs:150-155` | Con el cap de 4096 pasos, intervalos efectivos > 0.5 mm en viajes largos con radio chico pueden saltarse regiones estrechas de colisión; el test nuevo solo prueba tiempo/crash, no precisión. |

### Batch 16a/16b — `c4ce3a1..a1748bb` (E6.1–E6.5, approved con 2 correcciones)

16a (`review-4680899e6cdde0de`): R4-copper-fill-freeze corregido (earclip solo para contornos
chicos; fan para cualquier contorno de muchos vértices).
16b (`review-7e8b300d54d163cf`): R3-001 corregido (stable tool_id roto ya no cae a tool_number).

Ambas correcciones replicadas en `feat/tauri` (`f15bf60`, store fix).

| ID            | Lente                  | Ubicación                                  | Resumen                                                                                                 |
| ------------- | ---------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------- |
| R3-002/R4-001 | reliability/resilience | `cam-ui/src/tool_library/shell.rs:328-335` | open_tool_library mantiene snapshot stale si SQLite falla; el modal no distingue tools vivas de viejas. |
| R3-003        | reliability            | `cam-ui/src/shell/input.rs:313`            | Ctrl+Shift+S también ejecuta Save (pisa sin dialogo); falta test.                                       |

### Batch 15a — `7bced6e` (visual review + goldens/design)

**Sin review por decisión explícita del usuario**: el contenido binario/HTML de design/ excede el
presupuesto de lentes incluso como commit único.

### Batch 17 — `a1748bb..193269e` (review-8b0cf3d9296fe4f0, ESCALADO)

3 CRITICAL corregidos (cutout preview reusa cam_ops::cutout con params reales; uncut calculado
desde el heightfield contra la profundidad declarada; todo toolpath dropeado reporta warning).
El validador rechazó la corrección porque `collect_contours` seguía saltándose una entidad
faltante sin warning → lineage escalado/terminal. El gap se cerró al replicar en `feat/tauri`
(`3ea400a`, warning por entidad faltante/no-contour en `collect_contours`).

Follow-ups (del validator y reviewers, ver 094):

- R3-004: max_depth del gouge considera toolpaths dropeados (enmascara sobre-profundidad).
- R3-005: push_segment hardcodea feed/plunge/spindle; el preview ignora las rates guardadas.
- R3-002: patrón continue-silencioso también en NCC y missing-entity IDs de collect_contours
  (cerrado en `3ea400a` para el caso de entidades).

### Batch 18 — `193269e..b707a04` (review-b239ce7602b7f336, ESCALADO)

Spikes Tauri/GPUI + plan migración + script ui-review-sway. R3-001 (unknown causality) paró la
review; R3-001/R3-002 corroborados en `cam-ui` palette/panels (egui::Id::new deprecado → Id::unique,
fix ya presente en feat/tauri vía egui moderno). Advisories a deuda:

| ID     | Lente       | Ubicación                           | Resumen                                                                                                                    |
| ------ | ----------- | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| R3-004 | reliability | `scripts/ui-review-sway.sh:237`     | set -e aborta en el diff Python; el rc=$? nunca se alcanza y el loop de escenas para en la primera falla.                  |
| R3-005 | reliability | `scripts/ui-review-sway.sh:206`     | Threshold 16 + diff_px==0 tratado como match exacto: regresión visual uniforme se reporta OK contradiciendo el comentario. |
| R4-001 | resilience  | `scripts/ui-review-sway.sh:127-238` | Ídem R3-004: una escena fallida esconde las demás y aborta el run.                                                         |
| R4-002 | resilience  | `scripts/ui-review-sway.sh:109-116` | Wait loop sin flag de éxito: un fallo de build/launch se reporta como falla visual tras el wait completo.                  |

### Batch 20 — `24eaa31..b3f2f9c` (review-1bbeaee3ea6c0406, ESCALADO)

El refuter confirmó R4-002: el nuevo `scripts/check.sh` eliminó fmt/clippy/workspace-tests del gate
(solo cargo check + biome + svelte-check + vitest con passWithNoTests). **Gap cerrado después por
el batch 23** (`83758ce..0714240`, gates alineados a workspace: fmt/clippy/test vuelven al gate).
Advisories:

| ID     | Lente       | Ubicación             | Resumen                                                                                      |
| ------ | ----------- | --------------------- | -------------------------------------------------------------------------------------------- |
| R3-004 | reliability | `vitest.config.ts:13` | passWithNoTests: true permite gate verde con 0 tests frontend.                               |
| R4-003 | resilience  | `scripts/check.sh:39` | Vitest reintenta cualquier falla y descarta el stderr del primer run: flaky pasa sin rastro. |

### Batch 23 — move de crates (`59dcf88..0714240`)

Ya revisado por RDD en 4 slices al momento del move (lineages `review-a8e5725233b9ca13`,
`review-26f3ef70480a73b9`, `review-90a204de223801ee`, `review-7b6256d3e8215066`).

### Batches 24-27 — biome batches, Fase 0 docs, Fases 1-4

Cada commit ya tiene receipt individual de su entrega (assess not-due/under_budget o review
cerrada por commit; `f29731f` review-c749e0c862e54104; `d6a88f6` passive). Sin nueva revisión.

### Batch 19 — `24eaa31` scaffolding (17.734 líneas, 391 archivos)

**Sin review por decisión explícita del usuario**: no entra en el presupuesto ni partido.

### Cobertura final de feat/tauri (281 commits)

- Approved: batches 1, 2, 4, 5, 6a, 6b, 7, 11a, 11b (con corrección), 13, 14 (con 2 correcciones),
  16a (con corrección), 16b (con corrección).
- Escalados con correcciones aplicadas y replicadas en feat/tauri: 12 (12 hallazgos), 17 (3 + gap
  del validador cerrado), 18 (advisories).
- Escalado sin corrección nativa: 11c (cerrado por fix manual replicado), 20 (gap de gate cerrado
  por batch 23).
- Not-due: 3, 8, 9, 10, 21, 22.
- Sin review por decisión del usuario: 15a (assets), 19 (scaffolding).
- Ya revisados al momento: 23 (move) y 24-27 (entrega por commit).

## Agrupación propuesta para T2

1. **cam-cli export/drill**: R1-001 (symlink/atomic), drill NaN validation, drill atomic write, retract Amount validation (R3-001 batch2). — riesgo real, primero.
2. **cam-gerber**: región rota/line degenerada (batch5 R3-001), flash D03 sin aperture (6b R3-001), batching fallback/línea 0 (batch7), silent skip image (batch5 R4), slot SVG drop (6b R4).
3. **cam-model/app/export**: validación Drill/Slot + tests faltantes (6b R3-002/003/004).
4. **cam-geom**: tip-snap first-pass guard (batch7).
5. **scripts/CI**: check-records partial parse + engram guard (batch1), parity README 20s/30s (batch1), CI fallback push base (batch1), pin.py temp huérfano, collect.py process group (batch4).
