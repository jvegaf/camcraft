# wood-out-sweep

Borrar el dominio de mecanizado general ("madera") del workspace y rescatar las tres capacidades que el
producto PCB necesita. Es la primera tarea de `docs/SYSTEM_DESIGN_CAMCRAFT.md` (§2.4, decisión A5/A8) y no
depende de ningún otro hito.

## Specs

**S1 — El dominio de mecanizado general sale del producto, del binario y de la UI.**
> "solo vamos a hacer pcbs, al estilo de Flatcam, pero en rust, con mejor arquitectura ui y performace,
> asi que como dice milei, todo lo de la madera....afueraaa"

**S2 — Rescates primero, árbol verde, después borrar.**
> "Un PR de borrado no cambia comportamiento ni agrega features; si un rescate necesita una decisión de
> diseño, va al PR siguiente."

**S3 — Los tres rescates aterrizan en un `cam-ops` mínimo creado en el mismo PR 1.**
> "tres módulos públicos con sus tests; el trait `Node` llega en C4"

Origen de cada rescate (§2.2 del diseño):
> "**Anillos de offset con islas** (`pocket.rs:124-158,332-337`) → motor de isolation y NCC."
> "**Peck / retract / drill bank** (`drill.rs:85-132`) → ejecución de taladrado."
> "**Perfil con tabs** (`lib.rs:483`) → cuerpo del cutout."

**S4 — Criterio de aceptación de cada PR.**
> "los tests de las tres capacidades se mudan y quedan verdes; los tests de las 11 estrategias se borran
> con su código; `cam-ui` y `cam-cli` compilan"

Más los gates del repo: `just check` (fmt --check, clippy `-D warnings`, todos los tests) verde en cada PR.

**S5 — La blast radius real manda sobre §2.4.** Si borrar un crate rompe otro que §2.4 no nombra, ese
tercero entra al PR con su motivo escrito, no se deja el árbol rojo ni se inventa un stub. Se registra en
el Log con evidencia `path:line`.

## Tasks

| ID | Tarea | Spec | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Mapear la blast radius real (read-only) | S5 | `gentle-ai-explore` | — |
| T2 | Crear `cam-ops` con **copias** de `rings` (pocket completo), `peck` (drill) y `follow` (profile) + sus tests | S3, S4 | `gentle-ai-worker` | commit 1 |
| T3 | PR 1: borrar `cam-toolpath`, `cam-program`, el editor de estrategia de `cam-ui`, los oráculos de estrategia de `cam-sim` y los subcomandos `toolpath`/`emit`/`simulate` de `cam-cli`; árbol verde | S1, S4, S5 | inline | commit 2 |
| T4 | PR 2: borrar extras de `cam-geom` (medial, nest, nest_profile, moulding, texto) | S1, S4 | inline | PR 2 |
| T5 | PR 3: borrar PDF/EPS/PNG raster trace de `cam-import` | S1, S4 | inline | PR 3 |
| T6 | PR 4: limpiar `Operation` de madera en `cam-model`, `cutlist`/`parts`/`report` en `cam-cli`, autoría avanzada en `cam-ui` | S1, S4 | inline | PR 4 |
| T7 | Verificación independiente del barrido (riesgo item 3: contrato de CLI) | S4 | `gentle-ai-verify` | — |

## Log

**L1** (usuario, autoriza el pivote):
> "1. camcraft es el producto y campilot el legado. 2. solo vamos a hacer pcbs, al estilo de Flatcam,
> pero en rust, con mejor arquitectura ui y performace, asi que como dice milei, todo lo de la
> madera....afueraaa 3. los plazos no me importan, quiero algo de calidad, desarrollado con ODD, y que se
> pueda desarrollar en varios worktrees en paralelo. 4. totalmente confirmado, lo rediseñamos"

**L2** (usuario, autoriza la ejecución): "arranca el barrido"

**L3** (recon previo, evidencia): `git branch --show-current` = `camcraft`. El barrido se ejecuta sobre
esta rama, no sobre una nueva. `docs/SYSTEM_DESIGN_CAMCRAFT.md` está sin commitear al arrancar.

