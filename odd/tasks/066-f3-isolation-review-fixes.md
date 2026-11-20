# 066 — f3-isolation-review-fixes

Atender las correcciones/hallazgos indicados por los reviewers de la revisión RDD de F3 (isolation), según lo pedido.

## Surfaces

- `crates/cam-ops/src/isolation.rs`
- `crates/cam-cli/src/commands/isolate.rs`
- `crates/cam-cli/tests/isolate.rs`
- `odd/tasks/066-f3-isolation-review-fixes.md`

## Specs

- **S1 (pedido original, verbatim).** «ahora crea la tarea 066 y haz las correciones que te indico los reviewers»
- **S2 (pedido original, verbatim).** «todas»
- **S3 (hallazgo R2-001, readability, WARNING, informational; claim del reviewer, verbatim).** «The `isolation_passes` documentation describes `kind` and `keep_pours` selection as parameters/behavior of this function, but the function signature does not accept them and it unconditionally delegates to `isolation_passes_with_kind` with `IsolationKind::Full` and `keep_pours` false. This misdocuments the API and obscures the actual contract for callers.» Ubicación: `crates/cam-ops/src/isolation.rs:88-108`.
- **S4 (hallazgo R2-002, readability, WARNING, informational; claim del reviewer, verbatim).** «The test is named and commented as verifying that emitted G-code is accepted by cam-sim, but after writing the program it only asserts non-emptiness and then simulates a separately recomputed move stream (rebuilding copper/moves with hardcoded parameters) instead of validating the emitted output. This obscures what is actually covered and can mislead maintainers about emission correctness.» Ubicación: `crates/cam-cli/tests/isolate.rs:124-197`.
- **S5 (hallazgo R4-F001, resilience, CRITICAL, refuted; claim del reviewer, verbatim).** «The --out path uses a fixed temporary file name derived only from the destination (destination with a \".tmp\" suffix), so concurrent or overlapping isolate invocations targeting the same output write and rename the same temporary file. One run can overwrite another run's temporary content and cause the wrong G-code to be renamed into the destination, or a rename failure can remove the other run's temporary file, making the output non-retry-safe and racy under parallel execution.» Ubicación: `crates/cam-cli/src/commands/isolate.rs:278-286`.
- **S6 (hallazgo R4-F002, resilience, WARNING, informational; claim del reviewer, verbatim).** «During hole-collapse recovery, interior_boundaries_per_hole silently discards unexpected errors from per-hole region construction, normalization, or offset (treating them the same as intentionally skipped exhausted/collapsed holes). If a hole triggers such an error, its interior isolation contour is omitted without any diagnostic while isolation can still succeed, degrading to emitted toolpaths that leave that clearance unisolated with no observable failure signal.» Ubicación: `crates/cam-ops/src/isolation.rs:461-473`.

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Corregir doc de `isolation_passes` para reflejar su contrato real (delegación sin `kind`/`keep_pours`) y aclarar el API | S1,S2,S3 | inline | pending | — |
| T2 | Ajustar test `isolate_emits_gcode_accepted_by_camsim` para que valide la salida emitida y no solo un stream reconstruido aparte | S1,S2,S4 | inline | pending | — |
| T3 | Hacer la escritura de `--out` robusta ante concurrencia/sobrescritura usando nombre temporal único/seguro | S1,S2,S5 | inline | pending | — |
| T4 | Evitar descarte silencioso de errores inesperados en recuperación por colapso de huecos (`interior_boundaries_per_hole`), con diagnóstico observable | S1,S2,S6 | inline | pending | — |
| T5 | Verificar (fmt, clippy, tests relevantes), actualizar este documento y preparar entrega | S1-S6 | inline | pending | — |

## Log

- **L1 (pedido original, verbatim).** «ahora crea la tarea 066 y haz las correciones que te indico los reviewers»
- **L2 (pedido original, verbatim).** «todas»
