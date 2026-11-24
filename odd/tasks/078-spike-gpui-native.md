# 078 — Spike GPUI nativo

Spike de viabilidad: GPUI nativo en Rust para reexpresar el DS de camcraft e integrar un viewport 3D conectado a `cam-view`, para comparar contra Tauri con métricas comunes.

## Surfaces

- `spikes/gpui/`

## Specs

- **S1 (pedido original, verbatim).** «hay que sacar un plan para elaborar los dos spikes por dos agentes en paralelo, cada uno en su worktree, y luego sacaremos metricas y llegaremos a conclusiones»
- **S2 (confirmación, verbatim).** «si. preparalo todo»
- **S3 (slice vertical común, verbatim).** «Ventana desktop con theme dark/light tokens del DS. Start screen mínima (marca, New/Open, lista reciente mock o real). Un viewport con una escena fija exportada por `cam-view` (mismo fixture/escena). Cámara básica: orbit/pan/zoom. Un overlay UI sobre/junto al viewport (status o panel) para probar composición.»
- **S4 (scorecard común, verbatim).** «Fidelidad DS | Comparar start screen/panel contra `design/` (checklist visual: tipografía, spacing, radios, bordes, colores, estados) | Tauri debería ser alto; GPUI “aceptable sin CSS”. Viewport FPS | Escena fija, 60s, orbit continuo; reportar avg/min/1% low | >= 60 avg, sin stutters visibles. Input latency | Sensación/medición de orbit/pan/zoom y click | Sin lag perceptible. Integración `cam-view` | Esfuerzo y forma: directo Rust, serialización, o adaptación | Menos glue = mejor. Complejidad de build | Pasos desde cero, deps extra, CI Linux | Reproducible en <15 min. Tamaño/packaging | Binario/appimage/deb aproximado | Informativo. Riesgo técnico | Bloqueos encontrados, hacks, APIs inestables | Clasificar alto/medio/bajo. Costo de migración estimado | Por crate: reusar/rewrite/bridge | Semanas/persona grueso.»

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Bootstrap del spike en `spikes/gpui/`: app GPUI que levanta en Linux, README con deps y comandos reproducibles | S1,S2 | worktree | — |
| T2 | Chrome DS: start screen mínima con tokens mapeados desde `design/system.md` (colores, spacing, radios, tipografía); dark/light toggle | S2,S3 | worktree | — |
| T3 | Viewport 3D: escena fija de `cam-view` en una vista GPUI con orbit/pan/zoom; documentar estrategia de render/integración | S2,S3 | worktree | — |
| T4 | Composición: overlay UI junto/sobre viewport (status o panel) y wiring de estado básico (cámara/selección) | S2,S3 | worktree | — |
| T5 | Medición y reporte: completar `spikes/gpui/REPORT.md` con scorecard S4, capturas/gif, bloqueos, comandos de reproducción | S1,S4 | worktree | — |

## Log

- **L1 (pedido original, verbatim).** «hay que sacar un plan para elaborar los dos spikes por dos agentes en paralelo, cada uno en su worktree, y luego sacaremos metricas y llegaremos a conclusiones»
- **L2 (confirmación, verbatim).** «si. preparalo todo»
- **L3 (preparación).** Documento de tarea creado. Spike hermano: 077 (Tauri). Regla de no-solape: código nuevo solo en `spikes/gpui/`; no tocar `crates/cam-ui`; no tocar `crates/cam-view` salvo cambio mínimo coordinado y declarado antes; `design/`, `fixtures/ui/`, `spikes/shared/` solo lectura.
