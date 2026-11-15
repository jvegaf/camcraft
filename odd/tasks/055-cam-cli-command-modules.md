# 055 — Módulos por comando en `cam-cli` (preparación ola 4)

Preparación obligatoria antes de abrir dos worktrees para la ola 4: partir `crates/cam-cli/src/lib.rs` en módulos por comando, para que agregar `drill`, `cutout` u otros comandos no obligue a editar el mismo archivo monolítico.

## Surfaces

- `crates/cam-cli/src/`

## Specs

- **S1 (waves.md §6, verbatim).** «**Preparación obligatoria (1 agente, antes de abrir dos worktrees):** partir `crates/cam-cli/src/lib.rs` en módulos por comando (`commands/import.rs`, `commands/drill.rs`, …). Hoy es un archivo único de más de 500 líneas y **es el cuello de botella de todo paralelismo futuro**: dos agentes que agregan dos comandos chocan en el mismo archivo aunque no compartan una línea de lógica.»
- **S2 (criterio).** `crates/cam-cli/src/lib.rs` deja de contener las implementaciones de los comandos existentes; cada comando queda en su propio módulo bajo `crates/cam-cli/src/commands/` (al menos `validate`, `inspect`, `check`, `import`, `export`, `tools`, `postp`, y los ya separados por archivo se integran a la misma estructura cuando corresponda). `cam_cli::run` y las APIs públicas usadas por `main`, `script`, `pieces`, `printsheet` y los tests se preservan. Los comandos futuros `drill` y `cutout` pueden agregarse creando archivos en `commands/` y registrándolos sin modificar implementaciones ajenas.
- **S3 (invariante).** Ningún comportamiento cambia: `cargo test -p cam-cli` y los gates relevantes del workspace siguen en verde.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Crear `commands/` y extraer comandos de `lib.rs` preservando API y despacho | S1, S2 | inline | — |
| T2 | Actualizar módulos existentes (`script`, `pieces`, `printsheet`) y reexportes | S2 | inline | — |
| T3 | Verificar compilación y tests de `cam-cli` / workspace relevante | S3 | inline | — |

## Log

- **L1 (pedido, verbatim).** «lee @odd/roadmap/waves.md. Eres el agente encargado de hacer la tarea previa necesaria para arrancar los agentes de la ola 4»
- **L2 (alcance).** Waves §6 define la preparación como partir `crates/cam-cli/src/lib.rs` en módulos por comando para destrabar W4a/W4b. Se crea `crates/cam-cli/src/commands/` con un módulo por comando existente y `lib.rs` queda como fachada/dispatcher con tipos y helpers compartidos, sin alterar comportamiento.
- **L3 (implementación).** Se agregó `crates/cam-cli/src/commands/{mod,validate,inspect,check,import,export,tools,postp}.rs`. `lib.rs` conserva `Failure`, `HELP`, `run` (dispatcher), helpers compartidos (`only_path`, `open`, `classify_invalid`, `option_value`, `number`, `value_at`, `fresh_sheet_id`, `fresh_part_id`, `decimal`, `write_report`, `operation_label`) y reexporta `validate_report` como `commands::validate::report`. `pieces`, `printsheet` y `script` siguen como módulos existentes y consumen la misma API de crate; no se movieron para minimizar churn. W4 podrá agregar `commands/drill.rs` y `commands/cutout.rs` registrándolos en `commands/mod.rs` y en el `match` de `run`.
- **L4 (verificación).** `cargo fmt --all -- --check` OK. `cargo clippy -p cam-cli --all-targets -- -D warnings` OK. `cargo test -p cam-cli` OK (49 tests: 19 cli, 10 pieces, 8 printsheet, 8 script, 4 script_in_memory).