**L4** (recon previo, evidencia): `grep -rl "cam_toolpath\|cam-toolpath" crates --include=*.rs --include=*.toml`
devuelve 40 archivos en 9 crates, incluidos `cam-geom/src/moulding.rs`, `cam-move/src/lib.rs`,
`cam-post/src/lib.rs` y `cam-store/src/lib.rs`. Que un crate inferior al de toolpath lo nombre sugiere
mención en comentario, no dependencia (sería un ciclo) — **a confirmar en T1, no asumido**.

**L5** (recon previo, evidencia): `cam-program` (652 líneas) es usado por `cam-cli` y por
`cam-ui/src/chrome/ribbon.rs`, `cam-ui/src/shell/action.rs`, `cam-ui/src/view3d/shell.rs`,
`cam-ui/tests/tool_card.rs`, `cam-ui/tests/view3d/support.rs`. §2.4 no lo menciona: **hueco conocido del
plan, entra en T1**.

**L6** (recon previo, evidencia): conteos de tests por crate afectado — `cam-toolpath` 16 archivos de
test, `cam-ui` 15 (de 19.886 líneas), `cam-cli` 5 (5.293 líneas), `cam-sim` 5 (2.506 líneas),
`cam-geom` 24, `cam-model` 1.

**L7** (T1, evidencia — descarta el falso positivo más caro): **las 40 menciones a `cam-toolpath` en
`cam-geom`, `cam-move`, `cam-post`, `cam-store`, `cam-sim` y `cam-model` son todas comentarios o
docs.** Cero dependencias reales, cero `use cam_toolpath`. Evidencia: `cam-geom/Cargo.toml:9-16`,
`cam-move/Cargo.toml:9-11`, `cam-post/Cargo.toml:9-13`, `cam-store/Cargo.toml:9-17`,
`cam-sim/Cargo.toml:9-16` sin la dependencia; y `cam-sim/src/lib.rs:4` dice literalmente "Must not
depend on `cam-toolpath`". **El kernel no se toca.**

**L8** (T1, evidencia — blast radius real): dependencias reales de `cam-toolpath` son sólo
`cam-program` (`Cargo.toml:14`), `cam-cli` (`Cargo.toml:15-16`) y `cam-ui` (sólo vía `cam-program`,
`cam-ui/Cargo.toml:21`). `cam-desktop` rompe de forma transitiva por `cam-ui`. `cam-app`, `cam-view`,
`cam-export`, `cam-import` no se enteran.

**L9** (T1, evidencia — lo que §2.4 no sabía): además de las 11 estrategias mueren (a) `cam-program`
completo (652 líneas, 10 tests inline; se rescatable conceptualmente su resolución de herramienta y
`NoCuttingDataForMaterial`, pero eso es trabajo de C4, no de un PR de borrado); (b) **75 tests de
oráculo en `cam-sim`** (`oracle.rs` 23, `moulding.rs` 23, `target.rs` 24, `vcarve.rs` 5); (c) en
`cam-cli` los subcomandos `toolpath`, `emit` y `simulate`, más los caminos de `script` que los invocan.
Sobreviven en `cam-cli`: `validate`, `inspect`, `postp`, `import`, `export`, `check`, `nest`, `parts`,
`cutlist`, `report`, `tools`. `cam-sim` conserva `sweep` 14, `heightfield` 11, `frame` 3, `origin` 2.

**L10** (decisión de plan, ajuste 1): **se rescata `pocket` completo (rings **y** raster)**, no sólo los
anillos. Partir el módulo obligaría a rediseñar su API dentro de un PR de borrado (viola S2), y NCC va a
querer el método *lines* (E4.6), que es exactamente el raster. Separarlos es una decisión de C5.

**L11** (decisión de plan, ajuste 2): **los rescates se copian, no se mueven**, y el PR 1 pasa a tener
dos commits. Commit 1 crea `cam-ops` con copias + tests propios, con `cam-toolpath` intacto y verde: así
el commit 1 **prueba** la capacidad rescatada en vez de sólo trasladarla. Commit 2 borra el original y
con él la duplicación. Si se moviera, el commit 1 no demostraría nada.

**L12** (consecuencia de producto, aceptada y declarada): hasta C4 el CLI pierde `emit` y `simulate`,
es decir el repo se queda sin pipeline de G-code usable. No es silencioso: es la consecuencia directa de
que en campilot todo el pipeline cuelga de las estrategias. S2 se lee en consecuencia — un PR de borrado
no cambia el comportamiento de la superficie que **sobrevive**.
