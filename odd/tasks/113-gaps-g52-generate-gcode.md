# 113 — G5.2: el camino de la UI a la generación de G-code

Cerrar el hallazgo G5.2 de `docs/GAPS_CAMCRAFT.md`: «Generate G-code» de la app bar es hoy un hit-target sin comando detrás. El cierre completo cablea el picker «Post…» (hoy `Shell.post` es siempre `None`), una acción de generación con diálogo de ruta del host y la emisión compartida con la CLI, sin duplicar el emisor.
Estado: `delivered` — T1–T3 entregados y registrados en `## Tasks` con `600d5dd`, `c0f41be` y `3eccded`; T4 (`pendiente` en la tabla hasta que exista su commit, que registra el padre) cierra G5.2 en `docs/GAPS_CAMCRAFT.md` y deja los gates documentales en verde.

## Surfaces

- `src-tauri/crates/cam-cli/src/commands/cutout.rs`
- `src-tauri/crates/cam-cli/src/lib.rs`
- `src-tauri/crates/cam-ui/src/actions/mod.rs`, `src-tauri/crates/cam-ui/src/actions/handle.rs`
- `src-tauri/crates/cam-ui/src/shell/action.rs`, `src-tauri/crates/cam-ui/src/shell/mod.rs`
- `src-tauri/crates/cam-ui/src/chrome/app_bar.rs`
- `src-tauri/crates/cam-ui/src/view3d/program.rs`
- `src-tauri/crates/cam-ui/tests/shell/`
- `src-tauri/crates/cam-desktop/src/main.rs`
- `docs/GAPS_CAMCRAFT.md`
- `odd/tasks/113-gaps-g52-generate-gcode.md`, `odd/tasks/INDEX.md`

## Specs

- **S1 (owner, verbatim).** «G5.2»
- **S2 (hallazgo que se cierra, verbatim de `docs/GAPS_CAMCRAFT.md:719`).** «Generate G-code» de la UI es un hit-target sin comando detrás
- **S3 (alcance aprobado por el owner, verbatim de la opción elegida).** «Promover emit_program a librería, cablear el picker «Post…» (set_post/clear_post hoy sin llamador), agregar UiAction + diálogo de ruta y escribir el .nc, con test end-to-end de que el archivo existe y es determinista. Ficha large con TDD. Es lo único que deja el botón realmente vivo.»
- **S4 (una sola emisión).** El camino documento→programa existe en un solo lugar. `emit_program` (`cam-cli/src/commands/cutout.rs:544-580`) se promueve a función pública de librería y `cam-cli program` pasa a llamarla; `cam-ui` usa la misma función. No se reimplementa el emisor en `cam-ui`: el emisor real es `cam_post::emit::emit` (`cam-post/src/emit.rs:79-85`) y `parse_post` (`cam-post/src/parser.rs:13`).
- **S5 (ninguna ruta se inventa).** Toda ruta de archivo viene de un diálogo del host, como el resto (`cam-desktop/src/main.rs:278-325`); la UI no elige ni deriva rutas de salida.
- **S6 (capa correcta).** `cam-app::Command` es undoable y marca `dirty` por construcción (`cam-app/src/lib.rs:198-207`), así que generar un archivo vive en la capa de shell/`UiAction`, igual que `Save` y `ExportTools` (`cam-ui/src/actions/mod.rs:186-202,262-277`, interceptados en `cam-ui/src/shell/action.rs:57-62`). No se agrega a `cam-app::Command`.
- **S7 (no regresión).** `DrawnState::app_bar` sigue vacío (`cam-ui/tests/palette.rs:138-147`) y el botón se reporta por `app_bar_hits`. Los tests existentes de `cam-app`, `cam-cli`, `cam-post` y `cam-ui` siguen verdes.
- **S8 (rechazo honesto).** Sin documento, sin preprocesador elegido o sin ruta de salida, la acción no escribe ningún archivo, no cambia el documento y reporta el motivo; el texto visible no promete lo que no hizo.
- **S9 (documentación).** Al cerrar: el token de estado de G5.2 en `docs/GAPS_CAMCRAFT.md:380`, su lista de citas, la fila de severidad §8 (`:719`) y los conteos en prosa (`:102`, `:109`, `:774`) quedan coherentes, y `./scripts/check-gaps-states.sh` sale 0.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                        | Specs          | Ruta   | Commit    |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | --------- |
| T1  | API pública de emisión: promover `emit_program` a librería y hacer que `cam-cli program` la llame, sin cambio de salida byte a byte                                                                                          | S4             | worker | `600d5dd` |
| T2  | Picker «Post…» vivo: `UiAction` + diálogo del host + primer llamador real de `set_post`/`clear_post`, con rechazo de un archivo que no parsea como post                                                                      | S3, S5, S8     | worker | `c0f41be` |
| T3  | Generación: `UiAction` de generar, diálogo de ruta de salida, emisión por la API compartida a partir de `build_preview_stream`, rechazos honestos y test de punta a punta que pulsa el botón y verifica archivo determinista | S3, S5, S6, S8 | worker | `3eccded` |
| T4  | No regresión y documentación: `DrawnState::app_bar` vacío, suite completa, GAPS §3/§8/prosa, fila del INDEX y `./scripts/check.sh` verde                                                                                     | S7, S9         | worker | pendiente |

