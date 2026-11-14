# 048 — W2b Excellon (E1.6)

Completar la ola 2 ítem W2b: que el lector Excellon entienda el dialecto que traen los archivos reales del corpus (ceros omitidos, slots, tabla de herramientas) y que los 6 taladros del corpus lo prueben.

## Surfaces

- `crates/cam-excellon/`

## Specs

- **S1 (E1.6, verbatim ROADMAP).** «Parser Excellon: tabla de herramientas, coordenadas, unidades, ceros omitidos, slots». Criterio de salida: «Los 6 archivos de taladro del corpus coinciden en herramienta, diámetro y cantidad de hits».
- **S2 (ola 2, verbatim waves).** W2b entrega «Ceros omitidos, dialecto completo, slots y tabla de herramientas probados contra archivos reales». Superficies reclamadas: `crates/cam-excellon/`. Dependencias: —.
- **S3 (prueba sobre el corpus).** Los 6 archivos `fixtures/corpus/*/drill.drl` (`kicad-2l-simple`, `kicad-4l-pours`, `kicad-large-pours`, `substitute-altium`, `substitute-eagle`, `substitute-easyeda`) se leen sin error con la cantidad de herramientas, el diámetro de cada herramienta y la cantidad de features (taladros y ranuras) que el archivo declara. Las ranuras escritas como `X…Y…G85X…Y…` cuentan como una feature de tipo ranura.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ Soportar ranuras `G85` en línea (`X…Y…G85X…Y…`) y casos de ceros omitidos / dialecto necesarios para los archivos reales | S1, S2, S3 | inline | — |
| T2 | ✅ Pruebas contra los 6 taladros del corpus: herramientas, diámetros y cantidad de hits | S1, S2, S3 | inline | — |

## Log

- **L1 (pedido, verbatim).** «lee @odd/roadmap/waves.md, eres el encargado de completar w2b»
- **L2 (contexto).** Ola 2 / W2b (`odd/roadmap/waves.md`): E1.6 Excellon, superficies `crates/cam-excellon/`, sin dependencias declaradas. Trabajo en worktree `waves-w2b` (`/home/th3g3ntl3man/.herdr/worktrees/camcraft/waves-w2b`), rama `waves/w2b`, base `7a7c725`. El arnés de paridad (045) dejaba 4/6 drills rechazados por ranuras `X…Y…G85X…Y…` (`kicad-2l-simple`, `kicad-4l-pours`, `substitute-altium`, `substitute-eagle`) y 2/6 parseados (`kicad-large-pours`: 13 herramientas, 251 features; `substitute-easyeda`: 7, 165). Conteos esperados por inspección de los archivos reales: `kicad-2l-simple` 3/88, `kicad-4l-pours` 6/692, `kicad-large-pours` 13/251, `substitute-altium` 10/526, `substitute-eagle` 12/445, `substitute-easyeda` 7/165 (features = taladros + ranuras).

- **L3 (implementación).** Se agregó soporte para ranuras en línea `X…Y…G85X…Y…` en `crates/cam-excellon/src/lib.rs`: el tramo previo a `G85` fija el punto inicial y el posterior el final, y se emite una `Feature::Slot` (una feature/hit). Las coordenadas siguen las mismas reglas de unidad, formato y modo absoluto/relativo que el resto del lector. Se añadió la prueba de integración `the_six_corpus_drill_files_match_their_tools_and_hits` en `crates/cam-excellon/tests/excellon.rs`, que lee los 6 `fixtures/corpus/*/drill.drl` y verifica cantidad de herramientas, número y diámetro de cada herramienta, y cantidad de features (taladros + ranuras).
- **L4 (verificación).** `cargo test -p cam-excellon` → 15 passed, 0 failed (14 existentes + 1 corpus). Los 6 drills del corpus leen con los conteos esperados: `kicad-2l-simple` 3 herramientas / 88 features, `kicad-4l-pours` 6/692, `kicad-large-pours` 13/251, `substitute-altium` 10/526, `substitute-eagle` 12/445, `substitute-easyeda` 7/165. `cargo fmt --all` ok; `cargo clippy -p cam-excellon --all-targets -- -D warnings` ok.
