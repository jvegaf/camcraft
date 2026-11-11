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

## Delivery

**El rescate no entra como un candidato: entra como cadena.** El primer intento fue un slice único de
5.737 líneas y el proveedor de review lo rechazó en preflight (Log L16). Desde ahí, cada candidato es un
commit que compila y deja sus tests verdes por separado.

| # | Rama | Contenido | Líneas | Review |
| --- | --- | --- | ---: | --- |
| 1 | `camcraft-sweep-c1` | system design + esta especificación | 971 | **approved** (riesgo `low`, sin lentes) |
| 2 | `camcraft-sweep-c2` | `shared` + `follow` (profile + tabs) | 1.409 | pendiente |
| 3 | `camcraft-sweep-c3` | `rings` (pocket completo) + tests de anillos | 1.218 | pendiente |
| 4 | `camcraft-sweep-c4` | esta spec: cadena, Log y tareas al día | 72 | **approved** (riesgo `low`, sin lentes) |
| 5 | `camcraft-sweep-c5` | tests de islas de `rings` | 1.069 | **salteado** (L20) |
| 6 | `camcraft-sweep-c6` | `peck` (taladrado) | 1.043 | **salteado** (L20) |

**El orden follow -> rings -> islas -> peck no es arbitrario.** `follow` es el único modulo que alcanza
todos los helpers de `shared`; empezar por `peck` dejaria `push_horizontal`, `piece_endpoints` y
`sub_segment` sin usar, y clippy falla bajo `-D warnings`. Un crate nuevo no se salva de eso con
`#[allow(dead_code)]`, y no debe: es exactamente la mancha que se limpió en el primer intento (L10).

**Gates por candidato.** Cada uno se commitea con `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings` y `cargo test --workspace` en verde. El contador de tests sube candidato a
candidato: 1697 -> 1717 (c2) -> 1731 (c3) -> ... -> 1770 con el rescate completo.

**Estado final de la cadena y de su review: ver L20-L22.** Las ramas `camcraft-sweep-cN`, `-dA/-dB/-dC`
son andamiaje de review; los commits son el entregable.

**El presupuesto de review lo pone el proveedor, no el skill.** No hay `size:exception` posible: el techo
es el contexto del revisor. Los números de arriba son el resultado de aceptar ese techo, no de
negociarlo, y por eso el criterio de aceptacion es por candidato y no por linea.

**Base.** `origin/master` ya contiene el trabajo de start-screen, asi que la rama se cortó limpia desde
`b745501` (L13).

**Topologia.**

```
origin/master (b745501)
  +-- camcraft-sweep            <- tracker, draft, no-merge
        +-- camcraft-sweep-c1   <- aprobado
        +-- camcraft-sweep-cN   <- candidatos, encadenados
```

`camcraft` y `camcraft-sweep-s1` quedan como están: la primera intacta, la segunda como red de seguridad
con el slice sin rebanar. **Los PRs no se crearon: es decisión del dueño.**

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

**L13** (decisión del dueño, entrega): **Feature Branch Chain con tracker**, y **rama limpia desde
`origin/master`** en vez de seguir sobre `camcraft`. Motivo registrado: el diff de `camcraft` contra
master traía 7.314 líneas y 32 archivos, de los cuales 16 son del slice y el resto estado viejo
(`eefe76f` está detrás de master y habría revertido 2.455 líneas). El tracker es `camcraft-sweep`
(desde `origin/master`, draft y sin merge); el slice 1 vive en `camcraft-sweep-s1` y apunta al tracker
como padre. `camcraft` no se reescribe ni se borra.

**L14** (evidencia, gates del slice 1): sobre la base limpia — `cargo fmt --all --check` OK;
`cargo clippy --workspace --all-targets -- -D warnings` OK; `cargo test --workspace` **1770 passed,
0 failed**. `cam-toolpath` con cero archivos en el diff.

**L15** (decisión, re-rebanado): el rescate se parte en candidatos de ~1.000-1.400 líneas. Orden
`follow -> rings -> islas -> peck` por una razón mecánica verificada: `follow` es el único modulo que usa
todos los helpers de `shared`, asi que va primero o clippy rompe por `pub(crate)` inalcanzable. Los tests
de islas se separan de `rings` para que el candidato de `rings` quepa sin forzar nada.

