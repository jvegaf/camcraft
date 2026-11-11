# AGENTS.md — Reglas para agentes en camcraft

## Desarrollo en worktrees (obligatorio para trabajo paralelo)
- **Un agente = un worktree = una tarea.** Trabajá siempre en un worktree dedicado para la tarea que tomaste de `odd/tasks/INDEX.md`. No trabajes en el árbol principal (`master`).
- **Rama dedicada.** Usá `feat/NNN-<slug>` (o convención acordada) para cada tarea.
- **Reclamar antes del primer write.** Marcá la tarea como `in progress` en `odd/tasks/INDEX.md`, creá el claim en `odd/tasks/.claims/NNN.json` con `{agent, session_id, branch, worktree, crates[], started_at}` y asegurate de que nadie más la reclama. Si ya está reclamada, elegí otra tarea.
- **Acotar por paquetes.** Registrá los `crates/` afectados en el claim para evitar solapamientos entre agentes.
- **Validar.** Antes de commitear/correr checks, `just check-worktree` debe pasar.
- **Liberar.** Al entregar la tarea (`delivered`), liberá el claim (`just release N`) y coordiná cleanup.

## Flujo ODD
- Seguí el flujo ODD del sistema: autorizar/explorar/resolver/clasificar/track/implementar/cerrar.
- Para tareas grandes, usá el documento de tarea en `odd/tasks/` con Specs verbatim, Tasks y Log.
- Commitá en cortes revisables siguiendo las convenciones del repo.

## Comandos útiles
- `just worktree N` — crear worktree para tarea N
- `just claim N` — reclamar tarea N (requiere worktree activo)
- `just release N` — liberar claim
- `just check-worktree` — validar que trabajás en worktree reclamado correctamente
