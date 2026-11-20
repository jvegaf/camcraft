# 064 — e1-8-vbit-isolation-depth

Cierre del criterio de salida de E1.8/E3.3: derivación de la profundidad de aislamiento para V-bit a partir de ángulo incluido y punta, con prueba del valor esperado para una V-bit de catálogo (30°, punta 0.2 mm).

## Surfaces

- `crates/cam-ops/src/isolation.rs`
- `crates/cam-ops/src/lib.rs`
- `crates/cam-ops/tests/isolation.rs`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa E1.8»
- **S2 (seguimiento, verbatim).** «comitea mergea y haz deliver»
- **S3 (E1.8, `docs/ROADMAP_CAMCRAFT.md:97`, verbatim).** «| E1.8 | Librería de herramientas: V-bit con ángulo y punta, fresa de aislamiento, broca (verificar/cubrir G2.5) | **S** | Una V-bit del catálogo produce la profundidad esperada en isolation |»
- **S4 (E3.3, `docs/ROADMAP_CAMCRAFT.md:137`, verbatim).** «| E3.3 | Profundidad derivada del V-bit (ángulo y punta) | **M** | Una V-bit de 30° y 0.2 mm de punta da profundidad calculada; test con el valor esperado |»
- **S5 (`odd/roadmap/index.md`, estado previo E1.8, verbatim).** «The exit criterion is *a catalogue V-bit produces the expected depth in isolation*, and that is the isolation **operation**, which belongs to F3 (E3.x) and does not exist yet: W2d delivered the vocabulary the operation will read, not the operation»
- **S6 (`odd/roadmap/index.md`, estado previo E3.3, verbatim).** «`IsolationParams` carries `cut_depth` only (`cam-model/src/lib.rs:329`)» / «Depth derived from included angle and tip»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Implementar `vbit_isolation_depth_mm(target_diameter_mm, included_angle_deg, tip_diameter_mm)` en `cam_ops::isolation`, derivando profundidad de aislamiento desde ángulo incluido y punta/plano; exportar y validar casos inválidos | S3,S4,S6 | inline | — |
| T2 | Agregar tests: valor esperado para V-bit 30°/punta 0.2 mm (profundidad calculada), caso de catálogo produciendo profundidad esperada en isolation, borde en ancho de punta y rechazo bajo la punta | S3,S4 | inline | — |
| T3 | Actualizar `odd/roadmap/index.md` para reflejar E1.8/E3.3 completos con evidencia (sin modificar INDEX) | S1,S3,S4,S5,S6 | inline | — |
| T4 | Verificación focalizada (`cargo test -p cam-ops --lib isolation`, `--test isolation`, `cargo fmt -p cam-ops`), review nativo RDD, commit/merge/deliver según pedido | S1,S2 | inline | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa E1.8»
- **L2 (seguimiento, verbatim).** «comitea mergea y haz deliver»
- **L3 (implementación y evidencia).** Se agregó `vbit_isolation_depth_mm` en `crates/cam-ops/src/isolation.rs` (exportado en `crates/cam-ops/src/lib.rs`): `depth = max(0, target - tip) / (2*tan(angle/2))` para ancho efectivo de aislamiento `tip + 2*depth*tan(angle/2)` (mm, ángulo incluido en grados), con validaciones de finitud/rango y rechazo si `target < tip`. Tests: `isolation::tests::vbit_isolation_depth_for_30deg_tip_0_2mm` y `catalogue_vbit_produces_expected_isolation_depth` (`crates/cam-ops/tests/isolation.rs`) fijan V-bit 30° / punta 0.2 mm ⇒ ancho efectivo objetivo `0.7358983848622454 mm` ⇒ profundidad **1.0 mm**, más profundidad 0 en punta y rechazo bajo la punta. Verificación: `cargo test -p cam-ops --lib isolation` (2 passed), `cargo test -p cam-ops --test isolation` (10 passed), `cargo fmt -p cam-ops` OK. `cargo clippy -p cam-ops --all-targets -D warnings` bloqueado por warning preexistente ajeno en `cam-geom/src/boolean.rs:2051`.
- **L4 (documentación y review).** `odd/roadmap/index.md` actualizado: E1.8 y E3.3 marcados `✅` con evidencia; F1 pasa a E1.4 como único parcial. RDD lineage `review-5ca37fdf13a943ed` (target `sha256:a003bf335f76b6477e779c8ea2dae0e18be3d1a939306ecc9d710deedf287fd1`, lenses `review-resilience`,`review-reliability`) aprobada y reconocida (`acknowledge-approved`, authority burned).
