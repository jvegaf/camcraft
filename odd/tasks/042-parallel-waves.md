# 042 — Olas de trabajo paralelo y el tooling de claims que las sostiene

Mapa de olas concretas para el roadmap camcraft, y las tres correcciones que la 041 dejó pendientes para
que varios agentes puedan trabajar en worktrees sin pisarse. Documento de tarea del flujo ODD.

## Surfaces

- `odd/roadmap/waves.md`
- `odd/roadmap/index.md`
- `odd/tasks/042-parallel-waves.md`
- `odd/tasks/INDEX.md`
- `scripts/claim-task.sh`
- `scripts/check-worktree.sh`
- `scripts/check.sh`
- `scripts/deliver-task.sh`
- `scripts/test-worktree-tooling.sh`
- `scripts/tests/claim.sh`
- `scripts/tests/gate.sh`
- `scripts/tests/deliver.sh`
- `scripts/tests/lib/common.sh`
- `justfile`
- `AGENTS.md`

## Specs

- **S1.** «armame el mapa de olas concreto junto con el documento en odd/» — un documento en `odd/` que
  diga qué se puede construir en paralelo, con qué agentes, sobre qué archivos y en qué orden de merge.
- **S2.** «despues corrige esos tres puntos del tooling», que son los que el índice de roadmap dejó
  nombrados: (a) cablear `check-worktree` dentro de `scripts/check.sh`; (b) reclamar por **superficie
  de archivos** (paths/globs del documento de tarea) en vez de por crate; (c) mover la escritura del
  `odd/tasks/INDEX.md` del claim al merge.
- **S3.** «los worktrees los voy a generar yo desde herdr, y puse en la configuracion de herdr que la
  ubicacion de los worktrees seran en ~/.herdr/worktrees» — el tooling **no crea** worktrees, no asume
  `.worktrees/`, y resuelve cualquier ubicación que `git worktree list` declare.
- **S4.** El claim debe decir **qué superficie y de quién** cuando hay colisión, porque un agente que
  tiene que adivinar esquiva la herramienta.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Escribir `odd/roadmap/waves.md` con las olas, agentes, superficies y orden de merge | S1 | inline | `40195e1` |
| T2 | Enlazar el mapa desde `odd/roadmap/index.md` | S1 | inline | `40195e1` |
| T3 | Tests de comportamiento del tooling, uno por comportamiento para que cada commit lleve el suyo: claim, gate, deliver | S4 | inline | `6b7af24` `6917f42` `bf80cd7` |
| T4 | `claim-task.sh`: reclamar superficies del bloque `## Surfaces`, con mensaje de colisión por superficie | S2b, S4 | inline | `6b7af24` |
| T5 | `check.sh`: fallar en worktree vinculado sin claim y en el árbol primario con trabajo | S2a | inline | `6917f42` |
| T6 | `deliver-task.sh` + `just deliver`: INDEX y liberación del claim en el merge | S2c | inline | `bf80cd7` |
| T7 | `justfile`/`AGENTS.md`: worktrees desde herdr en `~/.herdr/worktrees` | S3 | inline | `bf80cd7` `7e29f68` |
| T8 | Verificación: test del tooling en verde y `./scripts/check-docs.sh` | S1–S4 | inline | — |

## Log

- **L1.** Pedido original, verbatim: «armame el mapa de olas concreto junto con el documento en odd/ y
  despues corrige esos tres puntos del tooling.Por cierto. los worktrees los voy a generar yo desde
  herdr, y puse en la configuracion de herdr que la ubicacion de los worktrees seran en
  ~/.herdr/worktrees , que te parece?»
- **L2.** Decisión de diseño para S2b: la superficie se declara en un bloque `## Surfaces` del
  documento de tarea, un path o glob relativo al repo por línea. Dos tareas colisionan si un path es
  igual a otro o si uno es prefijo de directorio del otro. Un documento sin bloque `## Surfaces` cae
  al modo de la 041 (tokens `crates/<name>` en el texto) y el claim lo avisa.
- **L3.** Decisión de diseño para S2c: `just claim` ya no escribe el INDEX. Lo escribe
  `just deliver N <commit>`, que corre en el árbol primario después del merge, marca `delivered`,
  registra el commit y libera el claim.
- **L4.** Respuesta a la pregunta de S3 en la respuesta al humano y en `AGENTS.md`: herdr como dueño de
  los worktrees es correcto y el tooling ya se apoya en `git worktree list`; lo único que había que
  arreglar era `just worktree`, que creaba en `.worktrees/` y competía con herdr. Queda reducido a un
  mensaje que explica que el worktree lo crea herdr.

- **L5.** Corte en commits revisables, a pedido del humano, antes de abrir RDD sobre el candidato. Los
  slices son, en orden, los que quedaron en la rama `feat/042-parallel-waves`:

  | Slice | Commit | Qué lleva |
  | --- | --- | --- |
  | 1 | `40195e1` | `docs(roadmap)` — auditoría F0–F8 del índice y el mapa de olas |
  | 2 | `ee26aba` | `docs(odd)` — este documento y su fila en el INDEX |
  | 3 | `6b7af24` | `feat(claims)` — claim por superficies, con su fixture y su test |
  | 4 | `6917f42` | `feat(gate)` — `--gate` y el gate que corre el claim |
  | 5 | `bf80cd7` | `feat(claims)` — `deliver-task.sh`, su test y las recetas del `justfile` |
  | 6 | `7e29f68` | `docs(agents)` — las reglas que el agente lee |

  El slice 3 pasó las ~400 líneas autoradas (≈480): el claim nuevo y el arnés de test que lo prueba son
  una sola unidad — el arnés sin la implementación no pasa y la implementación sin el arnés no se
  puede juzgar —, así que se dejó junto y se registra acá en lugar de partirse en un commit que no
  sería verde. Los demás quedaron por debajo.
- **L6.** Cada slice se verificó sobre su propio árbol (`git archive <commit>` en un directorio temporal,
  no sobre el working tree): `scripts/test-worktree-tooling.sh` corre 40 casos en tres archivos — 17 de
  claim, 14 de gate, 9 de deliver — y los tres quedan en verde en cada commit que los contiene.

