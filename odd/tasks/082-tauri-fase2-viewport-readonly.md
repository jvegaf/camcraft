# 082 — Fase 2: viewport read-only

Exponer snapshot serializable desde cam-view y renderizar viewport WebGL read-only con cámara orbit/pan/zoom y medición FPS.

## Surfaces

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src/lib/components/workbench/**`
- `src/routes/workspace/+page.svelte` o ruta viewport
- `package.json`
- `docs/MIGRATION_TAURI.md`
- `odd/tasks/082-tauri-fase2-viewport-readonly.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «perfecto, continua»

## Tasks

| ID  | Tarea                                                           | Specs | Ruta   | Commit    |
| --- | --------------------------------------------------------------- | ----- | ------ | --------- |
| T1  | Agregar comando Tauri de scene snapshot serializable (cam-view) | S1    | inline | `4a91aa2` |
| T2  | Renderizar viewport Three.js con orbit/pan/zoom                 | S1    | inline | `4a91aa2` |
| T3  | Verificar, RDD, ODD/Engram, commit y push                       | S1    | inline | `4a91aa2` |

## Log

- **L1 (pedido original, verbatim).** «perfecto, continua»
- **L2 (contexto).** Continúa Fase 2 de `docs/MIGRATION_TAURI.md`: cam-view snapshot serializable, frontend WebGL/Three.js, cámara orbit/pan/zoom, FPS medido.
- **L3 (ejecución, 2026-11-24).** Se agregó `scene_snapshot` en `src-tauri/src/lib.rs` con DTOs serializables construidos desde `cam-view`/`cam-sim` (escena canónica stock 100x100x20, pocket 60x40x5). Frontend: Three.js en `BoardCanvas`, OrbitControls, HUD FPS avg/min/1% low. Workspace renderiza `WorkbenchShell`. Verificados `cargo check --manifest-path src-tauri/Cargo.toml` y `pnpm check`.
- **L4 (cierre, 2026-11-24).** Commit `4a91aa2`. RDD assess: not-due (under_budget, medium). ODD/Engram sincronizados; push a `origin/feat/tauri`.