**L16** (provider stop, evidencia): el slice único de 5.737 líneas fue rechazado en `preflight` con
`lens_context_budget_exceeded`: *"the candidate's complete reviewer evidence exceeds the native context
budget, so no review authority was created"*. `mutation_performed: false`, nada que abandonar ni reparar,
y reintentar el mismo candidato no puede funcionar. **`size:exception` deja de ser una opcion: el techo es
el contexto del revisor.** El proveedor mismo indica partir en candidatos mas chicos.

**L17** (provider, evidencia): el candidato 1 devolvió `state: approved`, `action: closed`,
`risk_tier: low`, `risk_reasons: [non_executable_only]`, `lenses_required: false`,
`lineage: review-7c4d08a330fe1f89`, 2 archivos, 971 líneas, `correction_budget: 200`. Cierre terminal
directo: sin STATUS ni acknowledgement. **Leccion registrada**: el primer START fue rechazado con
`invalid_request` porque se pidieron lentes; un lente de agente exige review de riesgo medio o alto, y un
candidato solo de documentación se arranca con `{"mode":"ordinary"}` y **sin lentes**.

**L18** (provider, evidencia — c3): el refuter confirmo `R3-1` como CRITICAL `inferential`
(`rings.rs:295-326`: `raster_rows` no rechaza un inset que deja la region sin contornos, mientras
`offset_rings` si). **Verificado por el padre y rebajado en la practica:** un probe sobre 32 formas
degeneradas (rectangulos hasta 0,2 mm de alto, cuadrados de 0,5 a 3 mm, una isla que cancela su contorno
exactamente) devolvio `OffsetCollapsed` del kernel en los 32. El defecto de lectura es real; el bug vivo
no existe. Corregido igual como codigo defensivo (guarda + test del invariante), validado por el
`provider_targeted_validator` en 16 s y aprobado. **Leccion de calibracion**: el proveedor puede marcar
CRITICAL lo que es asimetria de codigo sin entrada alcanzable; conviene probar antes de creerle la
severidad.

**L19** (provider, evidencia — c4): `state: approved`, `risk_tier: low`,
`risk_reasons: [non_executable_only]`, `lenses_required: false`, 1 archivo, 72 lineas,
`lineage: review-c5a4f854435b4d03`. Cierre terminal directo, sin acknowledgement.

**L20** (decision del dueno, alcance de la review): **c5 y c6 no se revisan.** Motivo: para cuando se
tomo la decision el rescate ya estaba probado tres veces — los tests viajaron con el codigo y pasan, el
contenido es identico al slice sin rebanar (`git diff camcraft-sweep-s1 camcraft-sweep-c6 -- crates/cam-ops`
muestra solo la correccion), y la review encontro un defecto de lectura real en 3.700 lineas de copia
mecanica. c5 son tests copiados sin codigo de produccion y c6 es `drill.rs` copiado verbatim: el valor
marginal no paga 4 a 8 corridas de modelo. Queda registrado, no silenciado. Costo total de review del
rescate: 6 corridas (2 de c2, 4 de c3).

**L21** (evidencia — el borrado, y una perdida que hay que nombrar): el borrado aterrizo en cuatro
cortes verdes, no en uno: `da5ccd9` los 89 oraculos de `cam-sim`, `bde598b` el editor de estrategia de
`cam-ui`, `ef6ca70` los subcomandos de `cam-cli`, `fdb87cf` los crates. Cada uno compila porque el
anterior le quita el ultimo consumidor al siguiente; el arbol de `fdb87cf` es **identico** byte a byte al
borrado atomico que produjo el worker, verificado con `git diff`.

**Lo que se perdio de verdad:** 55 tests de `cam-ui/tests/view3d` (2.743 lineas, 7 archivos) murieron
junto con `tests/view3d/support.rs`, que construia los programas con `cam_program::program_for_snapshot` y
las fixtures con `PocketParams`/`ProfileParams`. Los tests eran de *playback, escena y overlay*, que
sobreviven como producto, pero su mobiliario era maquinaria borrada. **Consecuencia para C8: hay que
reconstruir la cobertura de preview 3D y playback, no solo la UI.** Queda escrito para que no se descubra
tarde.

