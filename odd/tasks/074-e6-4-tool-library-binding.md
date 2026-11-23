# 074 — E6.4 Binding de herramientas a la librería SQLite

Binding estable de herramientas PCB a la librería SQLite (no snapshot), de modo que editar una herramienta se refleje en las operaciones.

## Surfaces

- `crates/cam-model/src/`
- `crates/cam-model/tests/`
- `crates/cam-store/src/`
- `crates/cam-store/tests/`
- `crates/cam-ui/src/tool_library/`
- `crates/cam-ui/src/shell/mod.rs`
- `crates/cam-ui/src/shell/panels.rs`
- `crates/cam-ui/src/chrome/ribbon.rs`
- `crates/cam-ui/src/actions/`
- `crates/cam-ui/tests/`
- `crates/cam-app/src/`
- `crates/cam-app/tests/`
- `crates/cam-cli/src/`
- `crates/cam-cli/tests/`
- `odd/roadmap/index.md`
- `odd/tasks/074-e6-4-tool-library-binding.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
- **S2 (docs/ROADMAP_CAMCRAFT.md §9 E6.4, verbatim).** «| E6.4 | Panel de herramientas ligado a la librería SQLite (no a un snapshot) | **M** | Editar una herramienta se refleja en las operaciones |»
- **S3 (decisión de owner, 2026-05-23, verbatim).** «Identidad estable de librería»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Definir identidad estable de herramienta y contrato de binding en modelo/store/UI | S1,S2,S3 | inline | pending |
| T2 | Hacer que panel/operaciones resuelvan contra librería viva (no snapshot) y que edits se reflejen | S1,S2,S3 | inline | pending |
| T3 | Tests de regresión, validación y actualización de roadmap | S1,S2,S3 | inline | pending |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
- **L2 (decisión de owner, 2026-05-23).** Para E6.4 se eligió «Identidad estable de librería» (no la opción live sobre `tool_number`).
- **L3 (implementación).** Se agregó `Toolpath.tool_id` (Option<String>, serde default) como identidad estable de librería (geometry id) en `cam-model`; `tool_number` se mantiene como número de máquina/programa. `cam-store` expone `stored_tool_for_id`/`stored_tool_for_toolpath` para resolución id-first. `cam-app` y `cam-ui` actions/commands transportan `tool_id`. La ribbon y el modal de librería resuelven por identidad estable; al abrir la librería se recarga desde SQLite y "Use this tool" bindea `tool_id`+`tool_number`. Roadmap index marca E6.4 ✅.
