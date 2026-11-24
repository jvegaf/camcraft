# Plan de migración: egui → Tauri 2

> **Estado**: decisión tomada; scaffolding inicial creado en `feat/tauri`.
> **Fecha**: 2026-05-24 (actualizado con scaffolding real)
> **Decisión**: quedarse con **Tauri 2 + webview** para el shell desktop de Camcraft.

## Contexto

Camcraft usa hoy `cam-ui` (~16.8k LOC) sobre `egui/eframe 0.37`, con `cam-desktop` como host. El design system está definido en HTML/CSS en `design/system.md` y artboards en `design/`. La paridad visual con egui es costosa y ruidosa porque egui no es HTML/CSS.

Se ejecutaron dos spikes en paralelo (worktrees 077/078):

| Spike | Branch | Resultado |
| --- | --- | --- |
| 077 Tauri 2 + webview | `feat/077-spike-tauri-webview` @ `1b41dec` | DS HTML literal, bridge Rust→webview con `cam-view/cam-model/cam-ops/cam-sim/cam-app`, viewport WebGL/Three.js, overlay. FPS formal pendiente. |
| 078 GPUI nativo | `feat/078-spike-gpui-native` @ `7cf44b8` | DS mapeable pero no literal, integración directa Rust, viewport por proyección 2D CPU en canvas GPUI. Riesgo alto en render 3D; build pesado; parche `xattr`. |

Reportes: `.worktrees/077/spikes/tauri/REPORT.md` y `.worktrees/078/spikes/gpui/REPORT.md` (evidencia en worktrees al momento de la decisión).

## Por qué Tauri

- **Fidelidad DS**: el DS ya es HTML/CSS; Tauri lo vuelve fuente ejecutable.
- **Dominio Rust reusable**: `cam-view`/`cam-app`/`cam-ops`/`cam-model`/`cam-sim`/`cam-geom` se mantienen como backend.
- **Riesgo acotado**: el riesgo principal es viewport/picking/performance, no el chrome. El spike ya probó el bridge.
- **GPUI descartado por ahora**: buen encaje Rust, pero no literal para DS y el viewport 3D requeriría renderer GPU serio o quedaría limitado por canvas 2D CPU.

## Principios de migración

- **No big-bang.** Migrar por slices verticales, con paridad funcional antes de retirar egui.
- **Dominio Rust intacto.** La UI no duplica lógica CAM.
- **Contrato IPC explícito.** Comandos gruesos, eventos/diffs para estado vivo; nada de chatty UI por frame salvo canal dedicado.
- **Una fuente de verdad.** Rust dominio manda; frontend guarda estado de vista.
- **egui como legacy temporal.** Detrás de feature flag/fallback hasta paridad.

## Scaffolding real (feat/tauri)

```text
src/                     # Svelte 5 + SvelteKit frontend
  routes/
  app.html
src-tauri/               # Tauri 2 host Rust
  src/main.rs
  src/lib.rs
  capabilities/
  tauri.conf.json
static/                  # assets estáticos
styled-system/           # Panda CSS generado (no editar a mano)
panda.config.ts
vite.config.js
svelte.config.js
biome.json
package.json             # pnpm, scripts dev/build/check/lint/test
prev/                    # legado Rust/egui en migración (crates/, Cargo.toml, justfile, spikes)
design/                  # fuente DS
docs/                    # documentación
odd/                     # ODD/roadmap/tareas
```

Decisiones ya tomadas por el scaffolding:

- Frontend en `src/` (SvelteKit + Vite).
- Host Tauri en `src-tauri/`.
- Estilos con Panda CSS + `styled-system/`.
- Lint/format con Biome.
- Tests con Vitest + Testing Library.
- Logging con `tauri-plugin-log`.

Pendientes:

- Integrar crates de dominio desde `prev/crates/` al nuevo workspace/host.
- Definir `justfile`/scripts de gate para el nuevo stack.
- Cablear review visual para webview.

## Boundary de responsabilidades

| Capa | Responsabilidad | No hace |
| --- | --- | --- |
| Frontend webview | Chrome DS, input UI, render viewport (WebGL/WebGPU), estado de vista local | Lógica CAM, geometría autoritativa, persistencia de dominio |
| Tauri commands/events | Bridge tipado, validación, origen, colas/cancelación | Reglas de negocio CAM |
| `cam-app`/`cam-ops`/`cam-model` | Comandos, documento, operaciones, validación | Render HTML/CSS |
| `cam-view` | Escena, cámara, picking, snapshot/draw data serializable | Widgets/DOM |
| `cam-sim`/`cam-geom` | Simulación y geometría | IPC/UI |

