# gerber-geometry

Turn the Gerber reader's parsed command stream into cam-geom geometry: polarity as
semantics, `G36`/`G37` regions, aperture flashes, and the accumulator that applies
dark and clear. Feature C3 E3c of `docs/SYSTEM_DESIGN_CAMCRAFT.md`.

## Specs

**S1 — La polaridad es semantica, no texto.** El enum ya existe y hoy no se usa:
`Polarity { Dark, Clear }` (`crates/cam-gerber/src/lib.rs:79-85`), pero `Gerber` no
tiene campo de polaridad (`:136-149`) y `process_extended_block` mete el comando crudo
en `attributes` (`:498-509`). El dialecto medido trae `%LPD*%`. La regla del formato:
material **dark** suma, material **clear** resta.

**S2 — Las regiones `G36`/`G37` son regiones de verdad.** El dialecto medido del
proyecto real tiene **una** region `G36`/`G37` con **1673** comandos `D01` adentro.
Hoy se traducen a `Command::RegionBegin` / `Command::RegionEnd` (`:288-295`) y ahi se
termina: no hay geometria. Un contorno de region es una `Contour` cerrada cuyos
segmentos salen de `Command::Line` y `Command::Arc`.

**S3 — La geometria es del kernel, no del importador.** De
`docs/SYSTEM_DESIGN_CAMCRAFT.md:683-687`, verbatim: "la semantica PCB ... vive en
**metadata** ... `EntityKind` crece como mucho con `Drill` y `Slot`. Todo lo demas
sigue siendo `Region`/`Contour`". Un importador no inventa entidades: produce
`Region` y `Contour`.

**S4 — Los arcos no se aplanan.** `Segment::Arc` es de primera clase
(`crates/cam-geom/src/segment.rs:126-132`), `Contour` lo conserva
(`crates/cam-geom/src/contour.rs:229-240`) y `combine`/`normalize` lo mantienen, con
`flattened` como reporte y no como estrategia (`crates/cam-geom/src/boolean.rs:3-15`).
Un arco de Gerber entra a la region como arco.

**S5 — G1.1 hoy dice algo falso.** De `docs/GAPS_CAMCRAFT.md:98-101`, verbatim:
"- **G1.1 — No hay parser Gerber.** Cero coincidencias de `gerber|excellon|rs-274x|aperture`
en `crates/` (`grep -rniE` sobre `--include=*.rs`: vacio). El `Format` de importacion y
su deteccion cubren solo DXF/SVG/PDF/EPS/PNG (`crates/cam-import/src/lib.rs:46-55,72-80`)."
El lector existe desde `cc78d52`. La entrada se corrige para nombrar el hueco que
queda de verdad, en vez de dejar una afirmacion que un `grep` desmiente.

## Tasks

| ID | Task | Specs | Route | Commit |
| -- | ---- | ----- | ----- | ------ |
| T1 | `Command::Polarity` en el flujo de comandos; `LPD`/`LPC` dejan `attributes`; cualquier otro `LP` se rechaza por nombre | S1 | delegated | — |
| T2 | Un bloque `G36`/`G37` se convierte en un `Region` (lineas y arcos, cerrado) y el acumulador aplica `dark`/`clear` via `combine`; `Flash` y trazo se rechazan por nombre | S1, S2, S3, S4 | delegated | — |
| T3 | `Flash` de apertura `C` (circulo) y `R` (rectangulo) entra al mismo acumulador | S3 | delegated | — |
| T4 | G1.1 corregida en `docs/GAPS_CAMCRAFT.md` | S5 | inline | — |

**Fuera de alcance, declarado:** los `D01` con apertura (trazo = barrido de la
apertura a lo largo del segmento) son el caso dificil y van aparte; `O`/`RoundRect`
necesitan constructores que no existen (`crates/cam-geom/src/primitive.rs:59-95` tiene
`circle` y `rectangle`, no obround ni radio-esquinas) y `RoundRect` ademas es una macro
en el dialecto medido, o sea territorio de E3b; step&repeat tambien queda afuera.

## Log

**L1** (owner, autorizacion): la propuesta fue "C3 E3c — geometria de Gerber con
polaridad. Es el siguiente paso real: con C1 cerrado, las regiones `G36`/`G37` se
pueden unir y restar, y `LPD`/`LPC` pasan a ser `combine(A, B, Difference)`. Ahi
empieza a salir cobre de una placa de verdad." El owner respondio: "por ese orden
parece bien". El orden aceptado era E3c, despues E3b, despues los dialectos de
Excellon, despues R2-001.

**L2** (evidencia, reconocimiento read-only): los arcos no son el obstaculo — un
`Region` los guarda nativos. Los obstaculos reales son otros tres: la polaridad no se
guarda como semantica; no hay constructores de forma mas alla de `circle` y
`rectangle`; y la instanciacion de macros no existe. De ahi que T1 sea la polaridad y
no la geometria: sin estado de polaridad, un acumulador no tiene que aplicar.

**L3** (evidencia, postura de tests): el crate tiene 10 tests, todos contra un
`KICAD_EXCERPT` sintetico embebido (`crates/cam-gerber/tests/gerber.rs:6-47`). No hay
fixture de un archivo KiCad real en el repo: el real vive en `data/`, gitignoreado. Las
reglas que se agreguen aca se fijan con fixtures sinteticos propios, con parametros
elegidos a proposito.

**L4** (decision, el plan se reordena): la separacion original entre T2 (region) y T4
(acumulador) no se sostiene. Una vez que un bloque se vuelve un `Region`, el acumulador
que aplica `dark`/`clear` son veinte lineas: dejarlo para una tarea aparte habria
significado exponer un intermedio que no aplica polaridad todavia, o sea una API a medio
hacer a proposito. Se fusionan en T2, y T2 ademas **rechaza por nombre** lo que todavia
no sabe construir (`Flash`, trazo con apertura) en vez de devolver una imagen incompleta
como si fuera la placa. T3 levanta el rechazo de `Flash` agregando la forma al mismo
acumulador.

**L5** (T1 entregado, con una correccion sobre mi propio test): las tres puertas verdes
(`fmt`, `clippy -D warnings`, suite) y el workspace en **1283 passed, 0 failed, 3
ignored**. La prueba de que los tests muerden se hizo revirtiendo solo la rama `LP` a su
comportamiento viejo: fallan **los tres** —`parses_clear_polarity_as_a_command`
(`left: None`), `parses_format_units_first_command_and_dark_polarity`
(`left: Some(SelectAperture(10))`), y `refuses_a_polarity_it_does_not_implement`
(`Ok` con `%LPI*%` guardado en `attributes`)— y vuelven a verde al restaurar. Un detalle
que valia la pena: el fixture que escribi para el test de polaridad clara traia
`X0Y0D02*`, un `D02` sin apertura seleccionada, que el lector rechazaba con
`NoApertureSelected { line: 4 }`. El test habria estado midiendo otra cosa. La linea se
saco del fixture; el lector tenia razon y el fixture no.
