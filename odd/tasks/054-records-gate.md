# 054 — Gate de registros (ODD + Engram)

Que no se pueda dar una tarea por completada sin comprobar que quedó registrada: documento de tarea completo, fila en el INDEX, commit que existe, y una memoria en Engram.

## Surfaces

- `scripts/check-records.sh`
- `scripts/check.sh`
- `scripts/deliver-task.sh`
- `justfile`
- `AGENTS.md`
- `odd/roadmap/`
- `odd/tasks/`

## Specs

- **S1 (pedido, verbatim).** «mete un gate para que no se permita dar por completada cualquier cosa sin haber comprobado que queda registrado tanto en odd como en engram»
- **S2 (criterio).** Marcar `delivered` una tarea falla —sin escribir nada en el INDEX— si: falta el documento que enlaza la fila, al documento le falta `## Surfaces`/`## Specs`/`## Tasks`/`## Log`, el documento no tiene entrada de Log, el commit indicado no existe, o no hay ninguna memoria de Engram que nombre el slug de la tarea.
- **S3 (alcance honesto).** El chequeo estructural y el de memoria rigen desde la tarea 053: los documentos anteriores no tienen `## Surfaces` ni `## Log` y fallarlos sería fallar la historia. Esos casos se informan como aviso visible, nunca en silencio.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | `scripts/check-records.sh`: documento, secciones, Log, commit resoluble, memoria en Engram; `--task N` y `--engram-required` | S1, S2, S3 | inline | — |
| T2 | Enganchar al gate (`== records ==`) y a `just records` | S1 | inline | — |
| T3 | `deliver-task.sh`: negarse a marcar `delivered` sin registro, y a un commit inexistente | S2 | inline | — |
| T4 | Verificar en rojo y en verde: sección ausente, commit inventado, tarea entregada sin memoria | S2 | inline | — |

## Log

- **L1 (pedido, verbatim).** «mete un gate para que no se permita dar por completada cualquier cosa sin haber comprobado que queda registrado tanto en odd como en engram»
- **L2 (diseño).** Tres piezas: un verificador (`scripts/check-records.sh`), su lugar en el gate (`== records ==`, antes de fmt porque es lo más barato), y **la negativa en el punto exacto en que algo se llama terminado**: `scripts/deliver-task.sh` corre el verificador con `--engram-required` antes de escribir el INDEX. El corte importa: `check.sh` corre sin `--engram-required` porque CI no tiene demonio de memoria, y un check que no puede verificar no debe fingir que verificó.
- **L3 (decisiones de alcance).** (a) El corte es por número de tarea (53), no por fecha: la convención de `## Surfaces`/`## Log` no fue retroactiva, y fallar 30 documentos históricos no arregla nada. (b) Los entregados antes del corte se listan como **ADVISORY** si no tienen memoria, para que el hueco se vea. (c) `--task N` exige estructura y memoria **siempre**, sin importar el corte: el corte gobierna el barrido, no una entrega que se está decidiendo ahora. (d) En un clon shallow el chequeo de commit se informa como no verificado en vez de fallar: no se puede probar lo que no está.
- **L4 (verificación observada).** En rojo: quitar `## Specs` de un documento → falla; fila apuntando a `deadbee` → falla; `--task 049 --engram-required` → falla porque W2c se entregó sin memoria. En verde: el barrido completo pasa con 53 tareas y 0 fallos, más avisos de historia (011, 013, 019, 031, 047, 049, 050 sin memoria).
- **L5 (lo que el gate ya encontró).** Dos tareas entregadas hoy sin registro en Engram (049 W2c, 050 W2a): el aviso las deja visibles y quedan para rellenar cuando se toquen. `deliver 049` hoy se niega, que es exactamente lo buscado.
