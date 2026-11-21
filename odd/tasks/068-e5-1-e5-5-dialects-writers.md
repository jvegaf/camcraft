# 068 — E5.1 catálogos `.pp` + E5.5 escritores Gerber/Excellon

Completar dos ítems de F5 (PCB G-code): E5.1 catálogo de dialectos `.pp` con goldens byte a byte, y E5.5 escritores Gerber/Excellon en `cam-export` con round-trip contra `cam-gerber`/`cam-excellon`.

## Surfaces

- `crates/cam-post/posts/`
- `crates/cam-post/tests/`
- `crates/cam-post/src/lib.rs`
- `crates/cam-export/src/`
- `crates/cam-export/tests/`
- `crates/cam-export/Cargo.toml`
- `docs/ROADMAP_CAMCRAFT.md`
- `odd/roadmap/index.md`
- `odd/tasks/068-e5-1-e5-5-dialects-writers.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa F5»
- **S2 (alcance acotado por el usuario, verbatim).** «1. en esta task vamos con E5.5 y E5.1     2. con los 3 que indicas es suficiente 3. si»
- **S3 (ROADMAP_CAMCRAFT.md F5, verbatim).** «## 8. Fase 5 — G-code de PCB»
- **S4 (ROADMAP_CAMCRAFT.md E5.1, verbatim).** «| E5.1 | Catálogo de dialectos `.pp` de FlatCAM como datos (GRBL, Marlin, Repetier y los de uso real) | **M** | Cada dialecto con golden test byte a byte |»
- **S5 (ROADMAP_CAMCRAFT.md E5.5, verbatim).** «| E5.5 | Escritores Gerber y Excellon en `cam-export` | **M** | Round-trip: exportar e importar recupera la geometría dentro de tolerancia |»
- **S6 (estado actual E5.1, `odd/roadmap/index.md`).** E5.1 está 🟡: parser/emitter y goldens de fixtures existen (`cam-post/src/lib.rs:27,137,268`; fixtures `post_inch.pp`, `post_mm.pp`); falta el dato de dialectos GRBL/Marlin/Repetier. `crates/cam-post/posts/grbl.pp` ya existe como post propio del proyecto.
- **S7 (estado actual E5.5, `odd/roadmap/index.md`).** E5.5 está ⬜: `cam-export/src/lib.rs` solo escribe DXF y SVG; no hay escritores Gerber/Excellon ni round-trip contra `cam-gerber`/`cam-excellon`.
- **S8 (decisión de dialectos, usuario).** El set de dialectos para esta tarea es GRBL, Marlin y Repetier; los tres nombrados son suficientes. Los posts se tratan como datos del proyecto (como `grbl.pp`), con golden test byte a byte por dialecto.

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T1 | E5.5: escritor Gerber en `cam-export` — API de exportación, representación de contornos/aperturas mínimas necesarias, tests unitarios y round-trip contra `cam-gerber` dentro de tolerancia | S1,S2,S3,S5,S7 | inline | done | — |
| T2 | E5.5: escritor Excellon en `cam-export` — tabla de herramientas, hits/slots, unidades/formato, tests unitarios y round-trip contra `cam-excellon` dentro de tolerancia | S1,S2,S3,S5,S7 | inline | done | — |
| T3 | E5.5: cableado de formato/exportación en `cam-export` (Format/errores/docs) y cobertura de round-trip integrada; verificación focal | S1,S5,S7 | inline | done | — |
| T4 | E5.1: añadir dialectos Marlin y Repetier como datos `.pp` en `crates/cam-post/posts/`, con golden test byte a byte por dialecto; confirmar GRBL cubierto por golden | S1,S2,S3,S4,S6,S8 | inline | done | — |
| T5 | Actualizar `odd/roadmap/index.md` y `docs/ROADMAP_CAMCRAFT.md` con el estado honesto de E5.1/E5.5; verificación, registro y cierre | S1-S8 | inline | done | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa F5»
- **L2 (alcance acotado por el usuario, verbatim).** «1. en esta task vamos con E5.5 y E5.1     2. con los 3 que indicas es suficiente 3. si»
- **L3 (clasificación).** F5 completo es grande; el usuario acotó esta tanda a E5.1 (M) y E5.5 (M). Siguen siendo dos entregables en crates distintos (`cam-post`, `cam-export`) con criterio de aceptación propio; se trackea como tarea ODD grande antes del primer write de implementación. E5.2/E5.3/E5.4 quedan fuera de esta tarea.
- **L4 (T1-T3 E5.5, done).** Implementados escritores Gerber y Excellon en `cam-export`: `crates/cam-export/src/gerber.rs` exporta contornos como RS-274X (G36/G37, G01/G02/G03, FSLAX46Y46, MM, LPD) con `export_contours` y `export(Document)`; `crates/cam-export/src/excellon.rs` exporta Drill/Slot como Excellon (M48/METRIC/tool table/holes/slots, M30) con `export_features` y `export(Document)`. `Format`/`export_bytes` cableados en `crates/cam-export/src/lib.rs` (`Gerber`, `Excellon`). Round-trips verificados: `crates/cam-export/tests/gerber_round_trip.rs` (4 tests: rectángulo, círculo, contorno con arco, documento) parsea con `cam-gerber` y compara forma dentro de tolerancia; `crates/cam-export/tests/excellon_round_trip.rs` (3 tests: holes, slots, documento) parsea con `cam-excellon` y compara herramienta/diámetro/hits/posiciones. `cargo test -p cam-export`: 31 passed, 0 failed.
- **L5 (T4 E5.1, done).** Añadidos `crates/cam-post/posts/marlin.pp` y `crates/cam-post/posts/repetier.pp` (datos del proyecto, no vendorizados) y constantes `MARLIN_POST`/`REPETIER_POST` en `crates/cam-post/src/lib.rs`. Goldens byte a byte: `crates/cam-post/tests/marlin.rs` y `crates/cam-post/tests/repetier.rs` (`profile_emits_byte_for_byte` + determinismo); GRBL ya cubierto por `crates/cam-post/tests/grbl.rs`. `cargo test -p cam-post`: 36 passed, 0 failed. Dialectos cubiertos por esta tarea: GRBL, Marlin, Repetier (S8).

- **L6 (T5 verificación).** `cargo fmt -p cam-export -p cam-post -- --check` OK; `cargo clippy -p cam-export -p cam-post --tests -- -D warnings` OK; `cargo test -p cam-export` 31 passed, 0 failed; `cargo test -p cam-post` 36 passed, 0 failed. Actualizado `odd/roadmap/index.md`: E5.1 ✅, E5.5 ✅.
