# 076 — E6.6 Preview, simulación e integración PCB

Previsualización de toolpaths y simulación integradas para el flujo PCB, con gouge/uncut en playback y cierre de la puerta F6.

## Surfaces

- `crates/cam-ui/src/view3d/`
- `crates/cam-ui/src/shell/mod.rs`
- `crates/cam-ui/src/shell/panels.rs`
- `crates/cam-ui/src/actions/`
- `crates/cam-ui/tests/`
- `crates/cam-app/src/`
- `crates/cam-app/tests/`
- `crates/cam-ops/src/`
- `crates/cam-ops/tests/`
- `crates/cam-sim/src/`
- `crates/cam-sim/tests/`
- `crates/cam-post/src/`
- `crates/cam-export/src/`
- `odd/roadmap/index.md`
- `odd/tasks/076-e6-6-pcb-preview-sim.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
- **S2 (docs/ROADMAP_CAMCRAFT.md §9 E6.6, verbatim).** «| E6.6 | Previsualización de toolpaths y simulación integradas | **M** | El playback muestra gouge/uncut del programa |»
- **S3 (docs/ROADMAP_CAMCRAFT.md §9 puerta F6, verbatim).** «**Puerta de F6 = Hito 4 del plan original.** Flujo completo en la interfaz: importar, configurar, generar, simular, exportar.»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Pipeline UI/app para generar programa PCB desde operaciones y correr simulación | S1,S2,S3 | inline |  |
| T2 | Integrar preview 3D/playback con reporte de simulación (gouge/uncut visibles) | S1,S2 | inline |  |
| T3 | Cerrar flujo importar/configurar/generar/simular/exportar en UI y actualizar roadmap | S1,S2,S3 | inline |  |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/README.md @docs/ROADMAP_CAMCRAFT.md @odd/roadmap/index.md y completa F6»
