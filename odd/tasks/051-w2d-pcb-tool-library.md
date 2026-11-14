# 051 — w2d-pcb-tool-library

Librería PCB (E1.8 / W2d): fresa de aislamiento como tipo real, V-bit con ángulo y punta end-to-end, y `tools add` completo en CLI.

## Surfaces

- `crates/cam-store/`
- `crates/cam-cli/src/lib.rs`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md, eres el encargado de completar w2d»
- **S2 (W2d en el mapa de olas, verbatim).** «| W2d | **E1.8 librería PCB** | Fresa de aislamiento como tipo real, V-bit con ángulo y punta end-to-end, `tools add` completo | `crates/cam-store/`, `crates/cam-cli/src/lib.rs` | — |»
- **S3 (E1.8 en el roadmap, verbatim).** «| E1.8 | Librería de herramientas: V-bit con ángulo y punta, fresa de aislamiento, broca (verificar/cubrir G2.5) | **S** | Una V-bit del catálogo produce la profundidad esperada en isolation |»
- **S4 (índice de roadmap, brecha observada, verbatim).** «| E1.8 PCB tool library | 🟡 | `cam-store/src/tools.rs:174` geometry carries `included_angle`, `flat_diameter`, `tip_radius`, `ToolKind::{VBit, Drill, …}` | `cam-cli tools add` writes the basics only (`cam-cli/src/lib.rs:598`); no isolation-cutter type and no PCB workflow |»
- **S5 (superficie/colisión de ola, verbatim).** «**Colisión declarada y cómo se resuelve:** W2d toca `cam-cli` (el `tools add`). Nadie más lo toca en esta ola: el `import` de W2c ya existe y no cambia.»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Agregar `ToolKind::IsolationMill` como tipo real en `cam-store` (código/slug/label/ALL/from_slug/from_shape/of, match arms y consumidores), con soporte en serialización `tool_file` y validación/edición; preservar compatibilidad con filas y archivos existentes | S2,S3,S4 | inline | — |
| T2 | Asegurar V-bit con ángulo y punta end-to-end: `included_angle` y punta (`flat_diameter`, y geometría relevante) creables/validables/persistibles/serializables sin pérdida, coherente con el modelo existente | S2,S3 | inline | — |
| T3 | Hacer `tools add` completo en `cam-cli/src/lib.rs`: aceptar kind y parámetros geométricos/de corte necesarios (p. ej. ángulo, punta/flat, flutes, longitudes, tip/shank, feed/plunge/spindle/stepdown/stepover, name-format), validar y guardar con las APIs de `tool_edit`/store, y actualizar ayuda/listado según corresponda | S2,S4,S5 | inline | — |
| T4 | Tests y docs: cubrir nuevo tipo, round-trip V-bit (ángulo/punta) end-to-end en store/archivo/CLI según aplique, y casos de `tools add`; actualizar referencias de ayuda si cambian. Verificación focalizada y regresión relevante | S2,S3,S4 | inline | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md, eres el encargado de completar w2d»
- **L2 (implementación y verificación, T1-T4).** `cam-store`: se agregó `ToolKind::IsolationMill` como tipo real (`raw=7`, slug `isolation-mill`, label `Isolation mill`, incluido en `ALL`) y se actualizaron los match/lectores coherentes; el kind se preserva por código/slug y round-trip en `tool_file` (`crates/cam-store/src/tools.rs`, `tool_file.rs` vía `ToolKind` API). V-bit conserva `included_angle` y punta (`flat_diameter`) end-to-end en store/archivo/validación existente; `tools add` ahora puede crearlos con esos campos. `cam-cli`: `tools add` acepta `--kind` (slug) y parámetros geométricos/de corte (`--flutes`, `--included-angle`, `--flat-diameter`, `--tip-radius`, `--flute-length`, `--shank-diameter`, `--overall-length`, `--feed`, `--plunge`, `--spindle`, `--stepdown`, `--stepover`) además de los existentes, valida/guarda con `ToolLibrary::save_tool` y actualiza la ayuda (`crates/cam-cli/src/lib.rs`). Tests agregados: `an_isolation_mill_stays_a_real_type_through_a_file` (`crates/cam-store/tests/tool_file.rs`), `isolation_mill_is_a_real_kind_with_stable_identity` (`crates/cam-store/tests/tool_edit.rs`), `tools_add_creates_a_vbit_with_angle_and_tip_end_to_end` y `tools_add_creates_an_isolation_mill_as_a_real_type` (`crates/cam-cli/tests/cli.rs`). Verificación: `cargo fmt -p cam-store -p cam-cli` OK; `cargo clippy -p cam-store -p cam-cli --all-targets -- -D warnings` OK; `cargo test -p cam-store` OK (58 tests en targets: 2+12+25+8+13+0); `cargo test -p cam-cli` OK (47 tests en targets, incl. 19 en `tests/cli.rs`). No se modificó `odd/tasks/INDEX.md`. W2d implementado en las superficies declaradas; commit pendiente de corte/decisión de entrega.

- **L3 (cierre del workspace roto, review nativo y gate).** `ToolKind::IsolationMill` se agregó en `cam-store` sin actualizar `crates/cam-ui`, de modo que el workspace **no compilaba** (`E0004` non-exhaustive patterns) aunque los tests de `cam-store`/`cam-cli` pasaran. Se cubrió la variante en los tres `match` de la UI: `ToolFamily::of` y `ShapeField::of` (`tool_library/family.rs:26`, `:79`) la dibujan como fresa plana y `shape_geometry` (`tool_library/draft.rs:173`) le quita radio de punta, ángulo incluido y plano de punta; los tests que fijaban el catálogo de seis tipos se actualizaron a siete (`family.rs`, `form.rs`, y en `tree.rs` el chequeo de encabezados clickeables pasó a iterar los encabezados dibujados). Además, el review anterior dejó un ajuste en `cam-cli tools add`: el slug del tipo se agrega a la línea de éxito solo cuando se pasó `--kind`, para no cambiar el stdout base (`b8abc66`). Verificación: `cargo test -p cam-ui --test tool_library` 55 passed; `just check` del workspace completo en verde; commit `d1f79b8`.
