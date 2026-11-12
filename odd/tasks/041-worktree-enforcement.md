# Worktree-only development enforcement

Force agents to develop exclusively in git worktrees (one agent = one worktree = one claimed task) to avoid multiple agents editing the same packages concurrently.

## Specs

**S1** (request): "podrias agregarlo al odd/engram como tarea pendiente con alta prioridad? de que manera podemos agregar las reglas de desarrollo para que los agentes solamente desarollen mediante worktrees? es que creo que podiamos aumentar el trabajo en diferentes tareas que previamente se han debido seleccionar para evitar que dos o mas agentes esten trabajando en el mismo paquete....."

**S2** (requirement): One agent = one worktree = one claimed task from `odd/tasks/INDEX.md`; forbid work in the primary tree/`master` when a task is in progress; enforce task claiming with branch/worktree/assignee/crates and prevent collisions.

**S3** (requirement): Provide `just` recipes to create/claim/release worktrees (`just worktree N`, `just claim N`, `just release N`), add worktree validation (script/check + optional git hooks), integrate with ODD "Track before the first write" to claim+create worktree+switch, and support package-surface tracking to avoid overlapping claims on the same crates.

## Tasks

| ID | Task | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ Add convention to `odd/README.md` and `AGENTS.md` (worktree-only per task) | S2 | inline | `—` |
| T2 | ✅ Add `just` recipes for worktree creation/claim/release | S2,S3 | inline | `—` |
| T3 | ✅ Create claim mechanism (`odd/tasks/.claims/NNN.json`) with collision detection | S2,S3 | inline | `9dd659c` |
| T4 | ✅ Add validation script (`scripts/check-worktree.sh`) and recipes integration | S2,S3 | inline | `9dd659c` |
| T5 | ✅ Integrate with ODD workflow/docs (`odd/README.md`, `AGENTS.md`) | S3 | inline | `—` |
| T6 | ✅ Harden claim/worktree enforcement after review (shared claims, primary-tree refusal, crate collisions, atomic claim) | S2,S3 | inline | `9dd659c` |
| T7 | ✅ Require claim ownership on release; release from any worktree subdirectory | S2,S3 | inline | `4fc46a6`, `e3369d7` |

## Log

**L1** (user, verbatim): "podrias agregarlo al odd/engram como tarea pendiente con alta prioridad? de que manera podemos agregar las reglas de desarrollo para que los agentes solamente desarollen mediante worktrees? es que creo que podiamos aumentar el trabajo en diferentes tareas que previamente se han debido seleccionar para evitar que dos o mas agentes esten trabajando en el mismo paquete....."

**L2** (implementado): Agregadas reglas en `odd/README.md` y `AGENTS.md`. Recipes `just worktree/claim/release/check-worktree`, scripts de claim/release/validación, directorio `odd/tasks/.claims/`. Integrado con flujo ODD.

**L3** (review RDD, correcciones): La primera implementación tenía los claims dentro de cada worktree (`git rev-parse --show-toplevel`), así que dos agentes en worktrees distintos podían reclamar la misma tarea; `check-worktree.sh` sólo rechazaba el árbol primario si estaba en `master`/`main` y nunca leía claims; y no había detección de colisión por crates ni creación atómica. Corregido en `9dd659c` (claims compartidos en `$MAIN/odd/tasks/.claims`, rechazo del árbol primario en cualquier rama, exigencia de worktree registrado + claim coincidente, colisión por task y por crates, `noclobber`+`mv`).

La validación dirigida de esa corrección encontró una regresión: `release-task.sh` borraba claims del directorio compartido sin verificar que el worktree actual fuera el dueño. Corregido en `4fc46a6` (rechazo del primario, worktree registrado, comparación del `worktree` grabado). El refuter de la review siguiente encontró que el guard comparaba `pwd -P` exacto contra los roots, y por eso `just release N` desde un subdirectorio del worktree fallaba en falso; corregido en `e3369d7` derivando el root con `git rev-parse --show-toplevel`. La review quedó aprobada y reconocida (autoridad burn).

Comportamiento verificado a mano en un repo temporal (sin test automatizado): liberar desde el árbol primario falla; desde un worktree ajeno falla y deja el claim intacto; desde el worktree dueño (raíz o subdirectorio) libera. Advisories no bloqueantes pendientes: `R1-WORKTREE-ROOT` y `R3-GUARD-ORDER` (orden/derivación del root en `release-task.sh:14-21`), `R3-MISSING-WORKTREE-FIELD` (claim sin campo `worktree`), `R3-TEST-GAP` (sin test de los scripts).
