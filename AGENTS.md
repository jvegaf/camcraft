# AGENTS.md — Reglas para agentes en camcraft

## Desarrollo en worktrees (obligatorio para trabajo paralelo)
- **Un agente = un worktree = una tarea.** Trabajá siempre en un worktree dedicado para la tarea que tomaste de `odd/tasks/INDEX.md`. No trabajes en el árbol principal (`master`).
- **Los worktrees los crea el humano con herdr**, no este repo: la ruta configurada es `~/.herdr/worktrees/<slug>`. La receta `just worktree N` sólo explica cómo pedirlo; no crea nada, y no hay `.worktrees/` en uso.
- **Brama dedicada.** Usá `feat/NNN-<slug>` (o convención acordada) para cada tarea.
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
