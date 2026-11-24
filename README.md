---
page-id: 19
---
# Camcraft

Aplicación CAM de escritorio para **fabricación de PCBs**, en migración a **Tauri 2 + Svelte 5 + Panda CSS**.

El objetivo sigue siendo el mismo: un modelo de documento y un núcleo geométrico/CAM confiables, con una UI de escritorio usable. La migración actual cambia el shell: de egui a webview (Tauri), manteniendo el dominio Rust como fuente de verdad.

**Estado.** Scaffolding inicial creado en `feat/tauri` (Tauri 2, Vite, SvelteKit, Svelte 5, Panda CSS, Biome, Vitest, Testing Library, `tauri-plugin-log`). El código Rust/egui anterior está en `prev/` como fuente de migración, no como código activo nuevo.

## Stack actual

- **Desktop host**: Tauri 2 (`src-tauri/`)
- **Frontend**: Svelte 5 + SvelteKit + Vite (`src/`)
- **Estilos**: Panda CSS (`panda.config.ts`, `styled-system/`)
- **Lint/format**: Biome
- **Tests**: Vitest + Testing Library
- **Paquetes**: pnpm

## Desarrollo rápido

```bash
pnpm install
pnpm tauri dev      # app desktop en desarrollo
pnpm dev            # solo frontend Vite
pnpm lint           # Biome
pnpm check          # svelte-check
pnpm test           # Vitest
pnpm build          # build frontend
```

Comandos Rust del host:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
```

Si existe `just`, usá `just --list` y `just check`.

## Estructura

| Ruta | Responsabilidad |
| --- | --- |
| `src/` | Frontend Svelte/SvelteKit |
| `src-tauri/` | Host Tauri, comandos IPC, capabilities, plugins |
| `design/` | Design system y artboards HTML/CSS |
| `fixtures/ui/` | Referencias visuales canónicas |
| `docs/` | Documentación de arquitectura, roadmap y migración |
| `odd/` | ODD: tareas, roadmap, decisiones, evidencia |
| `prev/` | Workspace Rust/egui anterior en migración (`crates/`, `justfile`, spikes) |

## Migración a Tauri

La decisión y el plan están en [`docs/MIGRATION_TAURI.md`](docs/MIGRATION_TAURI.md). Resumen:

- No big-bang: migración por slices verticales.
- Dominio Rust intacto; Tauri/webview como shell.
- `prev/` se migra por fases, con paridad funcional antes de retirar egui.

## Documentación

- [`docs/MIGRATION_TAURI.md`](docs/MIGRATION_TAURI.md) — plan de migración Tauri.
- [`docs/SYSTEM_DESIGN_CAMCRAFT.md`](docs/SYSTEM_DESIGN_CAMCRAFT.md) — contrato de arquitectura (en actualización).
- [`docs/ROADMAP_CAMCRAFT.md`](docs/ROADMAP_CAMCRAFT.md) — fases/objetivos del producto.
- [`odd/tasks/INDEX.md`](odd/tasks/INDEX.md) — tareas ODD.
- [`AGENTS.md`](AGENTS.md) — reglas para agentes.

## Desarrollo paralelo con worktrees

Cada agente trabaja en su propio worktree aislado, una tarea por agente:

```bash
just worktree N        # crea .worktrees/N y la rama feat/N-<slug>
cd .worktrees/N
just claim N           # reclama la tarea
# ... trabajar ...
just check-worktree
just release N         # o just deliver N <commit> tras el merge
```

Ver [`AGENTS.md`](AGENTS.md) y [`odd/README.md`](odd/README.md).
