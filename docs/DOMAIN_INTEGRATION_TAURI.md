# Integración de dominio Rust (`src-tauri/crates`) al stack Tauri

> Estado: mapeo base actualizado; los crates de dominio ya fueron movidos físicamente a `src-tauri/crates/` en `feat/tauri`.

## Objetivo

Reusar el dominio CAM existente ahora ubicado en `src-tauri/crates/` desde el host Tauri (`src-tauri/`) sin arrastrar egui/eframe al nuevo shell.

## Inventario de crates

| Crate | Rol | Decisión inicial | Notas |
|---|---|---|---|
| `cam-units` | mm, ángulos, formateo decimal | **Reusar** | Base sin UI. |
| `cam-geom` | geometría arc-native, booleans, offsets, regiones | **Reusar** | Núcleo crítico; mantener determinismo y tests. |
| `cam-move` | vocabulario de movimientos | **Reusar** | Contrato toolpath/emitter/sim. |
| `cam-model` | documento, capas, entidades, validación | **Reusar** | Fuente de verdad del documento. |
| `cam-ops` | operaciones CAM (follow, drill, rings, etc.) | **Reusar** | Evaluar superficie PCB actual vs legado. |
| `cam-store` | `.cmc`, SQLite tool library, migraciones | **Reusar** | Adaptar paths/config al host Tauri. |
| `cam-post` | parser/emitter `.pp` | **Reusar** | Sin UI. |
| `cam-sim` | simulación heightfield/remoción | **Reusar** | Candidato a worker/background. |
| `cam-excellon` | parser Excellon | **Reusar** | Entrada PCB. |
| `cam-gerber` | parser Gerber RS-274X | **Reusar** | Entrada PCB. |
| `cam-export` | export DXF/SVG/Gerber/Excellon según soporte | **Reusar** | Confirmar alcance actual. |
| `cam-import` | import DXF/SVG/folder/report | **Reusar** | Adaptar reportes a IPC. |
| `cam-app` | state machine/comandos/undo | **Reusar/adaptar** | Buen candidato a núcleo detrás de comandos Tauri; revisar acople a modelo actual. |
| `cam-view` | escena/picking/snap/cámara agnóstica | **Reusar/adaptar** | Útil para snapshot/scene data; no depende de egui. |
| `cam-cli` | superficie headless | **Diferir** | Útil después; no bloquea shell Tauri. |
| `cam-ui` | shell egui | **Retirar** | Reemplazado por Svelte/Tauri. |
| `cam-desktop` | host eframe | **Retirar** | Reemplazado por `src-tauri/`. |

## Estrategia de integración

### Layout elegido — crates dentro de `src-tauri/`

- Los crates de dominio viven en `src-tauri/crates/<name>`.
- El `Cargo.toml` raíz sigue siendo el workspace root e incluye `src-tauri` y `src-tauri/crates/cam-*`.
- `src-tauri` consume crates de dominio vía dependencias `workspace = true`.
- Ventaja: el dominio queda físicamente junto al host Tauri; un solo grafo Cargo en raíz.
- Tradeoff: se desvía de la Opción A originalmente recomendada (`crates/<name>` en raíz del repo). Si se quiere recuperar esa separación, hay que hacer otro move explícito.

## Fases propuestas

### F0 — Preparación
- Decidir layout workspace (A o B).
- Asegurar que crates de dominio compilan en isolation (sin egui).
- Definir feature flags/límites para no traer `cam-ui`/`cam-desktop`.

### F1 — Núcleo de documento
- Integrar `cam-units`, `cam-geom`, `cam-move`, `cam-model`, `cam-store`, `cam-app`.
- Exponer comandos Tauri mínimos: nuevo documento, abrir, guardar, estado, undo/redo.
- Definir DTOs IPC (no exponer tipos internos sin contrato).

### F2 — Import/export/parsers
- Integrar `cam-import`, `cam-excellon`, `cam-gerber`, `cam-export`.
- Comandos: importar archivos/carpeta, reporte de import, exportar.

### F3 — Operaciones y simulación
- Integrar `cam-ops`, `cam-post`, `cam-sim`.
- Ejecutar trabajos pesados fuera del hilo UI/Tauri main; emitir progreso por eventos.

### F4 — Viewport/scene
- Integrar `cam-view` como generador de snapshot/scene.
- Definir formato de escena para frontend (JSON inicial, binario si hace falta).

## Contrato IPC (borrador)

```text
Tauri command -> adapter Rust -> cam-app/cam-* -> DTO/Evento -> Svelte
```

Reglas:
- DTOs serializables y versionables; no filtrar structs internas por accidente.
- Errores con código + mensaje humano + detalle técnico opcional.
- Comandos mutantes pasan por validación del dominio.
- Trabajos largos: `start_*` + evento `*_progress` + `*_finished`/`*_failed`.

## Riesgos

- **Acople a egui oculto**: auditar dependencias antes de mover crates.
- **Costo de mover crates**: paths, CI, scripts, ODD claims; hacerlo por slices.
- **Modelos grandes por IPC**: empezar con snapshots gruesos, medir, luego diff/binario.
- **Undo/estado**: `cam-app` puede requerir adaptación para el modelo de comandos Tauri.

## Próximo paso concreto

1. Crear tarea ODD para F0: "workspace/domain integration baseline".
2. Elegir A o B.
3. Hacer un spike chico: compilar `cam-model` + `cam-store` + `cam-app` desde `src-tauri` con un comando `document_status`.
