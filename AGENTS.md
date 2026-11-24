---
page-id: 18
---
# AGENTS.md — Reglas para agentes en camcraft

## Stack

- **Desktop**: Tauri 2 (`src-tauri/`)
- **Frontend**: Svelte 5 + SvelteKit + Vite (`src/`, `static/`)
- **Estilos**: Panda CSS (`panda.config.ts`, `styled-system/` generado)
- **Tests**: Vitest + Testing Library (`@testing-library/svelte`)
- **Lint/format**: Biome (`biome.json`)
- **Paquetes**: pnpm
- **Legado en migración**: `prev/` contiene el workspace Rust/egui anterior (`crates/`, `justfile`, spikes). Es fuente de migración, no código activo nuevo salvo tareas explícitas de migración.

## Ramas: `dev` integra, `master` publica

- **`dev` es el tronco de desarrollo.** Todo el trabajo integrado llega ahí.
- **`master` es sólo releases.** Se mueve únicamente por PR desde `dev`, con tag (`v0.X.0`).
- No pushees directo a `master`.

## Commits revisables por RDD (obligatorio)

- **Cada commit debe ser revisable por Receipt-Driven Development.** Mantené los commits chicos: objetivo ~400 líneas de cambio autoral por unidad de trabajo, con tests y docs junto al comportamiento que cambian.
- Si un cambio excede ese tamaño, partilo en cortes lógicos y revisables; no mezcles refactor grande, feature y limpieza no relacionada en el mismo commit.
- Commits de scaffolding/migración mecánica también se parten por slices revisables cuando sea posible; si no, explicá la razón en el mensaje o en el Log de la tarea.
- No commitees a menos que el usuario lo pida explícitamente, salvo los commits de unidad de trabajo autorizados por el flujo ODD en tareas grandes.

## Desarrollo en worktrees (obligatorio para trabajo paralelo)

- **Un agente = un worktree = una tarea.** Trabajá siempre en un worktree dedicado para la tarea que tomaste de `odd/tasks/INDEX.md`.
- **Worktrees dentro del repo.** `just worktree N` crea `.worktrees/N` con la rama `feat/N-<slug>` (derivada del documento `odd/tasks/N-*.md`). Trabajá desde ahí (`cd .worktrees/N`).
- **Rama dedicada.** Usá `feat/NNN-<slug>` para cada tarea.
- **Declará tus superficies antes de reclamar.** El documento de la tarea lleva un bloque:

  ```markdown
  ## Surfaces

  - `src/routes/+page.svelte`
  - `src-tauri/src/lib.rs`
  ```

  Un path o glob relativo al repo por línea. Sin ese bloque el claim cae al modo viejo (tokens en texto) y avisa: no lo dejes pasar.
- **Reclamá antes del primer write.** `just claim N` desde el worktree. Falla si otra tarea ya reclamó una superficie que se superpone.
- **Validar.** `just check-worktree` valida que trabajás en un worktree reclamado.
- **No escribas el INDEX.** `odd/tasks/INDEX.md` lo actualiza **quien mergea**, con `just deliver N <commit>`, que además libera el claim.
- **Liberar.** Si abandonás la tarea, `just release N`. Si la entregás, `just deliver N <commit>`.

## Registro obligatorio (gate)

- Nada se da por completado sin estar registrado en **ODD** (documento de tarea + fila en `odd/tasks/INDEX.md`) y en **Engram** (una memoria que nombre el slug de la tarea).
- `scripts/check-records.sh` lo verifica; `scripts/deliver-task.sh` lo exige con `--engram-required` **antes** de escribir `delivered`.
- `just check`/gates corren el chequeo de records cuando aplique. Los documentos anteriores a la tarea 053 se informan como avisos, no se fallan.

## Flujo ODD

- Seguí el flujo ODD del sistema: autorizar/explorar/resolver/clasificar/track/implementar/cerrar.
- Para tareas grandes, usá el documento de tarea en `odd/tasks/` con Specs verbatim, Tasks y Log.
- Antes de abrir un worktree, mirá `odd/roadmap/waves.md`: dice qué se puede hacer en paralelo, con qué superficies y en qué orden de merge.
- Commitá en cortes revisables siguiendo las convenciones del repo.

## Comandos útiles

```bash
# Frontend
pnpm install
pnpm dev            # Vite dev server
pnpm tauri dev      # app Tauri en desarrollo
pnpm build          # build frontend
pnpm tauri build    # build desktop
pnpm check          # svelte-check
pnpm lint           # biome check
pnpm format         # biome check --write
pnpm test           # vitest (cuando haya tests configurados)

# Tauri/Rust
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml

# ODD/worktrees
just claim N
just release N
just deliver N <commit>
just check-worktree
```

> Nota: el `justfile` raíz está en migración; si una receta no existe, usá los comandos `pnpm`/`cargo` equivalentes y actualizá este documento.

## Skills del proyecto

- `.agents/skills/odd-docs/SKILL.md` — disciplina de documentación ODD (nombres, splits, referencias).
- `.agents/skills/ui-visual-review/SKILL.md` — iteración visual de la UI contra referencias en `fixtures/ui/`.
- `.agents/skills/tauri-dev/SKILL.md` — desarrollo Tauri 2: comandos, IPC, capabilities, plugins, packaging.
- `.agents/skills/svelte-dev/SKILL.md` — desarrollo Svelte 5/SvelteKit: runes, rutas, componentes, estado.
- `.agents/skills/panda-css/SKILL.md` — estilos con Panda CSS: tokens, recipes, patterns, codegen.
