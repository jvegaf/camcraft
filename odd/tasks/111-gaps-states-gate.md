# 111 — GAPS states gate: fail when the diagnosis loses a state or a citation

Construir el gate que hoy sólo sostiene la atención humana: un checker barato que falla cuando un
hallazgo de [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) §3 pierde su identificador, su estado
o su cita resoluble, con sus tests y cableado en el gate del repositorio.
Estado: `open` — nada implementado: no existe el checker, no hay tests, no está cableado y ningún
`S#` está satisfecho.


- `scripts/check-gaps-states.sh` (nuevo: el checker, con su `python3` embebido)
- `scripts/check-gaps-states.py` (el esbozo del verificador, adaptado)
- `scripts/tests/gaps-states.sh` (nuevo: los tests, sobre `scripts/tests/lib/common.sh`)
- `justfile` (receta `check-gaps-states`), `scripts/check.sh` (paso `== gaps ==`), `.github/workflows/ci.yml` (paso del job `gate`)
- `odd/tasks/111-gaps-states-gate.md`

## Surfaces


- `scripts/check-gaps-states.sh` (nuevo: el checker, con su `python3` embebido)
- `scripts/check-gaps-states.py` (el esbozo del verificador, adaptado)
- `scripts/tests/gaps-states.sh` (nuevo: los tests, sobre `scripts/tests/lib/common.sh`)
- `justfile` (receta `check-gaps-states`), `scripts/check.sh` (paso `== gaps ==`), `.github/workflows/ci.yml` (paso del job `gate`)
- `odd/tasks/111-gaps-states-gate.md`

## Specs

- **S1 (mandato del owner, verbatim).** En la misma decisión que creó la
  [tarea 110](110-gaps-resurvey.md), el owner eligió:

  ```text
  3. crea una tarea nueva para hacerlo
  ```

  «Hacerlo» es el **gate de regresión** que un verificador propuso al comprobar de forma independiente
  el T2 de 110, y que se rechazó dentro de 110 porque su S7 y su S9 prohíben tocar código. El
  verificador dejó un esbozo funcional de checker (`scripts/check-gaps-states.py`); esta tarea existe
  para construirlo en serio —reglas, tests, cableado— y no para volver a medir el diagnóstico. El
  entregable es el gate; el documento que vigila no se reescribe acá.

- **S2 (qué lee y qué falla por identificador).** El gate lee **sólo** el §3 de
  [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md): desde la línea que abre `## 3.` hasta la
  siguiente cabecera `## ` de ese mismo nivel (los `###` de fase no cierran la sección). Un hallazgo
  empieza en una línea `- **G<fase>.<n> — `; el resto de las menciones de un id dentro del texto no
  crea un hallazgo ni cuenta como duplicado.

  El **conjunto esperado** son los 30 identificadores que 110 dejó, enumerados en el checker y no
  derivados del documento: `G0.1`–`G0.3`, `G1.1`–`G1.4`, `G2.1`–`G2.5`, `G3.1`–`G3.7`, `G4.1`–`G4.4`,
  `G5.1`–`G5.3`, `G6.1`–`G6.4`. Derivarlo del documento haría que borrar un hallazgo entero pasara el
  gate, que es exactamente la pérdida que hay que detectar. Falla, como defecto distinto cada uno:

  1. un id del conjunto esperado que no aparece en ningún hallazgo (**missing**);
  2. un id que empieza más de un hallazgo (**duplicate**);
  3. un hallazgo cuyo id no está en el conjunto esperado (**unknown**), incluido un id nuevo que el
     barrido descubra: registrarlo obliga a actualizar el conjunto, nunca a que el gate lo ignore.

