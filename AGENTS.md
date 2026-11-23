---
page-id: 18
---
# AGENTS.md — Reglas para agentes en camcraft

## Ramas: `dev` integra, `master` publica
- **`dev` es el tronco de desarrollo.** Todo el trabajo integrado llega ahí; los pushes a `dev` corren los gates baratos (`gate` siempre, `parity` sólo si el push toca parsers, kernel, corpus o arnés).
- **`master` es sólo releases.** Se mueve únicamente por PR desde `dev`, con tag (`v0.X.0`), y está protegida: `gate`, `parity` y `bench` son checks requeridos. No pushees directo a `master`.
- El flujo completo, con los comandos de release, está en `odd/roadmap/workflow.md`.
- **Instalá los hooks una vez por clon:** `just hooks` (pre-commit: fmt, links de docs, build del medidor de paridad; pre-push: clippy y tests de los crates tocados por el push —o de todo el workspace si cambian `Cargo.toml`/`Cargo.lock` de raíz, cambia Rust fuera de `crates/`, o se fuerza con `PRE_PUSH_FULL=1`— y `parity` cuando el push puede moverlo).

## Desarrollo en worktrees (obligatorio para trabajo paralelo)
- **Un agente = un worktree = una tarea.** Trabajá siempre en un worktree dedicado para la tarea que tomaste de `odd/tasks/INDEX.md`. Las ramas de tarea salen de `dev`, no de `master`.
- **Worktrees dentro del repo.** `just worktree N` crea `.worktrees/N` con la rama `feat/N-<slug>` (derivada del documento `odd/tasks/N-*.md`). Trabajá desde ahí (`cd .worktrees/N`); los scripts resuelven el árbol con `git worktree list`, así que la ubicación registrada es la que vale. `.worktrees/` está ignorado por git.
- **Rama dedicada.** Usá `feat/NNN-<slug>` (o convención acordada) para cada tarea.
- **Declará tus superficies antes de reclamar.** El documento de la tarea lleva un bloque:

  ```markdown
  ## Surfaces

  - `crates/cam-geom/src/boolean.rs`
  - `fixtures/corpus/`
  ```

  Un path o glob relativo al repo por línea. Sin ese bloque el claim cae al modo viejo (tokens `crates/<name>` en el texto) y avisa: no lo dejes pasar.
- **Reclamá antes del primer write.** `just claim N` desde el worktree. Falla si otra tarea ya reclamó una superficie que **se superpone** (mismo path, o uno dentro del otro), y el mensaje dice qué superficie y de quién. Si ya está reclamada, elegí otra tarea.
- **Validar.** `just check` incluye la verificación del claim: en un worktree vinculado sin claim falla, y en el árbol primario falla si hay trabajo sin commitear en `crates/` o `scripts/`. `just test-tooling` corre las pruebas de comportamiento de este tooling.
- **No escribas el INDEX.** `odd/tasks/INDEX.md` lo actualiza **quien mergea**, con `just deliver N <commit>`, que además libera el claim. Un claim nunca toca el índice: lo que se escribe desde un worktree no está en ninguna rama y deriva.
- **Liberar.** Si abandonás la tarea, `just release N`. Si la entregás, `just deliver N <commit>`.

## Registro obligatorio (gate)
- Nada se da por completado sin estar registrado en **ODD** (documento de tarea + fila en `odd/tasks/INDEX.md`) y en **Engram** (una memoria que nombre el slug de la tarea).
- `scripts/check-records.sh` lo verifica; `scripts/deliver-task.sh` lo exige con `--engram-required` **antes** de escribir `delivered`, así que `just deliver N <commit>` se niega si falta el documento, una sección, una entrada de Log, el commit, o la memoria.
- `just check` corre el chequeo como `== records ==` (sin exigir Engram: CI no tiene demonio de memoria). Los documentos anteriores a la tarea 053 se informan como avisos, no se fallan.

## Flujo ODD
- Seguí el flujo ODD del sistema: autorizar/explorar/resolver/clasificar/track/implementar/cerrar.
- Para tareas grandes, usá el documento de tarea en `odd/tasks/` con Specs verbatim, Tasks y Log.
- Antes de abrir un worktree, mirá `odd/roadmap/waves.md`: dice qué se puede hacer en paralelo, con qué superficies y en qué orden de merge. Lo que está en la espina serial no se paraleliza.
- Commitá en cortes revisables siguiendo las convenciones del repo.

## Comandos útiles
- `just claim N` — reclamar tarea N por sus superficies (desde el worktree de la tarea)
- `just release N` — liberar el claim sin entregar
- `just deliver N <commit>` — marcar delivered en el INDEX y liberar el claim (después del merge)
- `just check-worktree` — validar que trabajás en un worktree reclamado
- `just check` — gate completo: claim, docs, fmt, clippy, tests
- `just test-tooling` — pruebas de comportamiento del tooling de claims

## Skills del proyecto
- `.agents/skills/odd-docs/SKILL.md` — disciplina de documentación ODD (nombres, splits, referencias).
- `.agents/skills/ui-visual-review/SKILL.md` — iteración visual de cam-ui contra referencias en `fixtures/ui/`.
