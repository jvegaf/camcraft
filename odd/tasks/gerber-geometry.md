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

**L6** (T2 entregado, con una verificacion independiente que corrigio dos cosas): `image(&self,
tolerance) -> Result<Region, GerberError>` recorre el flujo de comandos, arranca en `dark` cuando el
archivo no declara polaridad, junta los segmentos de `G36`/`G37` en un contorno cerrado
(`Contour::from_segments(.., true)`), conserva los arcos como `Segment::Arc` y aplica la polaridad en
`RegionEnd` con `combine` (`dark` → `Union`, `clear` → `Difference`). `Combined::Empty` se vuelve una
imagen vacia y no un error. Lo que no sabe construir se rechaza con `Unsupported { line: 0, what:
"command <index>: …" }`, porque `Command` nunca llevo numero de linea y no se inventa uno.

**Lo que la verificacion encontro.** Tres cosas, en orden de utilidad:

1. **El conteo agregado del workspace si era producible**: **1287 passed, 0 failed, 3 ignored**. El
   worker habia dicho que cargo no lo emitia en una sola linea. Era falso: hay 107 suites y el total se
   cuenta sumando. Un conteo que no esta no se aprueba, se mide.
2. **La regla del arco estaba bien implementada y sin probar.** `arc_from_gerber` usa
   `(start_angle - end_angle).rem_euclid(TAU)`, o sea el recorrido **en la direccion mandada**, que en
   multi-cuadrante llega a 270 grados. Pero el unico test de arco usaba un cuarto de circulo de 90
   grados: una implementacion que tomara el camino corto pasaba igual. Se agrego
   `a_multi_quadrant_arc_sweeps_the_long_way_round`: centro (0,0), radio 10, de (0,10) a (-10,0) en
   sentido horario, o sea 270 grados. Probado por mutacion: intercambiando las dos ramas de direccion
   el test muere con `-1.5707` (90 grados) contra los 270 esperados.
3. **Tres mutaciones mas, todas muertas**: clear aplicando `Union` en vez de `Difference` (la mata el
   test del hueco), arcos aplanados a rectas (los mata el test de `Segment::Arc`), y polaridad por
   defecto `Clear` (la matan los dos tests que no declaran `LPD`).

**Un error mio, cazado por una sonda.** La primera version del test de 270 grados afirmaba
`!arc.is_ccw()` y fallo. La sonda mostro por que: el arco guardado era `start (-10,0) → end (0,10)`,
`sweep +3π/2`, `ccw true`. Es el mismo arco recorrido al reves, porque `Contour::from_segments`
**canonicaliza el contorno externo a antihorario** e invierte cada segmento. El codigo estaba bien y mi
afirmacion era la que estaba mal: la orientacion de un contorno es canonica y no dice nada sobre la
direccion que mando el archivo. El test ahora afirma la **curva**: |sweep| = 3π/2 y el punto medio en
(7.07, -7.07), que un atajo de 90 grados pondria en el lado contrario.

**Limitacion declarada:** un `Move` (D02) adentro de un `G36`/`G37` se rechaza, pero en Gerber un D02
adentro de una region **empieza otro contorno de la misma region**. El archivo real medido pasa (un solo
contorno continuo de 1673 `D01`), pero un bloque con dos contornos se rechaza hoy. Levantarlo es trabajo
aparte y se anota como tal, no se disimula.

**Estado:** workspace en **1288 passed, 0 failed, 3 ignored**; `fmt` y `clippy -D warnings` limpios.
