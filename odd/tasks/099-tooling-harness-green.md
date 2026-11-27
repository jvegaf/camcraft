# 099 — Poner verde el arnés del tooling de worktrees

El arnés `scripts/test-worktree-tooling.sh` está rojo en `dev` por huecos del fixture, no por los scripts que dice probar: los caminos exitosos de `deliver.sh` y de `gate.sh` dependen de `scripts/check-records.sh` y de un commit que el fixture nunca provee. Esta tarea cierra esos huecos sin tocar los scripts bajo prueba ni debilitar un solo caso.

## Surfaces

- `scripts/tests/lib/common.sh`
- `scripts/tests/deliver.sh`
- `scripts/tests/gate.sh`
- `scripts/tests/claim.sh`
- `scripts/tests/deliver-index-formatting.sh`
- `justfile`
- `odd/tasks/099-tooling-harness-green.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «me parece bien, arreglalo», sobre la deuda reportada al cerrar la 098: «`scripts/test-worktree-tooling.sh` **ya estaba rojo en `dev`** y sigue igual: `deliver.sh` 11/7 y `gate.sh` 16/3, porque el repo de prueba de `lib/common.sh` no copia `scripts/check-records.sh` (de ahí el `exit 127`).»
- **S2 (criterio de éxito, verbatim).** El arnés completo tiene que terminar en verde: «Arreglarlo excede las superficies de la 098» → ahora está dentro del alcance, y el resultado es `bash scripts/test-worktree-tooling.sh` con todos los casos pasando y exit 0.
- **S3 (no debilitar el arnés).** Los casos existentes no se borran, no se saltean y no se vuelven vacuos: el arreglo es del fixture y de los datos que los tests usan, nunca de la exigencia. Un `skip` nuevo sólo vale para un binario externo ausente, como ya hace `deliver.sh` con `deliver-task.sh`.
- **S4 (no tocar lo que se prueba).** Si el arreglo exigiera cambiar `claim-task.sh`, `release-task.sh`, `check-worktree.sh`, `check.sh`, `check-docs.sh`, `check-records.sh` o `deliver-task.sh`, la tarea se detiene y lo reporta: esos son los sujetos del arnés, no su andamiaje.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                     | Specs      | Ruta   | Commit    |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | --------- |
| T1  | `build_repo` completa el fixture: copia `scripts/check-records.sh`, agrega un stub `engram` en `$WORK/bin` que responde lo que el chequeo de registros pregunta, y expone el commit real del repo de prueba como `FIXTURE_COMMIT` para que el chequeo de «el commit existe» tenga algo que resolver                       | S1, S2, S3 | worker | `b6017f5` |
| T2  | `deliver.sh` usa `$FIXTURE_COMMIT` en lugar del hash inexistente `abc1234`; `gate.sh` pasa el chequeo de registros con el fixture completo, y los caminos exitosos de ambos llegan a exit 0                                                                                                                               | S1, S2, S3 | worker | `b6017f5` |
| T3  | Receta `just test-tooling` para que el arnés se pueda correr sin leer el repositorio: su invisibilidad es parte de por qué se quedó rojo                                                                                                                                                                                  | S1, S3     | worker | `b6017f5` |
| T4  | Este documento y su fila en `odd/tasks/INDEX.md`                                                                                                                                                                                                                                                                          | S1, S2     | worker |           | —, este documento (`git log -- odd/tasks/099-tooling-harness-green.md`) |
| T5  | `deliver-task.sh` resuelve `check-records.sh` desde la raíz del repo y no desde el cwd del que lo llama, así que entregar desde un subdirectorio del árbol principal deja de romperse: es el defecto real que el caso «deliver works from a subdirectory of the primary tree» estaba detectando bajo el hueco del fixture | S1, S2     | worker | `65bc234` |

## Log

- **L1 (pedido original, verbatim).**

  ```
  me parece bien, arreglalo
  ```

  Responde al reporte de cierre de la 098, que dejó el arnés como deuda explícita.

- **L2 (evidencia medida del padre, 2026-11-27).** `bash scripts/tests/deliver.sh` → 11 pasan, 7 fallan; los siete son los caminos exitosos: «deliver works from a subdirectory of the primary tree (expected exit 0, got 1)», «the row is delivered», «the row records the commit (no `abc1234`)», «the claim is released» y los tres equivalentes de «deliver without a commit». `bash scripts/tests/gate.sh` → 16 pasan, 3 fallan con exit 127: «the gate passes in a claimed worktree», «on a clean primary tree» y «again once the tree is clean». `claim.sh` pasa 32 de 32. El mismo conjunto se midió en `dev` antes y después del cambio de la 098: es preexistente y estable, no intermitente.
- **L3 (causa raíz, verificada en el código).** `lib/common.sh:build_repo` copia `claim-task`, `release-task`, `check-worktree`, `check`, `check-docs` y `deliver-task`, pero no `check-records.sh`. `deliver-task.sh` lo invoca con `--engram-required` antes de escribir el índice, así que todo camino exitoso muere ahí; `check.sh` lo invoca en el paso `== records ==`, de ahí el 127 de `gate.sh`. El fixture sí inicializa un repo git real (`git init` + commit `init`), pero `deliver.sh` le pasa el hash de fantasía `abc1234`, que el chequeo de «el commit existe» rechaza antes de llegar al índice.
- **L4 (decisión de alcance del padre).** El arnés no se engancha a `just check` en esta tarea: `check.sh` se copia al fixture, así que un arnés dentro del gate se llamaría a sí mismo dentro del repositorio de prueba y hay que resolver esa recursión con cuidado. Queda como decisión aparte, anotada al cerrar.

- **L5 (defecto real encontrado al completar el fixture, y decisión del padre).** Con el fixture completo, el caso «deliver works from a subdirectory of the primary tree» no era un hueco: `deliver-task.sh` invoca `./scripts/check-records.sh` relativo al cwd del que llama, así que desde `$MAIN/odd` busca `$MAIN/odd/scripts/check-records.sh` y muere con «No such file or directory». Antes pasaba porque el script no validaba nada de eso. El primer alcance de esta tarea prohibía tocar `deliver-task.sh` (S4) justamente para no maquillar los sujetos del arnés; el padre levanta esa prohibición **sólo para esa ruta de una línea**, porque dejarla rota significa que la entrega desde un subdirectorio está rota para una persona también. El resto de S4 sigue en pie: los otros cinco scripts siguen fuera de alcance.
- **L6 (prohibición explícita del padre).** El arreglo no puede ser plantar un `odd/scripts/check-records.sh` falso en el fixture: eso haría pasar el caso escondiendo el defecto, que es exactamente lo que S3 prohíbe.

- **L7 (evidencia medida al cerrar).** Arnés completo en verde: `claim.sh` 32/0, `deliver-index-formatting.sh` 22/0, `deliver.sh` 18/0 (era 11/7), `gate.sh` 19/0 (era 16/3) — 91 casos, exit 0. `just test-tooling` corre lo mismo y sale 0. El arreglo del script es una línea (`./scripts/check-records.sh` → `"$TOPLEVEL/scripts/check-records.sh"`), y el resto de las rutas de `deliver-task.sh` se revisó: índice, claim dir y `rm -f` ya se construían desde `$MAIN`. Ninguna aserción se borró, se salteó ni se debilitó: lo que cambió en `deliver.sh` es el dato (`$FIXTURE_COMMIT` en lugar de `abc1234`), no la exigencia.
- **L8 (lo que el fixture tuvo que completar).** `check-records.sh` exige, para tareas ≥53, los bloques `## Specs`, `## Tasks` y `## Log`; los documentos del fixture sólo tenían `# NN — slug` y, en algunos, `## Surfaces`. Se agregaron esos bloques a los ocho documentos y un `## Surfaces` vacío a 103/104, que conserva el fallback a tokens de crate que los 32 casos de `claim.sh` verifican. El stub de `engram` responde sólo las dos llamadas que el chequeo hace (`ps` → `running`, `search` → `Found 1 memory`).
