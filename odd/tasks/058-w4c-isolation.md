# 058 — W4c Aislamiento de cobre (E3.1, E3.2)

Ola 4, ítem W4c: unión previa del cobre de la capa y offset multipasada acumulado para aislamiento, con dirección por tipo de fresado.

## Surfaces

- `crates/cam-ops/src/isolation.rs`
- `crates/cam-ops/src/lib.rs`
- `crates/cam-ops/tests/`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md y completa w4c»
- **S2 (waves.md W4c, verbatim).** «| W4c | **E3.1 + E3.2** | Unión de cobre previa y multipasada acumulada con la ley de offset y dirección por tipo de fresado | `crates/cam-ops/src/isolation.rs` | W3, W2a, W1a |»
- **S3 (ROADMAP_CAMCRAFT.md E3.1, verbatim).** «| E3.1 | Unión previa del cobre de la capa (requiere E0.1) | **M** | La unión de una capa del corpus coincide en área con FlatCAM dentro de tolerancia |»
- **S4 (ROADMAP_CAMCRAFT.md E3.2, verbatim).** «| E3.2 | Offset multipasada acumulado: pasada `i` a distancia `dia·((2i+1)/2) − i·overlap·dia`, dirección por tipo de fresado | **L** | 3 pasadas con overlap 0.5 producen anillos sin colisión entre sí y sin gouge |»
- **S5 (contexto Fase 3, ROADMAP_CAMCRAFT.md).** «**Objetivo.** El primer producto que un usuario de PCB quiere, y el que más exige del kernel.»

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | Explorar APIs y requisitos concretos de E3.1/E3.2 (geom boolean/offset/tolerance, modelo, patrones ops) | S1-S4 | inline | done | — |
| T2 | Implementar unión de cobre de capa (E3.1) en `isolation.rs` con API y tests | S3 | inline | done | — |
| T3 | Implementar offset multipasada acumulado con fórmula y dirección por tipo de fresado (E3.2) con tests | S4 | inline | done | — |
| T4 | Exportar/integrar en `cam-ops`, verificar (fmt/clippy/tests) y documentar cierre | S1-S4 | inline | done | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md y completa w4c»
- **L2 (contexto).** Worktree `camcraft/w4c` (`~/.herdr/worktrees/camcraft/w4c`, rama `w4c`, HEAD base `1a24e84`). Waves define W4c = E3.1+E3.2 en `crates/cam-ops/src/isolation.rs` con deps W3/W2a/W1a entregadas (052/050/045). Tarea 058 creada y reclamada (`just claim 058`, superficies `isolation.rs`, `lib.rs`, `tests/`). Exploración: `boolean::combine`/`Combined` para unión, `Region::offset` con `TolerancePolicy` explícita (E0.3), `OffsetOptions`/distancia negativa para crecer cobre, fórmula `dia*((2*i+1)/2)-i*overlap*dia`, `MillingDirection::{Climb,Conventional}` en modelo.
- **L3 (T2,T3 implementación).** `crates/cam-ops/src/isolation.rs`: `unite_copper` (E3.1) une contornos de cobre por `BooleanOp::Union` con normalización; `isolation_passes` (E3.2) calcula offsets desde el cobre original con la ley de pasadas, omite pasadas colapsadas tras la primera, orienta contornos según `MillingDirection` (climb invierte). `pass_distance` expone la fórmula. Exportado en `crates/cam-ops/src/lib.rs`. Tests en `crates/cam-ops/tests/isolation.rs`: unión (vacía, simple, solapada, disjunta), fórmula de distancias, rechazo de parámetros inválidos, 3 pasadas con overlap 0.5 sin gouge ni colisión entre anillos (S4), y orientación por fresado sin cambio geométrico.
- **L4 (T4 verificación).** `cargo fmt -p cam-ops --check` OK; `cargo clippy -p cam-ops --all-targets -- -D warnings` OK; `cargo test -p cam-ops` OK (incluye `isolation` 9 passed y suites existentes: excellon_drill 4, follow 27, peck 13, peck_targets 5, penetration 5, rings 15, rings_islands 7, rings_raster_islands 5). Riesgo: `Risk: none` (módulo nuevo en `cam-ops`, sin cambios en contratos compartidos existentes ni ítems de alto riesgo de la lista).
