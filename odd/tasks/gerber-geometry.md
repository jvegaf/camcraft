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

**S6 — Un `D01` con apertura fuera de una region es un trazo, no un rechazo.** Es el barrido de la
apertura a lo largo del segmento: el material que la herramienta dejaria si arrastrara esa forma desde el
punto inicial hasta el final. Para una apertura circular de diametro `d` el trazo es una capsula —el
rectangulo de ancho `d` sobre el segmento mas un semicirculo en cada punta— y su area es
`largo * d + pi * (d/2)^2`, exacta. Para una apertura rectangular es la suma de Minkowski del rectangulo
y el segmento, que es un hexagono convexo sin arcos. Es el comando con el que Gerber dibuja **pistas**, asi
que sin esto un archivo real solo aporta pads y planos.

**S7 — Un `D02` dentro de `G36`/`G37` empieza otro contorno de la misma region.** No es un error: una
region puede tener varios contornos, y cada `D02` cierra el que venia. Hoy se rechaza por nombre.

## Tasks

| ID | Task | Specs | Route | Commit |
| -- | ---- | ----- | ----- | ------ |
| T1 | `Command::Polarity` en el flujo de comandos; `LPD`/`LPC` dejan `attributes`; cualquier otro `LP` se rechaza por nombre | S1 | delegated | — |
| T2 | Un bloque `G36`/`G37` se convierte en un `Region` (lineas y arcos, cerrado) y el acumulador aplica `dark`/`clear` via `combine`; `Flash` y trazo se rechazan por nombre | S1, S2, S3, S4 | delegated | — |
| T3 | `Flash` de apertura `C` (circulo) y `R` (rectangulo) entra al mismo acumulador | S3 | delegated | — |
| T4 | G1.1 corregida en `docs/GAPS_CAMCRAFT.md` | S5 | inline | — |
| T5 | Un `D01` con apertura `C` fuera de region se vuelve una capsula y entra al acumulador | S6 | delegated | — |
| T6 | Un `D01` con apertura `R` fuera de region se vuelve su suma de Minkowski con el segmento | S6 | delegated | — |
| T7 | Un `D02` dentro de una region cierra el contorno actual y empieza otro | S7 | delegated | — |

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

**L7** (T3 entregado, y el defecto de kernel que aparecio en el camino): un `D03` con apertura `C` es
`primitive::circle(at, d/2)`; con apertura `R` es ese rectangulo **centrado** en el punto del flash
—`primitive::rectangle` toma la esquina inferior izquierda, asi que se traslada medio ancho y medio
alto—. Las dos formas entran al mismo acumulador que un bloque de region: `dark` une, `clear` resta, y un
flash claro cava un hueco exactamente como lo hace geometria clara. `O`, `RoundRect` y las aperturas que
nombran una macro se rechazan nombrando la plantilla y el indice del comando.

**El primer intento se rechazo, y era lo correcto.** El autor original construyo el `C` como poligono de
64 lados porque `combine` rechazaba el circulo. Ese rechazo no era del primitivo: `dedup_pieces`
(`crates/cam-geom/src/boolean.rs`) agrupaba las piezas por **par de extremos**, y las dos
semicircunferencias de un circulo comparten los dos puntos de costura, asi que borraba una y dejaba un
conjunto que no cerraba. El motor tenia razon al negarse: le habian sacado media circunferencia. Arreglado
en `c3c1fb0`, con la clave llevando la direccion con el intercambio de extremos y la geometria del arco
(centro, radio, sweep con signo).

**Por que no lo habia visto nadie.** Los discos que ya probaba `boolean_operations.rs` **se intersecan**,
y al cortarse en los puntos de cruce los extremos dejan de coincidir con la costura. Una union contra
region vacia, o restarle un disco *contenido* a un rectangulo, no corta nada: la costura sobrevive y el
dedup se come un arco. Ningun caso existente pasaba por ahi; un pad redondo de Gerber lo encontro en una
llamada. Los generadores del proptest ahora incluyen un disco de dos arcos.

**Prueba de que la forma es real.** Volviendo el `C` a poligono de 64 lados, los dos tests del flash
claro mueren con el mensaje "a circular aperture is a circle, not a polygon". El test afirma que todos
los segmentos del contorno son `Arc` y que el area es exactamente pi (`100 - pi` para el material que
queda), cosas que un poligono no puede afirmar.

**Tambien se fue el atajo de imagen vacia.** `apply_region` lo tenia porque `combine(empty, forma, Union)`
fallaba; ese fallo era el defecto del kernel. Con el kernel arreglado el camino real corre siempre, y el
atajo quedo fuera en vez de quedarse como codigo muerto justificado por un bug que ya no existe.

**Estado:** workspace en **1293 passed, 0 failed, 3 ignored**; `fmt` y `clippy -D warnings` limpios.

**L8** (review de la feature, aprobada sin hallazgos): el candidato fue el rango commiteado
`8ee32fe..02f5426` —seis commits, 8 archivos, 1.028 lineas—, con base explicita y `committed-only`, para
no revisar tres fragmentos por separado. Tier `high` por la señal `process_boundary` sobre
`cam-gerber/src/lib.rs`, que es el falso positivo conocido: ese crate lee archivos con `std::fs`, no
levanta procesos. **Aprobada sin un solo hallazgo**, ni bloqueante ni informativo; la autoridad quedo
quemada.

**Lo que costo llegar al START, por si vuelve a pasar.** Tres intentos rechazados con `lenses-invalid`
antes de entender: la lista de lentes va en el orden canonico (`risk, resilience, readability,
reliability`), y pasar `risk, reliability, readability` se rechaza. Se arranco con la lente de riesgo sola
—riesgo cubre de verdad lo que se toco: entrada no confiable y algebra de conjuntos— en vez de quemar otra
autoridad adivinando. Es un defecto de herramienta, no del codigo, y queda anotado como tal.
