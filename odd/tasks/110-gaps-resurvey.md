# 110 — GAPS re-survey: measure the tree, not the plan

Re-medir el árbol actual y reescribir con evidencia de hoy los hallazgos de [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md), que sigue siendo el diagnóstico del workspace **anterior al giro a PCB** y afirma cosas que ya no son ciertas; el resultado esperado es un diagnóstico otra vez válido, no un plan nuevo ni un arreglo de lo que el barrido encuentre.
Estado: `in progress` — T1 y T2 entregados y commiteados (`e50aa7c`, `a2591c3`); T3 en curso y
T4–T7 abiertos.

## Surfaces


- `docs/GAPS_CAMCRAFT.md` (§Método y §1-§9 re-medidos y re-escritos con evidencia de hoy)
- `odd/tasks/110-gaps-resurvey.md`

La fila de `odd/tasks/INDEX.md` la agrega otro writer en el mismo cambio y no se escribe desde acá
(S9).

## Specs

- **S1 (decisión del owner, verbatim).** El owner eligió, en una decisión estructurada, la opción
  **«Relevamiento nuevo como tarea ODD»**, descrita así:

  ```text
  Abrir una tarea que re-mida el árbol y reescriba los hallazgos con evidencia actual; el doc vuelve a ser diagnóstico válido.
  ```

  Consecuencia: el entregable de esta tarea es [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md)
  re-medido y re-escrito contra el árbol de hoy, y nada más. El «por qué» es el que el documento
  declara de sí mismo en su §Método (`docs/GAPS_CAMCRAFT.md:13`): se relevó «read-only sobre el
  workspace (16 crates, ≈58k líneas en `src/`, ≈124k con tests)» con fecha «2026-05-09»
  (`:18`), y desde entonces el árbol cambió sin que el documento vuelva a medirse.