- **S3 (tres estados, exactamente uno).** Cada hallazgo declara su estado con el marcador
  `Estado: «…»` dentro de su cabecera en negrita (desde el bullet hasta el `**` que la cierra), y el
  texto entre comillas es **exactamente** uno de los tres de la tarea 110, S3:

  1. `sigue siendo cierto`
  2. `ya no aplica`
  3. `cambió de forma`

  Falla un hallazgo con **cero** estados (incluido el que ni siquiera tiene `Estado:`), con **dos o
  más** marcadores `Estado:` en la cabecera, y con un estado **que el gate no puede parsear**: un
  `Estado:` sin comillas, una etiqueta distinta de las tres (aunque venga entre comillas, por ejemplo
  «abierto»), o comillas sin cerrar. Las etiquetas de confianza **INFERIDO** y **A VERIFICAR** no
  cuentan como estado y no lo sustituyen: un hallazgo que sólo lleva una de ellas sigue teniendo cero
  estados.

- **S4 (citas que resuelven, con la línea dentro del archivo).** Cada hallazgo necesita **al menos una
  cita de ruta**: un span de código en línea (`` `…` ``) cuya parte de ruta contenga `/` y resuelva,
  desde la raíz del repositorio, a un archivo o directorio existente. Las rutas de archivo de la raíz
  (`justfile`, `Cargo.toml`) también cuentan. Un hallazgo que perdió todas sus citas falla, aunque
  conserve id y estado.

  Una **cita con línea** es un span de código con la forma `ruta:N` o `ruta:N-M`, con `/` en la ruta;
  sobre toda cita con línea el gate exige: que la ruta resuelva a un archivo, que `N` y `M` estén
  entre `1` y el número de líneas del archivo, y que `N <= M`. Falla una ruta inexistente, la línea
  `0`, una línea pasada de EOF y un rango invertido. Una ruta sin línea que no resuelve **no** se
  marca como defecto: el documento cita rutas de módulo y rutas históricas
  (`tool_geometry/tool_cutting_data/…`, `crates/cam-toolpath`) que no son archivos de este árbol, y no
  son una afirmación de `path:line`.

  Limitación declarada: un span sin `/` que no sea archivo de la raíz (`camlib.py:8504`, fuente
  upstream que el documento marca como fuera del árbol) no es una cita de repositorio y el gate no lo
  resuelve. Tampoco convierte el gate en un verificador de que la línea citada contenga el símbolo que
  el texto nombra: eso es S4 de 110 y lo sigue juzgando una persona.

  **Estado real al abrir esta tarea (no se repara acá).** En `a2591c3` el §3 conserva al menos una
  cita con línea que no resuelve desde la raíz: `tests/boolean_properties.rs:108`
  ([docs/GAPS_CAMCRAFT.md:136](../../docs/GAPS_CAMCRAFT.md)). Con esta regla la primera corrida sobre
  el documento commiteado es **roja por una cita**. Es un defecto del entregable de 110, se reporta en
  el `## Log` de esta tarea (S9) y no se corrige desde acá; el gate no se debilita para que pase.

- **S5 (no es un linter de prosa).** El gate no juzga redacción, títulos, severidad, orden de las
  frases, ortografía ni la verdad del hallazgo. Un hallazgo cuya prosa cambió por completo pero
  conserva su id del conjunto esperado, **un** estado válido y **una** cita de ruta resoluble pasa el
  gate. Tampoco compara §3 contra §2, §4 o el resumen de severidad del §8, ni valida hashes de commit,
  ni exige que la cita con línea apunte al símbolo nombrado. Su contrato es la forma verificable de
  110 S3/S4, no su contenido.

- **S6 (salida y exit codes compatibles con el gate existente).** El gate imita a
  [scripts/check-docs.sh](../../scripts/check-docs.sh): un defecto por línea y un resumen final.

  - Con defectos: escribe una línea por defecto en **stderr** con la forma `<regla>  <id>  <detalle>`
    (`missing`, `duplicate`, `unknown`, `state-count`, `state-value`, `citation`, `line`), ordenadas
    por id y luego por regla, más el resumen `N defect(s) in docs/GAPS_CAMCRAFT.md §3` en stderr, y
    termina en **1**.
  - Sin defectos: imprime `gaps states ok: 30 findings` en **stdout** y termina en **0**.
  - No hay un tercer veredicto: un archivo o una sección ilegible son un defecto (`citation` o
    `missing`), no un exit code distinto; una excepción del intérprete no es un resultado del gate.

  Con este contrato el gate y `check-docs` pueden correr en el mismo paso de CI sin parsers distintos:
  la salida propia del gate no se confunde con la de `check-docs`.

