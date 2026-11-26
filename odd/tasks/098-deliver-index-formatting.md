# 098 — `deliver-task.sh` deja el índice fuera de formato

Cerrar la entrega escribiendo la fila del índice con el mismo formato que el resto de la tabla, en vez de dejar que el `pnpm lint` del gate lo descubra después.

## Surfaces

- `scripts/deliver-task.sh`
- `scripts/tests/deliver-index-formatting.sh`
- `odd/tasks/098-deliver-index-formatting.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (pedido original, verbatim).** «arregla el script»
- **S2 (el defecto que nombra el pedido, verbatim del reporte que lo precede).** «`scripts/deliver-task.sh` reescribe las celdas de estado y commit de `odd/tasks/INDEX.md` sin el padding de la tabla, así que toda entrega deja el índice fuera de formato para Biome y `just check` falla en el paso de lint antes de llegar a los tests.»
- **S3 (alcance: sólo el índice).** El script sigue haciendo lo suyo (verificar el commit, chequear el registro, liberar el claim, escribir el índice de forma atómica); lo único que cambia es que la fila que escribe queda formateada.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                | Specs      | Ruta   | Commit    |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ------ | --------- |
| T1  | `scripts/deliver-task.sh` calcula los anchos de columna de la propia tabla (desde la fila separadora) y escribe las celdas de estado y commit con ese padding; si el valor no entra en el ancho de la columna, ensancha la columna entera —cabecera, todas las filas y la fila separadora— en vez de desbordar una sola fila; el ancho mínimo de columna que usa el formateador es 5 | S1, S2, S3 | worker | `d23e804` |
| T2  | `scripts/tests/deliver-index-formatting.sh` prueba que una entrega deja la fila en la forma padded — y que el resto de la tabla queda intacto — dentro del arnés `scripts/test-worktree-tooling.sh`                                                                                                                                                                                  | S1, S3     | worker | `d23e804` |

## Log

- **L1 (pedido original, verbatim).**

  ```
  arregla el script
  ```

  Se refiere al script nombrado en el reporte inmediatamente anterior: `scripts/deliver-task.sh`, que al marcar una tarea como entregada reescribe las celdas `estado` y `último commit` de `odd/tasks/INDEX.md` sin el padding que usa el resto de la tabla.

- **L2 (evidencia medida).** La entrega de la 097 produjo la fila `| 097 | ... | delivered | 2026-11-27 | \`9b7d202\` |`; `just check`falló en el paso de Biome con el diff de formato esperado, y hubo que realinearla a mano en`cc58dc6`. El defecto es del script, no del operador: cada entrega vuelve a dejarlo en el mismo estado.

- **L3 (decisión del padre durante la implementación).** El primer borrador de T1 decía que un valor que no entraba en su columna debía hacer fallar la entrega. Se cambió a ensanchar la columna de forma uniforme: fallar deja la entrega sin salida justo en el paso que registra el trabajo, y ensanchar es lo que haría el formateador. La detección usa `max(5, contenido más ancho + 2)` porque Biome nunca baja de 5 el ancho de una columna.
- **L4 (evidencia medida).** `bash scripts/tests/deliver-index-formatting.sh`: 22 casos, 0 fallos, con RED observado contra la revisión previa del script (fila desbordada y columna sin ensanchar). El arnés completo `scripts/test-worktree-tooling.sh` conserva el mismo conjunto de fallos que ya existía en `dev`: `deliver.sh` 11/7 y `gate.sh` 16/3, idénticos antes y después de este cambio; ninguno lo toca esta tarea. `pnpm lint` limpio.
- **L5 (deuda que queda a la vista).** `scripts/test-worktree-tooling.sh` está rojo en `dev` por dos causas ajenas a esta tarea: el repo de prueba de `lib/common.sh` no copia `scripts/check-records.sh`, así que los caminos exitosos de `deliver.sh` fallan con "task 100 is not recorded yet", y `gate.sh` muere con 127 por el mismo archivo faltante. Arreglarlo excede las superficies de la 098.
