# Worktree-only development enforcement

Force agents to develop exclusively in git worktrees (one agent = one worktree = one claimed task) to avoid multiple agents editing the same packages concurrently.

## Specs

**S1** (request): "podrias agregarlo al odd/engram como tarea pendiente con alta prioridad? de que manera podemos agregar las reglas de desarrollo para que los agentes solamente desarollen mediante worktrees? es que creo que podiamos aumentar el trabajo en diferentes tareas que previamente se han debido seleccionar para evitar que dos o mas agentes esten trabajando en el mismo paquete....."

**S2** (requirement): One agent = one worktree = one claimed task from `odd/tasks/INDEX.md`; forbid work in the primary tree/`master` when a task is in progress; enforce task claiming with branch/worktree/assignee/crates and prevent collisions.

**S3** (requirement): Provide `just` recipes to create/claim/release worktrees (`just worktree N`, `just claim N`, `just release N`), add worktree validation (script/check + optional git hooks), integrate with ODD "Track before the first write" to claim+create worktree+switch, and support package-surface tracking to avoid overlapping claims on the same crates.

## Tasks

| ID | Task | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | Add convention to `odd/README.md` and `AGENTS.md` (worktree-only per task) | S2 | inline | — |
| T2 | Add `just` recipes for worktree creation/claim/release | S2,S3 | inline | — |
| T3 | Create claim mechanism (`odd/tasks/.claims/NNN.json` or equivalent) with atomic locking and collision detection | S2,S3 | inline | — |
| T4 | Add validation script (`scripts/check-worktree.sh`) and integrate into `just check`/pre-commit hooks guidance | S2,S3 | inline | — |
| T5 | Integrate with ODD workflow (claim+worktree on task pickup) and document in task process | S3 | inline | — |

## Log

**L1** (user, verbatim): "podrias agregarlo al odd/engram como tarea pendiente con alta prioridad? de que manera podemos agregar las reglas de desarrollo para que los agentes solamente desarollen mediante worktrees? es que creo que podiamos aumentar el trabajo en diferentes tareas que previamente se han debido seleccionar para evitar que dos o mas agentes esten trabajando en el mismo paquete....."