- **S2 (qué se re-mide, bloque por bloque).** Cada hallazgo de los nueve bloques del documento se
  re-mide contra el árbol **actual**, no se re-lee del documento. La cobertura es completa, bloque
  por bloque:
  - §1 inventario (`:22`) — la tabla de crates con LOC de `src/`, la lista de dependencias externas
    del workspace, el conteo de `#[test]`, y las ausencias que el inventario afirma (`benches/`,
    `.github/workflows/`, corpus).
  - §2 veredicto corto (`:58`) — las cuatro filas de «Reutilizar / Adaptar / Construir / Riesgo de
    rediseño» y la frase que las resume.
  - §3 gaps por fase (`:75`) — **todos** los identificados: G0.1–G0.3, G1.1–G1.4, G2.1–G2.5,
    G3.1–G3.7, G4.1–G4.4, G5.1–G5.3, G6.1–G6.4.
  - §4 tabla de rendimiento (`:289`) — las cuatro palancas y el párrafo que las interpreta.
  - §5 riesgos arquitectónicos (`:308`) — los seis numerados.
  - §6 decisiones tomadas (`:339`) — D1–D6, cada una con el hecho que la decide, que es justo lo
    que hay que re-medir si el árbol cambió.
  - §7 lo que NO es un gap (`:400`) — los seis activos reutilizables.
  - §8 resumen de severidad (`:422`) — ver S6.
  - §9 próximo paso (`:451`) — la afirmación «El diagnóstico está cerrado y las decisiones D1–D6
    están tomadas» y la remisión al ROADMAP que la sigue: se re-mide si el diagnóstico sigue cerrado
    con los hallazgos corregidos, en vez de dejarla pasar como cierre del documento viejo.

  Re-medir es ejecutar la comprobación sobre el árbol de hoy (`grep`, `ls`, conteo sobre
  `Cargo.toml`/`src/`/`tests/`, lectura del archivo citado), no reformular el hallazgo con otras
  palabras. La muestra verificada en [L2](#log) es el piso del barrido, **no** su inventario: el
  hecho de que siete hallazgos se sepan falsos no dice nada sobre los otros.
- **S3 (tres estados, y sólo tres).** Cada hallazgo termina en exactamente uno de estos estados, con
  la evidencia al lado:
  1. **«sigue siendo cierto»** — la afirmación se sostiene hoy; la evidencia es un `path:línea` que
     muestra la ausencia o la presencia que el hallazgo afirma, o un conteo medido.
  2. **«ya no aplica»** — hay que decir **qué cambió y desde cuándo**: el commit que lo produjo
     (`7aeba69` y sus pares) o el documento de tarea que lo retiró (por ejemplo
     [039-wood-out-sweep.md](039-wood-out-sweep.md)), no «cambió el árbol».
  3. **«cambió de forma»** — la redacción actual del hallazgo y la evidencia que la sostiene: el
     problema no desapareció, cambió de tamaño, de dueño o de superficie.

  No hay una cuarta salida. El documento usa hoy **INFERIDO** y **A VERIFICAR** como etiquetas de
  confianza; se conservan, pero no sustituyen a un estado: un hallazgo que no se puede medir en este
  árbol lo declara con el motivo y la ruta que muestra por qué no se puede, y aun así cae en uno de
  los tres estados.
- **S4 (nada que un lector no pueda verificar).** Toda ruta citada existe en el árbol; toda línea
  citada cae sobre el símbolo que el texto nombra. Las rutas se citan de forma resoluble desde la
  raíz del repositorio: un `cam-ops/src/isolation.rs:638` sin prefijo no resuelve desde la raíz y el
  lector no lo puede abrir, así que el barrido normaliza esas citas (`src-tauri/crates/cam-ops/src/…`).
  Una cita que no resuelve se corrige; no se borra para que el documento quede prolijo.
- **S5 (los números se re-miden o se declaran historia).** Los números del documento —la tabla de LOC
  del §1, el «≈58k / ≈124k» del §Método (`:13`), los «≈1.687 atributos `#[test]`» (`:52`), los «142
  sitios en 41 archivos» (`:146-147`), el «596 líneas» de `boolean.rs` (`:345`)— se re-miden, y junto al
  número nuevo consta el comando con el que se midió para que otro lo repita; o bien el número queda
  **explícitamente encuadrado como historia**, con su fecha y el árbol al que pertenece (`:18`:
  2026-05-09). Lo que no se puede hacer es refrescar una cifra en silencio: un lector tiene que
  saber cuál es la medición de hoy y cuál es la del relevamiento viejo.
- **S6 (§8 se re-deriva, no se parchea).** La tabla de severidad (`:422`) se reconstruye a partir de
  los hallazgos ya corregidos, no fila por fila sobre la tabla vigente. Cada fila que queda tiene que
  explicarse por el estado que S3 le asignó a su hallazgo; un hallazgo que pasó a «ya no aplica»
  sale de la tabla, y uno que «cambió de forma» cambia de severidad o de «Bloquea» con el motivo
  escrito. La tabla nueva no puede contradecir al §3: si `G3.2` sigue BLOQUEANTE en §8, §3 tiene que
  seguir diciendo qué es lo que bloquea. Con la tabla queda la afirmación de §9 de que «El
  diagnóstico está cerrado» (`:453`): se sostiene sólo si cada fila de la tabla nueva se explica.
- **S7 (esto sigue siendo un diagnóstico: frontera).** El barrido **no** inventa decisiones de
  arquitectura: D1–D6 (§6) sólo cambian si una medición contradice el hecho que las decide, y ese
  caso entra como hallazgo, no como decisión nueva. Un gap que el barrido descubra y el documento
  todavía no nombre se registra **como hallazgo** con identificador, estado y evidencia; no se
  arregla en el lugar. No hay cambios de código, ni reescritura de
  [docs/ROADMAP_CAMCRAFT.md](../../docs/ROADMAP_CAMCRAFT.md), ni corrección de lo que el barrido
  encuentre, ni prioridades nuevas.
- **S8 (el gate de documentación, verde).** `./scripts/check-docs.sh` termina en `0`, y todos los
  enlaces internos del documento reescrito resuelven: los `../odd/tasks/NNN-*.md` que cita, el
  `../odd/roadmap/index.md` que hoy cita, con ese texto literal, en G2.1
  (`docs/GAPS_CAMCRAFT.md:160`), convertido en [`../roadmap/index.md`](../roadmap/index.md),
  `ROADMAP_CAMCRAFT.md`,
  los documentos de `docs/old/` del encabezado (`:9`), y este mismo documento de tarea.
- **S9 (superficie de esta tarea).** Los únicos archivos que esta tarea escribe son
  [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) y este documento. La fila de
  [INDEX.md](INDEX.md) la agrega otro writer en el mismo cambio y no se toca desde acá. Quedan fuera
  de alcance `docs/ROADMAP_CAMCRAFT.md`, `odd/roadmap/index.md`, `docs/old/`, `src-tauri/` y `src/`:
  si el barrido encuentra que uno de ellos también miente, se reporta, no se corrige.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | Specs          | Ruta   | Commit |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------ | ------ |
| T1  | Re-medir y reescribir el §1 inventario y el encuadre del documento: tabla de crates y LOC de `src/` (medida hoy, con el comando que la produce), lista de dependencias externas del workspace, conteo de `#[test]`, las ausencias que el inventario afirma (`benches/`, `.github/workflows/`, corpus de paridad) y la nota que ya advierte que la tabla mide antes del giro. El §Método y la «Fecha del relevamiento» dejan de prometer un estado que ya no es el de hoy: o se re-miden o quedan marcados como historia de 2026-05-09. Área: `docs/GAPS_CAMCRAFT.md`, §Método y §1. | S1, S2, S3, S5 | worker | `e50aa7c` |
| T2  | Re-medir el §2 veredicto corto y el §3 completo, hallazgo por hallazgo: G0.1–G0.3, G1.1–G1.4, G2.1–G2.5, G3.1–G3.7, G4.1–G4.4, G5.1–G5.3, G6.1–G6.4. Cada uno termina en «sigue siendo cierto», «ya no aplica» (con el commit o la tarea que lo cambió) o «cambió de forma» (con la redacción actual y su evidencia), y toda cita pasa a resolver desde la raíz del repositorio. Área: `docs/GAPS_CAMCRAFT.md`, §2 y §3. | S2, S3, S4     | worker | `a2591c3` |
| T3  | Re-medir la tabla de rendimiento del §4 —paralelismo, índice espacial, algoritmos preparados para rest/offsets incrementales, medición— y el párrafo que la interpreta, con la evidencia de cada palanca sobre el árbol de hoy. Área: `docs/GAPS_CAMCRAFT.md`, §4.                                                                                                                                                                                                                                                                    | S2, S3, S4, S5 | worker |        |
| T4  | Re-medir los seis riesgos arquitectónicos del §5 y sus citas (`cam-model`, `cam-move`, `cam-app`, `cam-geom`), asignando a cada uno uno de los tres estados. Área: `docs/GAPS_CAMCRAFT.md`, §5.                                                                                                                                                                                                                                                                                                                                      | S2, S3, S4     | worker |        |
| T5  | Re-medir el §6 decisiones (D1–D6, con el hecho decisivo de cada una) y el §7 activos reutilizables (los seis), sin agregar decisiones nuevas y sin retirar un activo porque hoy no se use: sólo se corrige lo que la medición contradice, y una contradicción entra como hallazgo. Área: `docs/GAPS_CAMCRAFT.md`, §6 y §7.                                                                                                                                                                                                               | S2, S3, S4, S7 | worker |        |
| T6  | Re-derivar el §8 resumen de severidad desde los hallazgos corregidos por T2–T5, no parchearlo fila por fila: cada fila se explica por su estado, los hallazgos que dejaron de aplicar salen y los que cambiaron de forma cambian de severidad o de «Bloquea» con el motivo escrito. Cierra además el §9: su «El diagnóstico está cerrado y las decisiones D1–D6 están tomadas» es la conclusión que el §8 y el §6 sostienen, así que sólo puede re-medirse después de T5 y T6, y no es un cierre de comprobaciones y registro como T7. Área: `docs/GAPS_CAMCRAFT.md`, §8 y §9. | S2, S6         | worker |        |
| T7  | Cierre: correr `./scripts/check-docs.sh` (debe salir en `0`), comprobar que todos los enlaces internos del documento reescrito resuelven, y registrar en el `## Log` del documento la fecha del barrido, el comando de cada medición y las citas normalizadas. No se toca nada más allá de lo que T1–T6 dejaron. Área: `docs/GAPS_CAMCRAFT.md`, `## Log`.                                                                                                                                                                          | S8, S9         | worker |        |

## Log

- **L1 (decisión del owner, verbatim, 2026-11-28).** En una decisión estructurada el owner eligió la
  opción **«Relevamiento nuevo como tarea ODD»**, y la describió así:

  ```text
  Abrir una tarea que re-mida el árbol y reescriba los hallazgos con evidencia actual; el doc vuelve a ser diagnóstico válido.
  ```

  Es la decisión que crea este documento: el relevamiento se rehace como tarea ODD, con el árbol
  actual como fuente, y el resultado es de nuevo un diagnóstico válido (S1).
- **L2 (evidencia verificada por el padre, 2026-11-28 — muestra, no inventario).** El padre verificó
  en este árbol un primer barrido de hallazgos que ya no son ciertos:

  - `src-tauri/crates/cam-gerber/` y `src-tauri/crates/cam-excellon/` existen, mientras el §8 sigue
    listando «Sin parser Gerber» (`docs/GAPS_CAMCRAFT.md:428`) y «Sin parser Excellon» (`:429`) como
    **BLOQUEANTE**.
  - `.github/workflows/ci.yml` existe, mientras G6.1 afirma «No hay CI. No existe
    `.github/workflows/`» (`:277`).
  - `src-tauri/crates/cam-ops/Cargo.toml:16` declara `rayon.workspace = true`, mientras el §4 afirma
    que el paralelismo está «Ausente | Sin `rayon` en ningún `Cargo.toml`» (`:296`) y G3.3 afirma que
    «No existe NCC» y que no hay «ninguna palanca de performance instalada» (`:209`).
  - `src-tauri/crates/cam-ops/src/ncc.rs` expone `ncc_moves` y
    `src-tauri/crates/cam-ops/src/isolation.rs` expone `isolation_moves`, alcanzables como `cam-cli
    ncc` y `cam-cli isolate` (`src-tauri/crates/cam-cli/src/lib.rs:72`, `:83`, `:239`, `:240`),
    mientras G3.1 afirma «No existe isolation. No hay operación, ni comando CLI, ni tipo» (`:192`).
  - G5.1 afirma que las capas son genéricas —«id/nombre/visible/entidades, `cam-model/src/lib.rs:208-218`»,
    «no capas tipadas con color por capa y modo cobre» (`docs/GAPS_CAMCRAFT.md:259-260`)—, mientras
    `src-tauri/crates/cam-model/src/lib.rs:270-285` declara `Layer` con `role: LayerRole` (`:274`),
    `side: Side` (`:276`) y `color: String` (`:278`): el rol de cobre y el color por capa ya están en
    el modelo, y el rango `:208-218` que el hallazgo cita ya no es donde vive `Layer`.

  Corregir estos siete casos no es el trabajo: el padre los entrega como **muestra** de que el
  diagnóstico quedó desfasado, no como lista cerrada de lo que hay que tocar. El punto de esta tarea
  es que nadie midió el resto (S2). Como parte del mismo barrido, quien escribe re-verificó en este
  árbol cada ruta citada en esta entrada —los dos crates, `ci.yml`, la línea de `rayon`, los dos
  módulos de `cam-ops` y las cuatro líneas de `cam-cli`— y contó **17** directorios en
  `src-tauri/crates/` frente a los «16 crates» que el §Método declara (`:13`): el barrido empieza
  desmintiendo ya el encuadre del documento.
- **L3 (T1 entregado, 2026-11-28, `e50aa7c`).** El §Método y el §1 quedaron re-medidos con dos fechas
  explícitas: la de 2026-05-09 como historia y la de hoy como medición vigente, cada número con su
  comando. Lo medido, tal como quedó en `docs/GAPS_CAMCRAFT.md`:
  - `find src-tauri/crates -maxdepth 1 -mindepth 1 -type d | wc -l` → **17** crates (más el paquete
    Tauri), frente a los 16 del relevamiento viejo: salieron `cam-toolpath` y `cam-program` (`fdb87cf`,
    [tarea 039](039-wood-out-sweep.md) L21) y entraron `cam-ops`, `cam-gerber` y `cam-excellon`.
  - `find src-tauri/crates/*/src -name '*.rs' -type f -exec cat {} + | wc -l` → **57.786** líneas en
    `src/`; `find src-tauri/crates -name '*.rs' -type f -exec cat {} + | wc -l` → **107.014** contando
    tests y benches.
  - `grep -rn '#\[test\]' src-tauri/crates --include='*.rs' | wc -l` → **1.395** atributos `#[test]`
    (el «≈1.687» de 2026-05-09 no mide este árbol).
  - Cinco ausencias del §1 quedaron refutadas, cada una con el commit que la refutó: **sí hay**
    `benches/` (`cfb7feb`, 2026-11-13), **sí hay** CI (`.github/workflows/ci.yml`, `65d1499`,
    2026-11-13), **sí hay** corpus de paridad (`fixtures/corpus/`, `1e7cb07`, 2026-11-13, más el arnés
    `scripts/parity/run.sh`), **sí hay** `rayon` (`Cargo.toml:58`, usado en
    `src-tauri/crates/cam-ops/Cargo.toml:16`) y `flate2` salió del workspace en `579194b` (2026-11-23);
    `criterion` para los benches entró con `cfb7feb`.
  - Tres filas de la tabla cambiaron de forma, no de estado: el conteo de subcomandos de `cam-cli`, el
    contenedor `.cmc` de `cam-store` y los formatos de `cam-import` después de que PDF/EPS/PNG se
    fueran con el barrido de madera.

  Hallazgo del barrido, registrado y no corregido (S7):
  `scripts/parity/measure/Cargo.toml:10-12` declara `path = "../../../crates/cam-*"` y esa raíz no
  existe desde la migración a `src-tauri/`.
- **L4 (T2 entregado, 2026-11-28, `a2591c3`).** Se re-midieron el §2 y los **30** hallazgos del §3,
  uno por uno, contra el árbol de hoy; cada uno cerró en uno de los tres estados de S3 con su
  evidencia, y ninguna cita quedó fuera de la raíz del repositorio. La distribución, tal como quedó en
  `docs/GAPS_CAMCRAFT.md`:
  - **6 «sigue siendo cierto»**, **6 «cambió de forma»** y **18 «ya no aplica»** —este último con el
    commit que lo refutó, no con «cambió el árbol».
  - Los dos hallazgos que más cambian el diagnóstico: **los parsers existen**. `cam-gerber` lee el
    dialecto de KiCad y evalúa la imagen de una capa sobre el motor de conjuntos (`Gerber::image`,
    `src-tauri/crates/cam-gerber/src/lib.rs:206`), y `cam-excellon` parsea tabla de herramientas,
    coordenadas y ranuras `G85` (`parse`, `src-tauri/crates/cam-excellon/src/lib.rs:146`).
  - Lo que la fecha de 2026-05-09 volvía falso por venir después: CI (`65d1499`), benches con baseline
    (`cfb7feb`, `8d0ad9d`), corpus y arnés de paridad (`1e7cb07`, `0061226`) y `rayon` en `cam-ops`
    (`8d0ad9d`), más isolation (`e91882b`), NCC (`8d0ad9d`), tabs (`487a020`) y el drill con
    ordenamiento por cercanía (`3f1a524`, `b324e3c`).
  - El §2 dejó de ser un veredicto de 2026-05-09 y dice sólo lo que los hallazgos corregidos sostienen;
    G5.1 cuenta **seis** roles de fabricación con keepout aparte
    (`src-tauri/crates/cam-model/src/lib.rs:177`, `:198`), como los cuenta el código.
  Las **ocho** observaciones que la verificación independiente levantó y este commit cierra, todas
  visibles en el diff de `a2591c3`:
  1. El §2 nombraba `cam-toolpath`, un crate que `fdb87cf` retiró, y su veredicto —«parte casi de cero
     en todo lo que hace PCB a PCB»— contradecía los hallazgos ya corregidos del §3.
  2. Las dos citas de código de FlatCAM que quedaban en §2-§3 se leían como si estuvieran en el árbol:
     hoy `camlib.py:8504` (G0.3) y `ToolPanelize.py:309` (G3.7) dicen **fuente upstream de FlatCAM
     Plus que no está en este árbol**, y la tercera (`camlib.py:3040,3104`, G3.5) salió al re-medir.
  3. Las citas de §2-§3 no resolvían desde la raíz (`crates/cam-…`, `cam-ops/src/…`): pasaron a
     `src-tauri/crates/…`.
  4. G2.1 citaba «142 sitios» sin comando y con rangos «desplazados»: `grep -rn 'EntityKind::'
     src-tauri/crates --include='*.rs' | wc -l` → **140** sitios en **41** archivos (82 `Contour`, 30
     `Drill`, 27 `Slot`).
  5. G2.3 daba `cam-model/src/lib.rs:57-88` como el contrato documento↔máquina, donde ya no viven
     `Board` (`src-tauri/crates/cam-model/src/lib.rs:117`) ni `Frame` (`:63`).
  6. G5.1 contaba el keepout entre los roles de fabricación; medido, son seis roles con keepout aparte
     (`src-tauri/crates/cam-model/src/lib.rs:177`, `:198`).
  7. G1.1 listaba como faltantes cosas que el lector ya hace —aperturas `O`, aperturas que nombran una
     macro y `D02` dentro de `G36`/`G37`— y nombraba `RoundRect` como si existiera: hoy separa lo
     medido de lo que se rechaza por nombre.
  8. Dos **A VERIFICAR** sobrevivían sin medición: ángulo y punta de V-bit quedaron verificados y en uso
     (`included_angle`, `flat_diameter` y `tip_radius` en
     `src-tauri/crates/cam-store/src/tools.rs:44-63`, derivados por `ToolShape::VBit`), y los tres
     dialectos `.pp` con golden byte a byte (G4.4) pasaron a medidos.

  Hallazgo del barrido, registrado y no corregido (S7): el §2 y el §5 no coinciden en el costo de
  `EntityKind` —«140 sitios» en el §2 y el §3 medido, «142 sitios» en el §5, que es de T4—; y el §8
  sigue listando hallazgos que el §3 ya dio por «ya no aplica» (G5.1, G2.5, G5.3), que es de T6.
