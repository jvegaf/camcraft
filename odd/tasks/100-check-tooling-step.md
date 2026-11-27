# 100 — El arnés del tooling entra al gate

`just check` corre `scripts/test-worktree-tooling.sh`, con la recursión resuelta de forma explícita: un gate anidado dentro del fixture no vuelve a correr el arnés.

## Surfaces

- `scripts/check.sh`
- `scripts/test-worktree-tooling.sh`
- `scripts/tests/gate.sh`
- `odd/tasks/100-check-tooling-step.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «si hazlo», sobre la decisión que quedó abierta al cerrar la 099: «**Decisión que queda abierta, anotada en el documento:** enganchar el arnés a `just check`. No lo hice porque `check.sh` se copia al fixture y el gate se correría a sí mismo dentro del repo de prueba — hay que resolver esa recursión antes.»
- **S2 (la recursión se resuelve, no se esconde).** El gate real corre el arnés. Un `check.sh` que se ejecuta dentro de un fixture nunca lo vuelve a correr, y el motivo queda impreso: un `skip` visible, no un silencio.
- **S3 (la rama nueva está probada).** `gate.sh` gana los casos que prueban las dos ramas — el arnés presente se corre, el guard lo saltea — sobre un arnés de mentira que el fixture planta, no sobre el real, para no duplicar el tiempo de la suite.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                      | Specs  | Ruta   | Commit |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------ | ------ |
| T1  | `scripts/check.sh` gana un paso `== tooling ==` que corre `./scripts/test-worktree-tooling.sh` cuando el archivo existe y no hay guard; `scripts/test-worktree-tooling.sh` exporta `GENTLE_TOOLING_TESTS=0` para que cualquier gate anidado lo saltee. El mensaje del salteo dice por qué, como los otros `skip:` del gate | S1, S2 | worker |        |
| T2  | `scripts/tests/gate.sh`: con un arnés de mentira en el fixture, el paso se corre y el gate lo reporta; con `GENTLE_TOOLING_TESTS=0`, el paso se saltea y el gate sigue en 0. Los casos existentes no cambian                                                                                                               | S1, S3 | worker |        |
| T3  | Este documento y su fila en `odd/tasks/INDEX.md`                                                                                                                                                                                                                                                                           | S1, S3 | worker |        |

## Log

- **L1 (pedido original, verbatim).**

  ```
  si hazlo
  ```

  Responde al cierre de la 099, que dejó la decisión de enganchar el arnés a `just check` pendiente y sin resolver la recursión.

- **L2 (por qué la recursión existe).** `scripts/tests/lib/common.sh:build_repo` copia `check.sh` al repositorio de prueba. Si `check.sh` corre el arnés y el arnés construye repos de prueba que corren `check.sh`, la profundidad no tiene techo. Copiar el arnés al fixture tampoco ayuda: sólo mueve el anillo un nivel más adentro.
- **L3 (forma decidida por el padre).** Dos guardas, ambas visibles: (1) el paso sólo corre si `scripts/test-worktree-tooling.sh` existe en la raíz del repo que se está chequeando, así que el fixture —que copia una lista explícita de scripts y no la arnés— no lo corre; (2) el propio arnés exporta `GENTLE_TOOLING_TESTS=0`, así que cualquier gate que se ejecute adentro de un repositorio de prueba saltea el paso aunque el archivo esté. Ninguna de las dos es silenciosa: el salteo imprime su motivo, en el mismo estilo que los `skip:` que ya tiene el gate para `pnpm` y `cargo`.
- **L4 (por qué el fixture no se toca en su lista de copias).** Que el fixture no copie el arnés es el comportamiento que hoy lo mantiene terminable; convertirlo en «copiar todo `scripts/`» sería un cambio de fixture mucho más grande y con efectos en los otros tres archivos de la suite. El paso nuevo se prueba plantando un arnés de mentira en el caso que lo necesita.
