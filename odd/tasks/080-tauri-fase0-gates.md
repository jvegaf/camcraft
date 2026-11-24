# 080 — Fase 0: gates, CI y justfile alineados al stack Tauri

Cerrar los ítems pendientes de Fase 0 del plan de migración Tauri: alinear gates/CI/justfile al stack nuevo y dejar definida la integración de dominio desde `src-tauri/crates/`.

## Surfaces

- `docs/MIGRATION_TAURI.md`
- `docs/DOMAIN_INTEGRATION_TAURI.md`
- `scripts/check.sh`
- `justfile`
- `.github/workflows/ci.yml`
- `AGENTS.md`
- `odd/tasks/080-tauri-fase0-gates.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, dale»

## Tasks

| ID  | Tarea                                                                       | Specs | Ruta   | Commit |
| --- | --------------------------------------------------------------------------- | ----- | ------ | ------ |
| T1  | Alinear gates/CI/justfile al stack Tauri (workspace Rust, scripts, recetas) | S1    | inline |        |
| T2  | Actualizar docs de migración/integración para reflejar Fase 0 cerrada       | S1    | inline |        |
| T3  | Verificar gates, RDD, commit y push                                         | S1    | inline |        |

## Log

- **L1 (pedido original, verbatim).** «perfecto, dale»
- **L2 (contexto).** Sigue a la recomendación de cerrar Fase 0 antes de Fase 1 (host + shell mínimo). Los crates de dominio ya están en `src-tauri/crates/` y el workspace raíz los incluye.
- **L3 (ejecución, 2026-11-24).** Se alineó `scripts/check.sh` a `cargo check --workspace` + `cargo test --manifest-path src-tauri/Cargo.toml`; `justfile` ganó `check-rust` workspace y `test-rust-workspace`; CI cachea `.` como workspace Cargo; `AGENTS.md` actualizado. `docs/MIGRATION_TAURI.md` y `docs/DOMAIN_INTEGRATION_TAURI.md` marcan Fase 0 base cerrada. Biome: se actualizó schema a 2.6.8 y se excluyeron `design`, `src/app.css`, `static` y baselines de benches; `pnpm format` aplicó fixes mecánicos en 145 archivos preexistentes. Gates verificados: biome OK, svelte-check OK, docs OK, cargo check --workspace OK, cargo test src-tauri OK, vitest OK. `check-records.sh` falla por 2 commits preexistentes rotos en INDEX (035 `9aed8275ac99`, 060 `027d7ed00b67`), no causados por este cambio.