- **S7 (tests propios, con los casos negativos).** El gate necesita tests de comportamiento en
  `scripts/tests/gaps-states.sh`, siguiendo la convención que ya existe: un script de bash que fuentea
  [scripts/tests/lib/common.sh](../../scripts/tests/lib/common.sh), se construye su propio repositorio
  de descarte con `mktemp -d` (nunca escribe en el árbol real) y usa `expect_ok`/`expect_fail`/
  `expect_contains` de ese `lib`; el arnés
  [scripts/test-worktree-tooling.sh](../../scripts/test-worktree-tooling.sh) lo levanta solo porque
  recorre `scripts/tests/*.sh`. No se inventa otro framework ni se agrega una dependencia.

  El test debe probar, como mínimo y cada uno con su caso que demuestra que **falla**: id esperado
  ausente, id duplicado, id desconocido, hallazgo con cero estados, con dos estados, con estado no
  parseable, hallazgo sin ninguna cita de ruta, cita con línea que no resuelve, línea `0`, línea pasada
  de EOF y rango invertido. Además: un caso que cambia la prosa de un hallazgo conservando id, estado
  y cita y exige **exit 0**; y en todos los casos el test afirma el exit code, la línea de defecto
  exacta y el resumen, no sólo que el comando falló. Los casos usan un fixture mínimo propio, no el
  `docs/GAPS_CAMCRAFT.md` real, para que el test no se ponga rojo cuando 110 cambie el documento.

- **S8 (cableado en el gate del repositorio).** El gate entra donde ya corre `check-docs`: receta
  `check-gaps-states` en el [justfile](../../justfile) junto a `check-docs`; paso `== gaps ==` en
  [scripts/check.sh](../../scripts/check.sh) junto a `== docs ==`; y un paso en el job `gate` de
  [.github/workflows/ci.yml](../../.github/workflows/ci.yml), después del checkout y antes de instalar
  Rust o pnpm, para que sea barato. Hoy ese job **no** corre `check-docs.sh`; el paso nuevo entra en
  el mismo job con la misma visibilidad que los demás chequeos baratos, y agregar `check-docs` a CI
  queda fuera de alcance. `bash scripts/tests/ci-workflow.sh` debe seguir verde después del cambio
  porque parsea la forma del workflow.

  El gate no puede requerir red, `cargo`, `pnpm` ni un build: usa `python3` de la biblioteca estándar,
  como `check-docs.sh`, para que el paso de CI no dependa del toolchain.

- **S9 (frontera: lee, reporta, no reescribe).** El gate **sólo lee** `docs/GAPS_CAMCRAFT.md`; nunca lo
  edita, ni lo normaliza, ni corrige el defecto que encuentra. Esta tarea no toca
  `docs/GAPS_CAMCRAFT.md`, `docs/ROADMAP_CAMCRAFT.md` ni `src-tauri/`: si el gate descubre una cita
  rota o un hallazgo mal formado, lo reporta en el `## Log` y el padre decide qué tarea lo repara. El
  único cambio fuera de `scripts/` es el cableado de S8 y este documento. Tampoco se permite una
  lista de excepciones, un `docs-check: ignore` ni un umbral que haga pasar el documento real: un
  falso positivo se arregla en el documento o en la regla, con su caso de test, no silenciándolo.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Specs          | Ruta   | Commit |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | ------ |