**Error propio, detectado y corregido:** al re-rebanar traje `crates/cam-ui` desde el arbol del worker con
`git checkout`, que restaura archivos pero **no** borra los que no existen alli. Resultado: 8 archivos
huerfanos en disco, sin declaraciones `mod`, que ni compilaban ni corrian (`cargo test --test view3d`
daba 0 tests). Se detecto comparando el arbol re-rebanado contra el original: el diff no estaba vacio.
Leccion: al re-rebanar por checkout de paths, las eliminaciones se hacen con `git rm`, nunca se infieren.

**L22** (decision del dueno — alcance de la review del borrado): **el borrado se acepta sin review, y el
motivo queda escrito.** No es una omision: es lo que el proveedor puede tomar.

Los hechos, todos medidos: el rescate (5.762 lineas) fue rechazado en preflight con
`lens_context_budget_exceeded` dos veces, la segunda sobre el borrado atomico de 27.870 lineas. El techo
real esta entre **1.409** (c2, aprobado) y **5.762** (rechazado); no lo conocemos con mas precision.
Re-rebanar el borrado en cuatro cortes verdes bajo el maximo de 27.870 a **11.257**, y ahi se agoto: los
dos cortes grandes siguen siendo el doble del unico tamano que sabemos que rebota, y bajarlos mas exige
separar los tests de su crate en commits propios (solo `cam-toolpath/tests` son ~7.000 lineas) sin
garantia de entrar.

Lo que **si** esta revisado y aprobado, tres veces, es la parte que se escribe a mano: el rescate de
`cam-ops` (c1-c3, con una correccion CRITICAL validada) y su spec. El borrado es eliminacion, y se
verifica con un grep —"¿queda algo usandolo?"— no con un revisor.

**Control compensatorio:** se corre una verificacion independiente sobre lo que sobrevive (subcomandos del
CLI, superficies de la UI, conciliacion del conteo de tests, ausencia de stubs y de codigo muerto), porque
el commit que borra 33.000 lineas es justamente el que no tiene segunda opinion.

Nada de esto autoriza delivery: push, PRs y merge siguen siendo decision del dueno, y el push de las ramas
re-escritas sigue bloqueado por la politica de seguridad del harness.

**L23** (evidencia — PR4 no es un borrado, es C3): medido antes de intentarlo. En `cam-model` quedan
**608 lineas** de parametros de madera (263-870: `PocketStrategy`, `PocketParams`, `DrillParams`,
`ProfileParams`, `ChamferParams`, `VCarveParams`, `EngraveParams`, `FlutingParams`, `TextureParams`,
`InlayParams`, `ThreadParams`, `MouldingParams`) y **7 validadores** por operacion (~190 lineas:
`check_chamfer`, `check_v_carve`, `check_fluting`, `check_texture`, `check_inlay`, `check_thread`,
`check_moulding`). Trece archivos fuera de `cam-model` referencian `Operation::` y dieciocho referencian
`Toolpath`.

**El bloqueo estructural:** `Toolpath.operation: Operation` es un campo **no opcional**
(`cam-model/src/lib.rs:892`). Borrar las once variantes deja un enum inhabitable y `Toolpath` sin poder
construirse, y con el se caen la lista de toolpaths de la UI (que la verificacion independiente dio por
buena y cableada), el `match` de nombres de `cam-cli/src/lib.rs:1517`, `cam-app` y la derivacion de
targets de `cam-sim`. Los tres caminos posibles son una variante de relleno (un stub con migracion de
schema, que C3 borraria despues), definir ya las operaciones PCB (trabajo de C3), o sacar `Toolpath` del
modelo (que mata una superficie de UI que sobrevive). Ninguno es un borrado.

