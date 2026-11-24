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
| T1 | Crear `docs/MIGRATION_TAURI.md` con decisión, evidencia de spikes, arquitectura objetivo, IPC, fases, riesgos y docs a actualizar | S1,S2 | inline | — |
| T2 | Registrar en ODD (task doc + INDEX) y Engram | S2 | inline | — |
| T3 | Commit y push | S2 | inline | — |

## Log

- **L1 (pedido original, verbatim).** «despues de evaluarlo todo ya he tomado la decision. nos quedamos con tauri. ahora tenemos que crear el plan de migracion, porque Tauri es un framework completo, no solo una libreria de UI como egui. por lo que tendremos que reformar el @docs/SYSTEM_DESIGN_CAMCRAFT.md y demas documentacion, ademas de restructurar los directorios.... Para eso primero voy a crear yo el scafolding en una nueva rama y a partir de ahi ya puedes continuar tu»
- **L2 (confirmación, verbatim).** «documenta, sincronizalo con engram y odd comitea y pushea»
- **L3 (ejecución).** Se creó `docs/MIGRATION_TAURI.md`. Se registró tarea 079 en ODD y se guardó memoria en Engram.