| T1  | Implementar el checker en `scripts/check-gaps-states.sh` con el mismo shape que `check-docs.sh` (bash + `python3` embebido, sin dependencias): extraer el §3, reconocer los hallazgos por bullet, comparar contra el conjunto esperado de 30 ids, parsear `Estado: «…»` contra los tres estados, recolectar las citas de ruta y las citas con línea, resolverlas desde la raíz y emitir defectos y resumen según S6. El esbozo `scripts/check-gaps-states.py` del verificador se adapta; no se copia una regla que el documento no pueda cumplir. | S2, S3, S4, S5, S6, S9 | worker |        |
| T2  | Escribir `scripts/tests/gaps-states.sh` con el fixture de descarte y un caso por cada condición negativa de S2–S4, más el caso de prosa cambiada que debe pasar (S5); afirmar exit code, línea de defecto y resumen en cada caso, y comprobar que el test lo levanta `scripts/test-worktree-tooling.sh` sin tocar el árbol real. | S2, S3, S4, S5, S6, S7 | worker |        |
| T3  | Cablear: receta `check-gaps-states` en el `justfile` junto a `check-docs`, paso `== gaps ==` en `scripts/check.sh`, y paso del job `gate` en `.github/workflows/ci.yml` después del checkout; verificar `just check-gaps-states`, la inclusión en `./scripts/check.sh` y que `bash scripts/tests/ci-workflow.sh` sigue en `0`. | S8 | worker |        |
| T4  | Cierre y prueba roja-verde: sobre una copia del documento en `mktemp -d` (nunca el árbol), partir del documento corregido sólo en la copia, obtener el verde del gate, romper a propósito cada condición —id, estado, cita, EOF— de a una y registrar el rojo observado con el comando exacto; correr `just test-tooling` y `./scripts/check-docs.sh`, y anotar en el `## Log` el defecto real de `tests/boolean_properties.rs:108` que la corrida sobre `a2591c3` descubre, sin repararlo (S9). | S2–S9 | worker |        |

## Log

- **L1 (decisión del owner, verbatim, 2026-11-28).** En la misma decisión estructurada que abrió la
  [tarea 110](110-gaps-resurvey.md), el owner eligió:

  ```text
  3. crea una tarea nueva para hacerlo
  ```

  «Hacerlo» es el gate que un verificador propuso durante la verificación independiente del T2 de 110:
  el §3 de [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) declara su propia disciplina —tres
  estados por hallazgo, cada uno con evidencia resoluble— y nada falla cuando esa disciplina se pierde.
  El verificador dejó un esbozo de `scripts/check-gaps-states.py` y se rechazó dentro de 110 porque su
  S7 y su S9 prohíben tocar código y limitan la superficie a `docs/GAPS_CAMCRAFT.md`. Esta tarea existe
  para que el gate se construya con tests y se cablee en el gate del repositorio.

- **L2 (evidencia de 110, 2026-11-28).** El documento que el gate debe vigilar quedó en su forma actual
  con dos commits de la tarea 110:

  - `e50aa7c` re-midió el §Método y el §1: 17 crates, 57.786 líneas en `src/`, 1.395 atributos
    `#[test]`, cinco ausencias refutadas con el commit que las refutó y las cifras de 2026-05-09
    conservadas como historia fechada.
  - `a2591c3` re-midió el §2 y el §3: los **30** hallazgos quedan en el conjunto `G0.1`–`G0.3`,
    `G1.1`–`G1.4`, `G2.1`–`G2.5`, `G3.1`–`G3.7`, `G4.1`–`G4.4`, `G5.1`–`G5.3`, `G6.1`–`G6.4`; el
    commit dice que seis siguen siendo ciertos, seis cambiaron de forma y dieciocho ya no aplican, y
    que cada uno queda con las citas que su estado necesita, normalizadas para resolver desde la raíz
    del repositorio.

  Esos commits son la fuente de S2–S4: el conjunto de 30 ids, los tres estados y la regla de citas
  resolubles son la forma que el gate tiene que exigir, no una interpretación nueva de esta tarea.
