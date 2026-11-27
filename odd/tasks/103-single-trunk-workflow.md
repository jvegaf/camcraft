# 103 — Un solo tronco: `master`, integración híbrida y CI con unitarios

`dev` se mergeó a `master` y se borró: el proyecto vuelve a un tronco, y lo que antes justificaba dos ramas —el costo de medir el corpus en cada integración— pasa a decidirse en los triggers de CI. La integración deja de ser siempre PR: es híbrida por riesgo, con PR para lo riesgoso y merge local para lo chico.

## Surfaces

- `.github/workflows/ci.yml`
- `odd/roadmap/workflow.md`
- `.githooks/pre-push`
- `odd/tasks/103-single-trunk-workflow.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (un solo tronco, verbatim).** «Ahora he mergeado dev a master y he borrado dev, por lo que ahora desarrollaremos contra master.» `master` es el único tronco vivo: ningún documento, hook, script ni trigger de CI nombra `dev` ni `feat/tauri` como rama que recibe trabajo.
- **S2 (integración híbrida por riesgo, verbatim).** «Híbrido por riesgo»: los cambios riesgosos —migraciones, contratos públicos, CI, dependencias— entran por PR desde una rama corta; los chicos, con merge local y push directo a `master`. Es la vía elegida por ser el único desarrollador, y la protección de rama de `master` tiene que permitir el bypass de admin para que exista.
- **S3 (CI: unitarios por ahora, verbatim).** «es que el CI lo vamos a modificar, para que de momento solo corra los test unitarios, y ya mas adelante ajustaremos el tema de parity y bench». En un push o un PR a `master`, el CI corre los tests unitarios —vitest y las suites de Rust— y no el gate documental/claim completo.
- **S4 (parity y bench a pedido, verbatim).** «Manual a pedido»: `parity` y `bench` no se disparan en push ni en PR; corren sólo por `workflow_dispatch`.
- **S5 (los nombres de job no se renombran).** `gate`, `parity` y `bench` conservan su nombre: la protección de rama los nombra y renombrar un job lo desrequiere en silencio.
- **S6 (el motivo del desdoblamiento se muda, no se olvida).** `odd/roadmap/workflow.md` cuenta el flujo nuevo —un tronco, la regla híbrida, el release por tag, y qué corre en CI y cuándo— y deja de describir dos troncos y un PR de release.
- **S7 (la historia no se reescribe).** `odd/tasks/053-ci-two-trunks.md` y los documentos ya entregados quedan como están; esta tarea los supersede y lo deja dicho acá.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                     | Specs          | Ruta   | Commit |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | ------ |
| T1  | `.github/workflows/ci.yml`: triggers de un solo tronco (`push` y `pull_request` sobre `master`, más `workflow_dispatch`), `gate` con los tests unitarios de frontend y Rust, `parity` y `bench` sólo por `workflow_dispatch`, nombres de job intactos | S1, S3, S4, S5 | worker | —      |
| T2  | `odd/roadmap/workflow.md` reescrito al flujo de un tronco: día a día, regla híbrida, release por tag, qué corre en CI y cuándo, y por qué no hay segunda rama                                                                                             | S1, S2, S6, S7 | worker | —      |
| T3  | `.githooks/pre-push`: el comentario que habla de `dev/feat` dice lo que el hook hace hoy                                                                                                                                                                  | S1             | worker | —      |
| T4  | Este documento y su fila en `odd/tasks/INDEX.md`                                                                                                                                                                                                          | S1, S7         | worker | —      |

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
