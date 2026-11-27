# 103 — Un solo tronco: `master`, integración híbrida y CI con unitarios

`dev` se mergeó a `master` y se borró: el proyecto vuelve a un tronco, y lo que antes justificaba dos ramas —el costo de medir el corpus en cada integración— pasa a decidirse en los triggers de CI. La integración deja de ser siempre PR: es híbrida por riesgo, con PR para lo riesgoso y merge local para lo chico.

## Surfaces

- `.github/workflows/ci.yml`
- `odd/roadmap/workflow.md`
- `odd/roadmap/waves.md`
- `odd/roadmap/index.md`
- `.githooks/pre-push`
- `odd/tasks/103-single-trunk-workflow.md`
- `odd/tasks/INDEX.md`
- `scripts/tests/ci-workflow.sh`

## Specs

- **S1 (un solo tronco, verbatim).** «Ahora he mergeado dev a master y he borrado dev, por lo que ahora desarrollaremos contra master.» `master` es el único tronco vivo: ningún documento, hook, script ni trigger de CI nombra `dev` ni `feat/tauri` como rama que recibe trabajo.
- **S2 (integración híbrida por riesgo, verbatim).** «Híbrido por riesgo»: los cambios riesgosos —migraciones, contratos públicos, CI, dependencias— entran por PR desde una rama corta; los chicos, con merge local y push directo a `master`. Es la vía elegida por ser el único desarrollador, y la protección de rama de `master` tiene que permitir el bypass de admin para que exista.
- **S3 (CI: unitarios por ahora, verbatim).** «es que el CI lo vamos a modificar, para que de momento solo corra los test unitarios, y ya mas adelante ajustaremos el tema de parity y bench». En un push o un PR a `master`, el CI corre los tests unitarios —vitest y las suites de Rust— y no el gate documental/claim completo.
- **S4 (parity y bench a pedido, verbatim).** «Manual a pedido»: `parity` y `bench` no se disparan en push ni en PR; corren sólo por `workflow_dispatch`.
- **S5 (los nombres de job no se renombran).** `gate`, `parity` y `bench` conservan su nombre: la protección de rama los nombra y renombrar un job lo desrequiere en silencio.
- **S6 (el motivo del desdoblamiento se muda, no se olvida).** `odd/roadmap/workflow.md` cuenta el flujo nuevo —un tronco, la regla híbrida, el release por tag, y qué corre en CI y cuándo— y deja de describir dos troncos y un PR de release.
- **S7 (la historia no se reescribe).** `odd/tasks/053-ci-two-trunks.md` y los documentos ya entregados quedan como están; esta tarea los supersede y lo deja dicho acá.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                              | Specs          | Ruta   | Commit    |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | --------- |
| T1  | `.github/workflows/ci.yml`: triggers de un solo tronco (`push` y `pull_request` sobre `master`, más `workflow_dispatch`), `gate` con los tests unitarios de frontend y Rust, `parity` y `bench` sólo por `workflow_dispatch`, nombres de job intactos                              | S1, S3, S4, S5 | worker | `a06484b` |
| T2  | `odd/roadmap/workflow.md` reescrito al flujo de un tronco: día a día, regla híbrida, release por tag, qué corre en CI y cuándo, y por qué no hay segunda rama                                                                                                                      | S1, S2, S6, S7 | worker | `6e070f8` |
| T3  | `.githooks/pre-push`: el comentario que habla de `dev/feat` dice lo que el hook hace hoy                                                                                                                                                                                           | S1             | worker | `6e070f8` |
| T4  | Este documento y su fila en `odd/tasks/INDEX.md`                                                                                                                                                                                                                                   | S1, S7         | worker | `6e070f8` |
| T5  | `scripts/tests/ci-workflow.sh`: test de regresión que afirma triggers de un solo tronco (push y PR sobre `master`), que ningún trigger nombra `dev` ni `feat/*`, que los jobs `gate`/`parity`/`bench` siguen existiendo y que `parity`/`bench` sólo corren por `workflow_dispatch` | S1, S4, S5     | worker | `a06484b` |

## Log

- **L1 (pedido original, verbatim).**

  ```
  vamos a cambiar el git workflow del proyecto, antes creo que teniamos fijado que desarrollabamos contra dev y master solo lo teniamos para las releases. Ahora he mergeado dev a master y he borrado dev, por lo que ahora desarrollaremos contra master. Lo que no tengo decidido es si crear pr cada vez o mergear en local a master y pushear, dado que solo desarrollamos nosotros.
  ```

- **L2 (alcance de CI, verbatim).** «es que el CI lo vamos a modificar, para que de momento solo corra los test unitarios, y ya mas adelante ajustaremos el tema de parity y bench».