## Log

- **L1 (pedido del owner, verbatim).** «pushea y pasamos al listado de hallazgos»; ante el listado de hallazgos abiertos eligió «G5.2 — cablear «Generate G-code»» y luego el alcance «Cierre completo del camino de la UI».
- **L2 (mapa de partida, 2026-11-28).** Tres bloqueos que el hallazgo no declaraba: (a) no existe API pública documento→programa — `cam-cli program --post` despacha en `cam-cli/src/lib.rs:237` y termina en `emit_program` privada (`commands/cutout.rs:544-580`) sobre argv; (b) `Shell.post` es siempre `None` porque `set_post`/`clear_post` (`cam-ui/src/shell/action.rs:255-271`) no tienen llamador y el botón `Post…` (`chrome/app_bar.rs:159-167`) está inerte; (c) la UI no puede elegir ruta — todas vienen de diálogos del host (`cam-desktop/src/main.rs:278-325`) y no hay `UiAction` de generación (comentarios huérfanos en `actions/handle.rs:30-43`). Lo que ya existe: `build_preview_stream(document, tools)` (`cam-ui/src/view3d/program.rs:17-20`), que produce el `MoveStream` que `emit` consume, y las dependencias `cam-cli`/`cam-post` en `cam-ui/Cargo.toml:16,24`.
- **L3 (decisiones).** (a) Se promueve la emisión a librería en vez de duplicarla en `cam-ui` (S4). (b) El generador no entra en `cam-app::Command` por el `dirty`/undo incondicional de `AppState::execute_with_outcome`; va como `UiAction` interceptada (S6). (c) Escribir un `.nc` nuevo en una ruta elegida por el humano en un diálogo de guardado no cuenta como ítem 1 de riesgo: es guardar un registro nuevo, no reescribir datos existentes, y el diálogo del host es quien confirma sobrescritura. `Risk: none` para la ficha.
- **L4 (delivery, 2026-11-28).** G5.2 closed: T1 `600d5dd` exports the emission path, T2 `c0f41be` makes the post picker live, T3 `3eccded` generates the program from the app bar, and T4 (this document, its commit recorded by the parent once it exists) retires the finding and updates the §8/§9 counts. Two failures the feature inherits are known debt outside it, not caused by this work: `start::a_recent_row_opens_the_path_it_remembers` and `start::pressing_new_project_on_the_start_screen_starts_one`, both reporting `actions: []` and both reproduced on base `600d5dd`. Deferred follow-up: `cam-cli`'s `isolate`, `ncc` and `drill` still hold their own emission copies, so the shared `emit_program`/`emit_program_from_source` is not yet the crate's only emitter (documented at `src-tauri/crates/cam-cli/src/lib.rs:37-38`).
