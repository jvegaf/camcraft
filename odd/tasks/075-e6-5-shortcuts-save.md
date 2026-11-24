# 075 — E6.5 Atajos de teclado y Ctrl+S

Atajos de teclado del flujo PCB, Ctrl+S sin diálogo cuando hay ruta, y mapa de atajos documentado en la ayuda.

## Surfaces

- `crates/cam-ui/src/shell/input.rs`
- `crates/cam-ui/src/shell/mod.rs`
- `crates/cam-ui/src/chrome/overlays.rs`
- `crates/cam-ui/src/chrome/help.rs`
- `crates/cam-ui/src/chrome/appbar.rs`
- `crates/cam-ui/src/actions/`
- `crates/cam-ui/tests/`
- `odd/roadmap/index.md`
- `odd/tasks/075-e6-5-shortcuts-save.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
- **S2 (docs/ROADMAP_CAMCRAFT.md §9 E6.5, verbatim).** «| E6.5 | Atajos de teclado y Ctrl+S (verificar G5.3) | **S** | Guardar sin diálogo; mapa de atajos documentado en la ayuda |»

## Tasks

| ID  | Tarea                                                                                            | Specs | Ruta   | Commit  |
| --- | ------------------------------------------------------------------------------------------------ | ----- | ------ | ------- |
| T1  | Implementar Ctrl+S (guardar sin diálogo cuando hay ruta; comportamiento definido si no hay ruta) | S1,S2 | inline | pending |
| T2  | Documentar mapa de atajos en ayuda/overlay, incluyendo flujo PCB relevante                       | S1,S2 | inline | pending |
| T3  | Tests y actualización de roadmap                                                                 | S1,S2 | inline | pending |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
- **L2 (implementación).** Se agregó Ctrl+S en `shell/input.rs`: guarda con `UiAction::Save` si hay ruta, o `RequestSaveAs` si no. Se documentó en la ventana de atajos (`chrome/overlays.rs`) y en la ayuda de Save (`src/help.rs`).