## Contrato IPC inicial (mínimo)

- `load_scene` → snapshot de escena (stock, mesh/lines, cámara, bounds).
- `document_command` / `project_command` → mutaciones de dominio.
- `open_project`, `save_project`, `recent_projects`, `settings_get/set`.
- Eventos: `scene_updated`, `job_progress`, `diagnostic`.
- Viewport: snapshot completo al inicio; después diffs/snapshots por cambio, no por frame.

## Fases

### Fase 0 — Scaffolding (hecho, base)

- [x] Crear rama `feat/tauri` y estructura Tauri.
- [x] Frontend SvelteKit + Vite + Panda CSS + Biome.
- [x] Host `src-tauri/` con plugin log/opener.
- [ ] Alinear gates/CI/justfile al stack nuevo.
- [ ] Definir integración de crates de dominio desde `prev/`.

**Salida**: app Tauri mínima compila en Linux.

### Fase 1 — Host + shell mínimo

- Start screen real con DS HTML/CSS.
- Comandos: recents, open/new, settings mínimas.
- **Salida**: start screen funcional con DS real.

### Fase 2 — Viewport read-only

- `cam-view` expone snapshot serializable.
- Frontend renderiza WebGL (Three.js, como spike) o WebGPU si se valida.
- Cámara orbit/pan/zoom.
- FPS medido: 60s orbit continuo, avg/min/1% low.
- **Salida**: documento visible y medido.

### Fase 3 — Interacción 3D

- Picking/selección/hover.
- Overlays/gizmos mínimos.
- Selección sincronizada con UI.
- **Salida**: selección 3D refleja estado.

### Fase 4 — Paridad de shell

- Floating shell, browser, inspector, status bar, settings.
- Migración por paneles.
- egui legacy detrás de flag.
- **Salida**: flujos principales sin egui.

### Fase 5 — Retiro egui

- Eliminar `cam-ui` egui y deps eframe/wgpu-egui del camino principal.
- Actualizar docs, CI, packaging, review visual.
- **Salida**: `just check/test` verde sin egui.

## Documentación a actualizar

| Documento | Cambio |
| --- | --- |
| `docs/SYSTEM_DESIGN_CAMCRAFT.md` | Arquitectura Tauri/webview/backend Rust, boundary viewport, IPC. |
| `docs/DOMAIN_INTEGRATION_TAURI.md` | Mapeo de crates `prev/crates/` a reusar/adaptar/retirar e integración con `src-tauri`. |
| `docs/ARCHITECTURE_TAURI.md` | Nuevo: detalle de boundary, protocolo, packaging Linux, seguridad IPC. |
| `README.md` | Dev setup Tauri/webkit2gtk/node, comandos, layout. |
| `AGENTS.md` | Superficies/worktrees para frontend Tauri; mantener ODD/records gate. |
| `design/system.md` | Nota de consumo por frontend web. |
| `justfile` | Recetas desktop dev/build/test; estrategia de review visual. |
| `odd/roadmap/*` | Registrar migración y fases. |

## Riesgos

| Riesgo | Impacto | Mitigación |
| --- | --- | --- |
| Viewport performance/picking | Alto | Medir FPS/picking antes de Fase 4; formato binario si JSON pesa. |
| WebKitGTK/WebGPU en Linux | Medio | Validar temprano; fallback WebGL. |
| Duplicación de estado | Alto | Rust dominio fuente de verdad; frontend solo vista. |
| Packaging Linux | Medio | AppImage/deb, deps webkit2gtk, Wayland/X11. |
| Review visual | Medio | Reemplazar kittest/sway por capturas Playwright/webview; definir política `fixtures/ui`. |
| Costo de migración UI | Alto | Migrar por slices; reusar DS HTML literal. |

## Huecos a cerrar (post-spike)

1. Benchmark FPS formal en Tauri (60s orbit continuo, avg/min/1% low, escena canónica).
2. Picking/selección 3D en webview.
3. Packaging release/tamaño en Linux.
4. Evaluar WebGPU en WebKitGTK; mantener fallback WebGL.

## Evidencia

- Spike Tauri: `.worktrees/077/spikes/tauri/REPORT.md`
- Spike GPUI: `.worktrees/078/spikes/gpui/REPORT.md`
- Plan común de spikes: `spikes/README.md`
- Escena canónica: `spikes/shared/SCENE.md`
