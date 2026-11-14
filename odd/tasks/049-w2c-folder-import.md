# w2c-folder-import

Importar una carpeta de fabricación como placa (E1.3 / W2c): detectar y agrupar los archivos por rol y contenido para abrir la carpeta como una placa con las capas correctas.

## Surfaces

- `crates/cam-import/`

## Specs

- **S1 (E1.3, verbatim ROADMAP).** «Importador de **carpeta de fabricación** (detecta y agrupa top/bottom/mask/silk/outline/drill por nombre y contenido)»
- **S2 (criterio de salida E1.3, verbatim ROADMAP).** «Una carpeta de KiCad se abre como placa completa con roles correctos»
- **S3 (ola 2 / W2c, verbatim waves).** «W2c | **E1.3 importador de carpeta** | Detectar y agrupar `top/bottom/mask/silk/outline/drill` por nombre y contenido; abrir la carpeta como placa | `crates/cam-import/` | W2a y W2b para el contenido, no para el agrupamiento»
- **S4 (roles de capa, verbatim E1.1 ROADMAP).** `LayerRole` (copper_top, copper_bottom, mask, silk, outline, drill)

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Detección y agrupación de archivos de una carpeta por nombre y por contenido en roles `top/bottom/mask/silk/outline/drill` con lado cuando aplica, ignorando artefactos ajenos y reportando lo no reconocido | S1, S3, S4 | inline | `19ee800` |
| T2 | Importar el contenido de cada archivo agrupado (Gerber/Excellon) y construir un `Document` de placa con `LayerRole`/`Side` correctos, dimensiones derivadas y geometría por capa | S1, S2, S4 | inline | `19ee800` |
| T3 | Pruebas: carpeta de estilo KiCad (nombres simplificados del corpus y/o convenciones KiCad) se abre como placa completa con roles correctos; agrupación por nombre y por contenido | S1, S2, S3 | inline | `19ee800` |

## Log

- **L1 (pedido, verbatim).** «lee @odd/roadmap/waves.md, eres el encargado de completar w2c»
- **L2 (contexto).** Ola 2 / W2c (E1.3) en `odd/roadmap/waves.md`: superficies `crates/cam-import/`; depende de W2a/W2b para el contenido, no para el agrupamiento. Trabajo en worktree `waves-w2c` (`/home/th3g3ntl3man/.herdr/worktrees/camcraft/waves-w2c`, rama `waves/w2c`), base incluye w2a/w2b (`7a7c725`). `crates/cam-import` hoy importa DXF/SVG; `cam-gerber::parse`/`Gerber::image` y `cam_excellon::parse` ya disponibles. Corpus de referencia en `fixtures/corpus/kicad-2l-simple/` con `top.gbr,bottom.gbr,mask-top.gbr,mask-bottom.gbr,silk-top.gbr,silk-bottom.gbr,outline.gbr,drill.drl`.
- **L3 (implementación T1–T3).** Se agregó `crates/cam-import/src/folder.rs` y exportaciones en `lib.rs`, con dependencias `cam-gerber` y `cam-excellon` en `Cargo.toml`. API: `detect_role` (nombre y/o contenido, con `Detection`), `import_folder` que escanea la carpeta, agrupa por rol/lado (`top/bottom/mask/silk/outline/drill` → `LayerRole`/`Side`), parsea Gerber (`parse`+`image`) y Excellon (`parse`) y arma un `Document` de placa con dimensiones derivadas del outline (o de los bounds totales), geometría trasladada con margen y capas con roles/lados correctos; archivos no reconocidos se reportan aparte y fallos de parseo/evaluación quedan como avisos. T1: detección por nombres simplificados del corpus y por convenciones KiCad (`F_Cu`/`.gtl`, `Edge_Cuts`/`.gm1`, etc.) y por contenido (`TF.FileFunction`/`FileFunction` para cobre/máscara/seda/perfil, firma Excellon M48). T2: conversión de `Region::boundaries` a entidades de contorno y de taladros/slots Excellon a contornos (círculos/ranuras), fusión por `(role,side)`. T3: `crates/cam-import/tests/folder.rs` cubre detección por nombre, por contenido y no reconocidos; una carpeta KiCad sintética (nombres simplificados, Gerber/Excellon parseables) se abre como placa completa con los 8 roles/lados correctos y dimensiones; agrupación por contenido con nombres genéricos produce la placa completa y reporta el archivo ajeno; y se verifica sobre `fixtures/corpus/kicad-2l-simple/` que los 8 artefactos de fabricación se agrupan con rol/lado correctos. Nota: en el corpus real, varios Gerber/Excellon no producen geometría con el estado actual de los parsers (p.ej. rotación `RoundRect`, `arrangement` fino, slots G85), lo que es limitación de contenido fuera de la superficie de W2c (S3: «depende de W2a y W2b para el contenido, no para el agrupamiento»); la importación completa se demuestra con entradas parseables.
- **L4 (verificación).** `cargo test -p cam-import --test folder` 8/8 ok; `cargo test -p cam-import` 72/72 ok (csv, folder, import, svg). `cargo fmt -p cam-import` y `cargo clippy -p cam-import --all-targets -- -D warnings` sin avisos. `cargo check -p cam-import` ok. Riesgo: `none` (código nuevo en `crates/cam-import`, sin cambios en contratos existentes).

- **L5 (endurecimiento posterior a la entrega, review nativo).** El review del diff acumulado (`a896872`, `c5d71c5`, `b8abc66`) marcó hallazgos sobre este import y se corrigieron en el árbol: una capa reconocida que **falla al parsear** ya no figura en `FolderImport::imported` (solo queda como aviso con su rol, `41540cc`); una capa que **parsea sin geometría** conserva el comportamiento base, por decisión del review aprobado `review-bf30a519b9b398b`; un drill que nombra un tool que el archivo no define se **rechaza entero** (`a4f967a`) en lugar de inventar 1.0 mm o saltear la feature, porque una capa de taladrado incompleta llegaría a la máquina con agujeros faltantes; un slot de longitud cero se importa como el agujero que es y se reporta. Tests: `a_recognized_file_that_fails_to_parse_is_warned_but_not_imported`, `a_drill_naming_an_undefined_tool_is_skipped_without_losing_the_folder`, `a_zero_length_slot_is_imported_as_a_hole_and_reported` (`crates/cam-import/tests/folder.rs`, 11/11 ok). Riesgo: `none` fuera de `crates/cam-import`.
