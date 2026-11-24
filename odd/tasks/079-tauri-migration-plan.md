# 079 — Plan de migración a Tauri 2

Documentar la decisión de migrar el shell desktop de camcraft de egui a Tauri 2, con plan incremental, boundary de arquitectura y tareas de documentación.

## Surfaces

- `docs/MIGRATION_TAURI.md`
- `odd/tasks/079-tauri-migration-plan.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «despues de evaluarlo todo ya he tomado la decision. nos quedamos con tauri. ahora tenemos que crear el plan de migracion, porque Tauri es un framework completo, no solo una libreria de UI como egui. por lo que tendremos que reformar el @docs/SYSTEM_DESIGN_CAMCRAFT.md y demas documentacion, ademas de restructurar los directorios.... Para eso primero voy a crear yo el scafolding en una nueva rama y a partir de ahi ya puedes continuar tu»
- **S2 (confirmación, verbatim).** «documenta, sincronizalo con engram y odd comitea y pushea»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Crear `docs/MIGRATION_TAURI.md` con decisión, evidencia de spikes, arquitectura objetivo, IPC, fases, riesgos y docs a actualizar | S1,S2 | inline | `b707a04` |
| T2 | Registrar en ODD (task doc + INDEX) y Engram | S2 | inline | `b707a04` |
| T3 | Commit y push | S2 | inline | `b707a04` |

## Log

- **L1 (pedido original, verbatim).** «despues de evaluarlo todo ya he tomado la decision. nos quedamos con tauri. ahora tenemos que crear el plan de migracion, porque Tauri es un framework completo, no solo una libreria de UI como egui. por lo que tendremos que reformar el @docs/SYSTEM_DESIGN_CAMCRAFT.md y demas documentacion, ademas de restructurar los directorios.... Para eso primero voy a crear yo el scafolding en una nueva rama y a partir de ahi ya puedes continuar tu»
- **L2 (confirmación, verbatim).** «documenta, sincronizalo con engram y odd comitea y pushea»
- **L3 (ejecución).** Se creó `docs/MIGRATION_TAURI.md`. Se registró tarea 079 en ODD y se guardó memoria en Engram.
- **L4 (progreso feat/tauri, 2026-05-24).** En `feat/tauri` se avanzó la migración: scaffolding Tauri 2+Svelte 5+Panda+Vitest/Biome, workspace raíz Cargo integrando `prev/crates/*`, comandos IPC documento/recents con DTOs y errores estructurados, rutas `/` y `/workspace`, y migración DS Panda (PostCSS/layers, `src/lib/ds/camcraft.preset.ts` con tokens semánticos/textStyles/recipes, bridge legacy en `src/app.css`, componentes workbench/start-screen y rutas migradas a `css()`/recipes). Commits: `71734d6..4341adf` (ver git log). RDD completo de rama sigue bloqueado por `lens_context_budget_exceeded`; trabajo validado por slices chicos con `biome check`, `pnpm run check`, `pnpm run build`, `cargo check/test -p camcraft` según slice.
- **L5 (decisión próxima sesión, 2026-05-24).** El usuario pidió indicar en el roadmap de migración que en la próxima sesión hay que mover los crates de dominio desde `prev/crates/` a `crates/` (Opción A, `docs/DOMAIN_INTEGRATION_TAURI.md`), actualizando workspace, paths de `src-tauri`, scripts/CI/tooling y referencias. Se actualizó `docs/MIGRATION_TAURI.md` Fase 0.