**Lo que si es cierto, y cierra el barrido:** las operaciones de madera ya son **inalcanzables**. Ningun
comando que sobrevive crea una — el subcomando `cam-cli toolpath` no existe, el `match` de `cam-cli` solo
las nombra para mostrarlas, y la UI solo **lee** `Toolpath` (cero construcciones de `Operation::` en
`cam-ui/src`). La unica forma de tener una es un documento viejo, y los `.c2d` se rechazan (A2). Son datos
de legado de solo lectura.

**Decision:** PR4 se pliega dentro de C3. El barrido termina aca: todo lo que era alcanzable esta afuera,
y lo que queda en el modelo se limpia al definir `camcraft/document@1`, una sola vez, en lugar de inventar
un relleno y migrar el schema dos veces.

**L24** (decision — topologia de entrega, y una correccion a L13): se eligio **Feature Branch Chain con
tracker** cuando parecia que la cadena no podia integrarse en pedazos. El barrido demostro lo contrario:
de los siete cortes, **solo uno rompe el producto** — el borrado (P5), que se lleva `emit` y `simulate`.
Los otros seis son aditivos o limpieza y pueden mergearse por separado, cada uno dejando el arbol verde.
Sobre una cadena lineal y mergeable el tracker no aporta nada, asi que se entrega como **PRs apilados**:
cada rama apunta a su padre y la primera apunta a `master`.

| PR | Rama | Cabeza | Que trae | Mergeable solo |
| --- | --- | --- | --- | --- |
| P1 | `camcraft-sweep-01-docs` | `e0ee403` | system design + spec del barrido | si (docs) |
| P2 | `camcraft-sweep-02-rescue` | `3f1a524` | `cam-ops`: shared, follow, rings, islas, peck (+73 tests) | si (aditivo) |
| P3 | `camcraft-sweep-03-docs` | `8300d13` | cierre de cadena, calibracion del CRITICAL, gsim-rs | si (docs) |
| P4 | `camcraft-sweep-04-oracles` | `da5ccd9` | los 89 oraculos de estrategia de `cam-sim` | si (tests) |
| P5 | `camcraft-sweep-05-deletion` | `fdb87cf` | el dominio de mecanizado general afuera | **no**: sin `emit`/`simulate` hasta C4 |
| P6 | `camcraft-sweep-06-cleanup` | `fcd83d9` | `medial` + los 7 hallazgos de review | si (borrado muerto + tests) |
| P7 | `camcraft-sweep-07-docs` | esta punta | L21-L24 y la reparacion del README | si (docs) |

Las ramas de andamiaje (`-c1`..`-c6`, `-s1`, `-dA/dB/dC`) quedan como historia local: los recibos de
review estan atados a identidades de target, no a nombres de rama, asi que no hacen falta para entregar, y
reescribir las remotas esta bloqueado por la politica de seguridad del harness.

**L25** (decision del dueno — topologia, reemplaza a L24): la entrega **no** va como siete PRs apilados.
Se consolido en una rama de integracion **`dev`**, con un commit de merge por slice (`--no-ff`), creada
desde `origin/master`.

Motivo: el proyecto lo lleva una sola persona, y siete PRs que no se pueden revisar por tamano son
ceremonia sin lector. La rama unica conserva lo unico que importaba de la cadena —las fronteras entre
slices visibles en la historia— sin el costo de siete punteros que hay que retargetear cada vez que se
mueve una base.

```
dev  (desde origin/master)
  merge P1  system design and the sweep specification
  merge P2  cam-ops, the three rescued capabilities
  merge P3  chain close, the CRITICAL calibration, gsim-rs
  merge P4  the strategy oracles of cam-sim
  merge P5  the general-machining domain removed
  merge P6  medial and the seven review findings
  merge P7  L21-L24 and the README repair
```

El arbol de `dev` es **identico** al de la punta de la cadena (verificado con `git diff`), asi que no
cambio contenido, solo como se presenta. Las siete ramas de slice y las de andamiaje quedan como historia.

`dev` es la rama de integracion de aca en adelante; `master` no se toca. Mergear `dev` a `master` sigue
siendo decision del dueno, y P5 --el merge que se lleva `emit` y `simulate`-- sigue siendo el unico que
necesita una decision consciente.
