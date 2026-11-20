# 065 — f3-isolation

Completar todo lo que falta de F3 (Isolation): criterio de E3.1 sobre capa del corpus, guarda anti-gouge (E3.4), selección/ejecución de `iso_type` y `keep_pours` (E3.5), y `cam-cli isolate` con ruta de post-procesado aceptada por `cam-sim` (E3.6), incluyendo el cableado de profundidad V-bit en el flujo de aislamiento.

## Surfaces

- `crates/cam-ops/src/isolation.rs`
- `crates/cam-ops/src/lib.rs`
- `crates/cam-ops/src/shared.rs`
- `crates/cam-ops/tests/isolation.rs`
- `crates/cam-ops/Cargo.toml`
- `crates/cam-cli/src/commands/isolate.rs`
- `crates/cam-cli/src/commands/mod.rs`
- `crates/cam-cli/src/lib.rs`
- `crates/cam-cli/tests/isolate.rs`
- `crates/cam-cli/Cargo.toml`
- `odd/roadmap/index.md`
- `odd/tasks/065-f3-isolation.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa todo lo que falte de F3»
- **S2 (ROADMAP_CAMCRAFT.md E3.1, verbatim).** «| E3.1 | Unión previa del cobre de la capa (requiere E0.1) | **M** | La unión de una capa del corpus coincide en área con FlatCAM dentro de tolerancia |»
- **S3 (ROADMAP_CAMCRAFT.md E3.3, verbatim).** «| E3.3 | Profundidad derivada del V-bit (ángulo y punta) | **M** | Una V-bit de 30° y 0.2 mm de punta da profundidad calculada; test con el valor esperado |»
- **S4 (ROADMAP_CAMCRAFT.md E3.4, verbatim).** «| E3.4 | Guarda anti-gouge: si el ratio de penetración supera el umbral, reparar o fallar con diagnóstico | **M** | Inyectar un caso de penetración hace fallar la operación con mensaje accionable |»
- **S5 (ROADMAP_CAMCRAFT.md E3.5, verbatim).** «| E3.5 | `iso_type` exterior/interior/completo y opción de conservar pours | **M** | Selector verificado en una capa con pours del corpus |»
- **S6 (ROADMAP_CAMCRAFT.md E3.6, verbatim).** «| E3.6 | Comando `cam-cli isolate` | **M** | G-code generado desde CLI, aceptado por `cam-sim` |»
- **S7 (`odd/roadmap/index.md` F3 E3.1 faltante, verbatim).** «**The criterion is not met yet**: it asks for a *corpus layer* whose union agrees in area with FlatCAM within tolerance, and nothing compares `unite_copper`'s area against `fixtures/baseline/` or measures a real corpus layer — the four tests are synthetic contours. The union step also has no CLI/operation consumer yet (E3.6)»
- **S8 (`odd/roadmap/index.md` F3 E3.3 faltante, verbatim).** «`IsolationParams` still stores `cut_depth` as an explicit value (`cam-model/src/lib.rs:329`); wiring automatic derivation into an isolation operation/CLI flow is F3 scope (E3.6)»
- **S9 (`odd/roadmap/index.md` F3 E3.4 faltante, verbatim).** «Repair-or-fail with an actionable diagnosis»
- **S10 (`odd/roadmap/index.md` F3 E3.5 faltante, verbatim).** «Nothing selects or executes them»
- **S11 (`odd/roadmap/index.md` F3 E3.6 faltante, verbatim).** «The command and its post-processor path»

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | E3.1: probar que la unión de una capa de cobre del corpus coincide en área con FlatCAM (`fixtures/baseline/`) dentro de tolerancia, con evidencia reproducible | S1,S2,S7 | inline | pending | — |
| T2 | E3.4: implementar guarda anti-gouge en el flujo de aislamiento (reparar o fallar con diagnóstico accionable) y test de penetración inyectada | S1,S4,S9 | inline | pending | — |
| T3 | E3.5: seleccionar/ejecutar `iso_type` exterior/interior/completo y opción `keep_pours`, verificado en una capa con pours del corpus | S1,S5,S10 | inline | pending | — |
| T4 | E3.6: implementar ejecución de aislamiento (movimientos/programa) con cableado de profundidad V-bit cuando corresponda, comando `cam-cli isolate` y post-procesado; G-code aceptado por `cam-sim` | S1,S3,S6,S8,S11 | inline | pending | — |
| T5 | Actualizar `odd/roadmap/index.md` (F3 completo) y verificar (fmt, clippy, tests relevantes) | S1-S11 | inline | pending | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa todo lo que falte de F3»
