# 072 — E6.2 Árbol de proyecto

Árbol de proyecto en la UI: placa → capas → operaciones. El árbol refleja el documento y navega a la operación.

## Surfaces

- `crates/cam-ui/src/chrome/palette.rs`
- `crates/cam-ui/src/shell/mod.rs`
- `crates/cam-ui/src/shell/panels.rs`
- `crates/cam-ui/tests/shell/panels.rs`
- `crates/cam-ui/src/drawn.rs`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «ahora completa la feature E6.2»
- **S2 (docs/ROADMAP_CAMCRAFT.md §9 E6.2, verbatim).** «| E6.2 | Árbol de proyecto: placa → capas → operaciones | **M** | El árbol refleja el documento y navega a la operación |»
- **S3 (odd/roadmap/index.md F6, verbatim).** «| E6.2 Project tree | ⬜ | No board→layers→operations tree in `cam-ui` | The tree and its navigation |»

## Tasks

| ID  | Tarea                                                                                     | Specs    | Ruta   | Commit    |
| --- | ----------------------------------------------------------------------------------------- | -------- | ------ | --------- |
| T1  | Modelar el árbol de proyecto en el browser: nodo placa, capas hijas, operaciones hijas    | S1,S2,S3 | inline | `17dc1f8` |
| T2  | Navegación desde el árbol: seleccionar operación y reflejar selección/capas del documento | S1,S2    | inline | `17dc1f8` |
| T3  | Tests y actualización de roadmap/index                                                    | S1,S2,S3 | inline | `17dc1f8` |

## Log

- **L1 (pedido original, verbatim).** «ahora completa la feature E6.2»
- **L2 (implementación).** Se reemplazó el browser plano por un árbol de proyecto en `cam-ui/src/chrome/palette.rs`: nodo raíz `Placa` con dimensiones, hijos `Capas` (swatch de rol, etiqueta, id, toggle de visibilidad) y `Operaciones` (lista seleccionable que navega con `set_toolpath_selection`). Se expuso estado del browser en `DrawnState` para verificación.
- **L3 (verificación).** Pasaron `cargo test -p cam-ui --lib` (34), `--test shell` (140), `--test input` (23), `--test palette` (4). Se agregó test `project_tree_reflects_document_and_navigates_to_operation`. `odd/roadmap/index.md` marca E6.2 ✅.
