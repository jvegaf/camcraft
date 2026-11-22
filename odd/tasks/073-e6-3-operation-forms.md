# 073 — E6.3 Formularios de operación

Formularios de isolation, NCC, drilling, cutout y paint sobre el editor de estrategia existente.

## Surfaces

- `crates/cam-model/src/lib.rs`
- `crates/cam-model/tests/`
- `crates/cam-app/src/command.rs`
- `crates/cam-app/src/lib.rs`
- `crates/cam-app/tests/`
- `crates/cam-ui/src/shell/panels.rs`
- `crates/cam-ui/src/shell/mod.rs`
- `crates/cam-ui/src/chrome/ribbon.rs`
- `crates/cam-ui/src/actions/mod.rs`
- `crates/cam-ui/src/actions/handle.rs`
- `crates/cam-ui/src/lib.rs`
- `crates/cam-ui/src/shell/output.rs`
- `crates/cam-ui/tests/`
- `odd/roadmap/index.md`

## Specs

- **S1 (pedido original, verbatim).** «ahora completa la feature E6.3»
- **S2 (docs/ROADMAP_CAMCRAFT.md §9 E6.3, verbatim).** «| E6.3 | Formularios de isolation, NCC, drilling, cutout y paint sobre el editor de estrategia existente | **L** | Cada operación se configura sin CLI |»
- **S3 (odd/roadmap/index.md F6, verbatim).** «| E6.3 Operation forms | ⬜ | `grep -rn "IsolationParams\|NonCopperClearParams\|CutoutParams\|PaintParams" crates/cam-ui/src` → 0 | One form per PCB operation |»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Exponer UI para crear/seleccionar toolpaths y editar nombre/operación base | S1,S2 | inline | `939595f` |
| T2 | Formularios de parámetros para Isolation y NonCopperClear (NCC) | S1,S2,S3 | inline | `939595f` |
| T3 | Formularios de parámetros para Cutout y Paint | S1,S2,S3 | inline | `939595f` |
| T4 | Drilling: agregar operación/parámetros al modelo/app y formulario UI | S1,S2,S3 | inline | `939595f` |
| T5 | Verificación, tests y actualización de roadmap | S1,S2,S3 | inline | `939595f` |

## Log

- **L1 (pedido original, verbatim).** «ahora completa la feature E6.3»
- **L2 (decisión).** El usuario eligió: «Agregar Drilling al modelo y UI» para cubrir la brecha de `Operation::Drilling` nombrada por E6.3.
- **L3 (implementación).** Se agregó `DrillingParams` y `Operation::Drilling` en `cam-model` con validación; se actualizaron matches en model/cli/sim. En `cam-ui` se agregó editor de operación en el inspector de Toolpaths con nombre, tipo y formularios para Isolation, NCC, Cutout, Paint, Drilling y Panelize; se expusieron acciones `AddToolpath`/`DeleteToolpath` y manejo de Set/Add/Delete. Roadmap index marca E6.3 ✅.
- **L4 (verificación).** Pasaron `cargo check -p cam-model -p cam-app -p cam-ui -p cam-cli -p cam-sim`, `cargo test -p cam-ui --lib` (34) y `cargo test -p cam-ui --test toolpath_list` (6).