- **L3 (bench, verbatim).** «Manual a pedido».

- **L4 (vía de integración, verbatim).** «Híbrido por riesgo»: PR para lo riesgoso —migraciones, contratos públicos, CI, dependencias— y push directo para lo chico, con bypass de admin.

- **L5 (estado medido por el padre, no de memoria).** `git ls-remote --heads origin` devuelve sólo `refs/heads/master`: `dev` ya no existe en el remoto, y el ref local `remotes/origin/dev` es basura de un fetch viejo. La protección de `master` tiene `enforce_admins: true` y PR obligatorio antes de mergear, con `required_status_checks.contexts` vacío; para que la vía directa exista hay que apagar `enforce_admins`. `ci.yml` dispara `push` en `[dev, feat/tauri]` y `pull_request` en `[master, dev]`, así que hoy un push directo a `master` no corre nada de CI: el trigger de push está muerto y el de PR es la única vía viva.

- **L6 (supersede a la 053).** `odd/tasks/053-ci-two-trunks.md` documentó los dos troncos y el PR de release; esta tarea los reemplaza por un solo tronco. El documento de la 053 no se toca: es historia entregada, y `odd-docs` pide que la historia no se reescriba.

- **L7 (adición autorizada por el padre).** Dos decisiones que las specs no cubren y que el padre ratificó al reabrir la tarea: (a) `gate` corre `pnpm codegen` antes de `pnpm test` porque `styled-system/` está en `.gitignore` y la suite importa `styled-system/*` — es setup, no un gate, y sin él la suite no resuelve sus imports en un clon limpio; (b) la prueba de grep de S1 no podía quedar limpia con las superficies originales, así que el padre extendió el alcance a `odd/roadmap/waves.md` (regla 7) y `odd/roadmap/index.md` (celdas E0.4 y E0.8). La historia fechada queda intacta por S7: `dev` `1396e91` en el audit de `index.md`, `feat/E1.1`/`feat/E1.4` en las filas E1.1/E1.4, `feat/tauri` en `docs/MIGRATION_TAURI.md` y `docs/DOMAIN_INTEGRATION_TAURI.md`, y las ramas cortas `feat/N-<slug>` que S2 sí permite.

- **L8 (corrección del padre).** El párrafo de protección de `odd/roadmap/workflow.md` y la celda E0.8 de `odd/roadmap/index.md` afirmaban que `master` tenía los checks requeridos: «`gate`/`parity`/`bench` as the required status checks» y «`master` is protected with `gate`, `parity` and `bench` as required checks». **Era falso, y hacerlo verdad sería peor**: `parity` y `bench` corren sólo por `workflow_dispatch`, y un check requerido que nunca reporta deja **cada pull request** esperando para siempre. Estado medido por el padre en la API de GitHub: `enforce_admins.enabled: true` —el bypass de admin no estaba disponible—; `required_status_checks` ausente (`contexts: null`, `checks: null`, `strict: null`); `required_pull_request_reviews` presente con 0 aprobaciones exigidas, así que el PR es obligatorio para mergear; `allow_force_pushes: false`; `allow_deletions: false`; `required_conversation_resolution: true`. El padre apaga `enforce_admins` para que exista la vía directa. Ambos documentos quedaron corregidos al contrato medido, con `gate` como único contexto que alguna vez podría requerirse.

- **L9 (slices, registro).** El cambio completo suma 433 líneas cambiadas de autoría en tres commits contando la apertura de la tarea (`6c993d6`): `a06484b` lleva solo el cambio de CI y su test de regresión, y `6e070f8` lleva la reescritura documental. Ese reparto es el registro de lo que quedó, no una promesa: cada slice se puede revisar solo.

- **L10 (polish del review nativo, cinco hallazgos).** El review aprobó el candidato `539e2bd`; este commit sólo corrige comentarios y documentación, sin cambio de comportamiento ni de semántica del workflow. R2-001 y R3-001: los comentarios de `.github/workflows/ci.yml` y de `scripts/tests/ci-workflow.sh` ya no dicen que la protección de rama nombre los jobs, sino que el nombre del job es el status context, que hoy no hay ningún check requerido y que `parity`/`bench` no pueden requerirse mientras sean dispatch-only. R2-002: el párrafo de hooks completa la idea — un hook que costara tanto como CI sería lo primero que se saltea, y por eso lo que no se puede eludir es CI, no el hook. R2-003: el comentario del paso de Rust nombra sólo lo que está en el archivo (`--workspace` y los excludes por display) y ya no receta del justfile. R2-004: la fila E0.7 dice que `bench` corre sólo por `workflow_dispatch`.
