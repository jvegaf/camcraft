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
- `crates/cam-ui/src/shell/action.rs`

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

- **L7.** El review nativo (RDD) sobre el candidato de esta rama, `sha256:07e6e697…`, linaje
  `review-f8d00f1d61933e35`: tier **high**, 1.012 líneas cambiadas, presupuesto de corrección 200,
  cuatro lentes (risk, resilience, readability, reliability) más refuter y validador dirigido.

  1. Refuter: **siete hallazgos CRITICAL, todos `introduced`** — `deliver` corrible desde un worktree
     vinculado; globs de superficie que no colisionaban; el gate parseando `git status` (rename staged
     invisible, `|| true` convirtiendo un fallo en limpio); claim escrito en el archivo final con el
     escaneo ignorando claims ilegibles; INDEX reescrito en el lugar.
  2. Corrección acotada, plan 160 líneas de diff: commit `cbfe321`.
  3. Validador dirigido: **rechazó la corrección**. Aceptó cinco arreglos, y rechazó dos con razón:
     un surface que empieza con glob (`*.rs`) no tenía prefijo literal y no colisionaba con nada, y el
     escaneo normalizaba sólo el claim nuevo, no el viejo. Además encontró una **regresión
     introducida por la corrección**: la guarda de `deliver` comparaba el directorio del shell con la
     raíz del árbol, así que correrlo desde un subdirectorio del árbol primario fallaba.
  4. Estado del linaje: **`escalated`** (`targeted_validator_rejected`), sin segunda corrección en el
     presupuesto. No se reinicia, no se reintenta y no se reusa su autoridad.
  5. Los tres puntos abiertos se cerraron como trabajo nuevo en `9bdcbc4`, con casos de test propios:
     glob sin prefijo literal, claim viejo sin normalizar, y claim/entrega desde subdirectorio. La
     misma regla — preguntarle al árbol por su raíz — quedó aplicada a `claim`, `deliver` y el gate, lo
     que cierra la asimetría que el registro de la 041 listaba como abierta.
  6. Verificación: tooling **61 casos en verde** (29 claim, 13 deliver, 19 gate), `check-docs` 92
     archivos, y `just check` completo en verde con `EXIT=0` medido dentro del comando.

  **Consecuencia abierta:** el candidato corregido es un candidato **distinto** del que el linaje
  revisó, y no está revisado por el proveedor. Revisarlo es abrir un linaje nuevo, con su propio
  consentimiento; la entrega sigue siendo decisión del dueño.

- **L8.** Segundo review, sobre el candidato corregido (`sha256:eb6acebd…`), linaje
  **`review-db8530b23d8a207a`**: tier high, 1.210 líneas, presupuesto 200. Refuter → **tres hallazgos
  CRITICAL, los tres `introduced`**:
  - el staging del claim se creaba en una ruta predecible y con el umask por defecto (world-readable, y
    un symlink pre-plantado se seguía);
  - `enforced_base` no quitaba la barra final, así que un claim viejo con `crates/cam-gerber/` no
    colisionaba con `crates/cam-gerber`;
  - `deliver-task.sh` descartaba el tercer argumento en silencio mientras reportaba éxito.

  Corrección: commit `74a0a7a`. Validador dirigido: **aprobado**, y el acknowledgement quemó la
  autoridad (`gentle-ai.review-acknowledged/v1`).

  **Advisories no bloqueantes** que el review dejó anotados como trabajo posterior, no como motivo para
  volver a revisar este candidato:
  - `R4-deliver-lost-update` (`deliver-task.sh:49-68`): dos entregas concurrentes hacen read-modify-write
    del INDEX sin bloqueo; una puede pisar a la otra. Vale un `flock` antes de la siguiente ola.
  - `R4-stale-worktree-remedy` (`check-worktree.sh:32`): el mensaje de worktree sin claim no dice qué
    hacer con un worktree viejo.
  - `R2-002` (`AGENTS.md:18`), `R2-003` (`waves.md:31`), `R2-004`/`R2-005`/`R2-006` (tests e INDEX):
    redacción y nombres.
  - `R1-003`, `R2-001`, `R3-001`, `R4-undeclared-surface`: senalaban que este bloque `## Surfaces` no
    declaraba `crates/cam-ui/src/shell/action.rs`, que es cierto y quedó corregido acá.

