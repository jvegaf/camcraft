# 071 — E6.1 Identidad de capas de placa

Capas con identidad de placa en la UI: color por rol, visibilidad y modo cobre, para que las capas de fabricación se distingan visualmente y se alternen.

## Surfaces

- `crates/cam-model/src/lib.rs`
- `crates/cam-model/tests/document.rs`
- `crates/cam-view/src/lib.rs`
- `crates/cam-view/tests/scene.rs`
- `crates/cam-ui/src/shell/`
- `crates/cam-ui/src/actions/`
- `crates/cam-ui/src/theme.rs`
- `crates/cam-ui/src/lib.rs`
- `crates/cam-ui/tests/shell/panels.rs`
- `odd/roadmap/index.md`
- `docs/ROADMAP_CAMCRAFT.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa E6.1»
- **S2 (ROADMAP_CAMCRAFT.md §9 E6.1, verbatim).** «| E6.1 | Capas con identidad de placa: color por rol, visibilidad, modo cobre | **M** | Las 6 capas se distinguen visualmente y se alternan |»
- **S3 (odd/roadmap/index.md F6, verbatim).** «| E6.1 Board layer identity | ⬜ | `Layer.role` exists in the model (`cam-model/src/lib.rs:237`), the UI still draws generic layers (`cam-view/tests/scene.rs:17` uses `LayerRole::Geometry`) | Colour by role, copper mode |»

## Tasks

| ID  | Tarea                                                                                      | Specs    | Ruta   | Commit    |
| --- | ------------------------------------------------------------------------------------------ | -------- | ------ | --------- |
| T1  | Exponer rol en la escena y mapear colores canónicos por rol de capa PCB                    | S1,S2,S3 | inline | `5a95d49` |
| T2  | Renderizar capas con color por rol en el canvas 2D y distinguir las 6 capas de fabricación | S1,S2    | inline | `5a95d49` |
| T3  | Agregar modo cobre (toggle) y relleno visual de capas de cobre cuando está activo          | S1,S2    | inline | `5a95d49` |
| T4  | Pulir el dock de capas: identidad visual por rol, visibilidad clara y estado de modo cobre | S1,S2    | inline | `5a95d49` |
| T5  | Tests, verificación focalizada y actualización de roadmap/docs                             | S1,S2,S3 | inline | `5a95d49` |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa E6.1»
- **L2 (implementación).** Se agregó identidad de rol PCB en `cam-model` (`LayerRole::{color_rgb,label,is_copper}`), se propagó `role` en `SceneItem`/`build_scene` de `cam-view` (incluyendo drill/slot como arcos/líneas), se pintaron capas por color de rol en `cam-ui` y se agregó toggle `Copper mode` con relleno de contornos de cobre. El dock de capas muestra swatch de rol, etiqueta, lado y visibilidad alternable.
- **L3 (verificación).** Pasaron `cargo test -p cam-view` (23 tests), `cargo test -p cam-model` (72 en document.rs + 4 en frame.rs), `cargo test -p cam-ui --lib` (34), `--test shell` (138), `--test input` (23), `--test palette` (4), `--test tool_library` (55), `--test tool_card` (5), `--test toolpath_list` (6), `--test script_panel` (8). `ui_snapshot` y `ui_review` ignorados por diseño. `cargo clippy -p cam-model -p cam-view -p cam-ui --all-targets` reportó solo warnings/deprecaciones preexistentes fuera de los cambios E6.1. Se actualizó `odd/roadmap/index.md` para marcar E6.1 ✅.
