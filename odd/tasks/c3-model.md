# c3-model

Definir `camcraft/document@1`: el modelo de placa. Absorbe lo que el barrido no podía llevarse (L23 de
`wood-out-sweep`) y es el primer hito de C3 del `docs/SYSTEM_DESIGN_CAMCRAFT.md`.

## Specs

**S1 — Se desarrolla contra `dev`; `master` queda quieto.**
> "vamos a dejar master de lado, desarrollaremos contra dev. continua con C3"

**S2 — El schema es nativo de PCB, no aditivo.**
> `schema: "camcraft/document@1"`
> `board: { width, height, thickness, origin, side_up }`
> `layers: [ { id, role: LayerRole, side, color, visible, entities, source: Provenance } ]`
> `role ∈ { CopperTop, CopperBottom, Mask, Silk, Outline, Drill, Keepout }`

**S3 — Un `.c2d` se rechaza, no se migra (A2 del diseño).**
> "Extensión `.cmc`. Un `.c2d` de campilot se rechaza como \"schema desconocido\", no se migra."

**S4 — La limpieza del modelo entra acá, no como PR aparte (L23).**
Los 608 líneas de parámetros de mecanizado (263–870), sus 7 validadores por operación y las once
variantes de `Operation` salen cuando el modelo de placa entra, no antes: `Toolpath.operation` es un campo
no opcional y borrarlo solo deja un enum inhabitable.

**S5 — Nada de lo que sobrevive pierde comportamiento.**

**S6 — Los importadores de PCB entran en C3, y un dialecto que no se entiende se rechaza nombrandolo.**
Un archivo real de placa trae el cobre en Gerber y los agujeros en Excellon, asi que ambos lectores son
parte del modelo y no un anexo. Lo que un lector no sabe leer —una polaridad, un formato de digitos, un
modo de coordenadas— se diagnostica con su linea; nunca se aproxima, porque una escala adivinada pone el
talon en el lugar equivocado.
`Job`, `Frame`, `OriginCorner`, `Layer`, `Entity`, el nesting con sus validadores, `cam-store` (contenedor
ZIP, escritura atómica, librería SQLite) y las superficies del CLI y la UI que la verificación
independiente dio por buenas.

## Tasks

| ID | Tarea | Spec | Commit |
| --- | --- | --- | --- |
| M1 | Tag `camcraft/document@1`, `LayerRole` en `Layer`, y rechazo de los schemas `c2d/*` sin migración | S2, S3 | — |
| M2 | Variantes `Operation` de PCB con sus params; salen las once de mecanizado y sus 7 validadores | S2, S4 | — |
| M3 | Reparar consumidores: `cam-cli` (match de nombres), `cam-sim/target.rs`, `cam-app`, `cam-ui` | S5 | — |
| M4 | `cam-store`: extensión `.cmc`, validación de load-time con roles, round-trip | S2, S3 | — |
| E1 | Lector de Excellon: tabla de herramientas, agujeros y slots | S6 | `a6e4d21` |
| E2 | Dialectos de Excellon: formato de dígitos (`;FILE_FORMAT=i:d`, patrón de ceros) y modo relativo `G91` | S6 | `533dddc`, `d26427b` |

## Log

**L1** (usuario, autoriza el hito y fija la rama):
> "vamos a dejar master de lado, desarrollaremos contra dev. continua con C3"

**L2** (estado de partida, medido): `cam-model/src/lib.rs` tiene 2.424 líneas. Los parámetros de
mecanizado ocupan 263–870 (`PocketStrategy`, `PocketParams`, `DrillParams`, `ProfileParams`,
`ChamferParams`, `VCarveParams`, `EngraveParams`, `FlutingParams`, `TextureParams`, `InlayParams`,
`ThreadParams`, `MouldingProfilePoint`, `MouldingParams`), y hay 7 validadores por operación
(`check_chamfer`, `check_v_carve`, `check_fluting`, `check_texture`, `check_inlay`, `check_thread`,
`check_moulding`). `DOCUMENT_SCHEMA = "c2d/document@2"` con `LEGACY_DOCUMENT_SCHEMAS = ["c2d/document@1"]`
y un `migrate()` que reescribe el tag.

**L3** (alcance de M1): el tag cambia a `camcraft/document@1`; `LEGACY_DOCUMENT_SCHEMAS` y `migrate()`
salen, porque migrar es exactamente lo que A2 prohíbe. `Layer` gana `role`, con un rol `Geometry` además de
los siete del diseño: A1 conserva DXF/SVG como geometría general, y una capa importada de un DXF no es
cobre ni máscara.

**L4** (evidencia — dos veces el mismo error, y es mio): la lista de superficies editables de M1 dejo afuera
`crates/cam-import/src/document.rs`, `crates/cam-view/tests/` y `crates/cam-cli/src/pieces.rs`, que tambien
construyen `Layer { }`. El worker las reporto como bloqueo en vez de salirse de su carril, que es lo
correcto, y las cerrei yo. Es el segundo caso igual: con el barrido paso lo mismo con `cam-desktop`. La
leccion: al agregar un campo requerido a un tipo del modelo, la superficie no es "las crates del modelo",
es **todo literal del tipo en el workspace**, y eso se mide con un grep antes de delegar.

**L5** (evidencia — los goldens): cuatro tests comparaban bytes contra fixtures escritas con el tag viejo
o sin `role`: `a_document_written_before_nesting_loads_and_re_saves_byte_identically` (fixture JSON),
`the_archive_holds_human_readable_json`, y dos aserciones de `\"schema\":\"c2d/document@2\"`. Se
actualizaron las fixtures, no se relajo ninguna asercion. Las referencias a `c2d/` que quedan en tests son
**casos de rechazo** y se conservan a proposito.

**L6** (el review de M1, y lo que encontro): el review de los cuatro lentes cerro **escalado** con
`cause: unknown_causality` y el hallazgo `R3-legacy-schema-refusal-bypassed`. El proveedor paro con
`native_stop_required` y no ofrecio ruta de correccion: es una decision del mantenedor, informativa.

Investigado a mano, el hallazgo es **real y preexistente**, y por eso la causalidad era desconocida.
`DocumentFile::to_bytes()` es publica y **no validaba**, mientras `DocumentFile::load`, `save` y
`save_document` si. Y como los campos de `DocumentFile` son publicos, el camino era: cargar un archivo
valido, cambiar `document.schema`, y pedir los bytes. Eso produce un archivo `c2d/` que ninguna puerta
validada habria escrito. La regla ".c2d se rechaza" de M1 era cierta en tres puertas y falsa en la cuarta.

Arreglado: `to_bytes` valida, con test. Aparecio un efecto secundario que vale registrar: el test de la UI
`a_refused_open_changes_nothing_and_says_why` **fabricaba su archivo corrupto con `to_bytes`**, o sea se
apoyaba en la puerta sin guardia. Ahora lo escribe a mano con `zip` (dev-dependencies de `cam-ui`:
`zip` y `serde_json`, ambos ya del workspace), que es el patron que ya usaba `cam-store` para su test de
schema legacy.

**Leccion:** una regla que se aplica en N−1 de N caminos de escritura no esta aplicada. Y un test que
necesita un artefacto invalido no debe pedirselo prestado al camino de escritura de produccion.

**L7** (evidencia — el proyecto real, y por que NO es un golden emparejado): el `775-noise-filter` da
parametros reales (dos cambios respecto de los defaults: `tools_ncc_method` 1→0 y `tools_ncc_overlap`
40→60.0) y tres programas que FlatCAM emitio de verdad. Pero **los programas no corresponden a los
parametros guardados**:

- el cutout emitido desciende a **Z = -1.9** y lo hace en **19 niveles de 0.1 mm**, mientras el objeto
  guardado dice `tools_cutout_z = -1.8` y `tools_cutout_depthperpass = 0.6` (o sea 3 pasadas de 0.6, no 19
  de 0.1);
- las fechas lo confirman: los `.nc` son de 04:06, 04:59 y 06:31, y el proyecto se guardo a las 06:31 —
  el usuario siguio editando despues de generar.

**Consecuencia:** los tres `.nc` sirven como evidencia de **forma** (una placa de 42.30 x 42.30 mm, 263
puntos distintos de contorno, el contorno partido en 6 tramos por pasada) y como muestra de tamaño real
(1.132 / 1.717 / 4.996 movimientos de corte). **No** sirven como golden con entradas conocidas: no sabemos
que parametros produjeron cada programa. Un fixture de aceptacion tiene que generarse con parametros
fijados a proposito, no heredarse de un proyecto que derivo.

**Y el dato de dominio que si vale:** el usuario no aislo, hizo **NCC**; y el taladro no lo taladro, lo
fresco como geometria. El primer caso real de este producto es limpieza de no-cobre, no isolation routing.

**L8** (evidencia — el modelo PCB, y un campo que quedo sin juez): M2 reemplazo las once variantes de
mecanizado por las cinco de PCB, con sus cinco validadores, y M3 repario los consumidores. El rescate de
`cam-ops`, que importaba sus parametros de `cam-model`, gano su propio vocabulario en
`crates/cam-ops/src/params.rs` — siete tipos de mecanizado **sin serde**, porque no se persisten: el
documento guarda `IsolationParams`, y un adaptador (C5) traducira uno al otro.

**Lo que salio mal y se corrigio:** `cam-model/tests/document.rs` se reescribio de 104 tests a 4. El arbol
quedaba verde con el modelo practicamente sin red. Restaurados 66, entre ellos la garantia que introdujo M1
(un tag `c2d/` se rechaza nombrandolo), los rechazos de schema, los ids duplicados, la geometria abierta,
las dimensiones de job, la deserializacion de geometria invalida, la identidad byte a byte del documento
pre-nesting, y los 46 de nesting. Mas tests para los tres validadores nuevos que no tenian ninguno
(NCC, cutout, paint).

**El hueco que queda abierto:** `Toolpath.island_ids` **sobrevive** — se serializa, el informe lo imprime
(`cam-cli/src/printsheet.rs:249`) y un comando de `cam-app` lo escribe (`command.rs:734`) — pero su
validador (`check_islands`) se borro, asi que **nada comprueba que una isla exista, este dentro de su
region, no se superponga con otra ni sea cerrada**. Nueve tests de islas de HEAD quedan borrados porque su
sujeto ya no existe.

**Decision pendiente (del dueno), con mi recomendacion:** el campo deberia irse. Un keep-out ya se expresa
como capa con rol `LayerRole::Keepout`, asi que "una operacion nombra islas" es una segunda manera de decir
lo mismo, y hoy es una manera que nadie valida. Sacarlo antes del release es gratis; agregarlo despues es
una migracion, y `camcraft/document@1` no va a tener migraciones. La alternativa es restaurar el validador
y darle un uso real a las islas — pero ningun parametro de las cinco operaciones PCB las usa hoy.

**L9** (decision del dueno — alcance de la review de M2): M2 se acepta **sin review**, salvo `cam-ops`.
No es una omision: es otra vez lo que el proveedor puede tomar.

Medido: el commit del swap son **5.468 lineas** y fue rechazado en preflight con
`lens_context_budget_exceeded`, sin autoridad creada. Se re-rebabano en dos: `cam-ops` (**141**) va a
review, y el resto (**5.327**) se acepta. El techo empirico queda entre **1.409** (aprobado) y **4.365**
(rechazado) y no se conoce con mas precision.

**Por que el swap no se puede partir mas sin mentir:** el modelo, sus consumidores y los tests de esos
consumidores tienen que ir juntos o el arbol no compila ni corre. Bajarlo exigiria commitear un archivo de
tests escrito para el commit y no para el producto, que es justo el artefacto que este proyecto rechaza.

**Lo que queda sin ojo externo, dicho sin adorno:** los cinco validadores nuevos (reglas de rango y
positividad, cubiertas por tests que escribio el delegado y verifique el padre), la reparacion de
consumidores, y la cobertura restaurada. Lo que si va a review es la decision de arquitectura: dos
vocabularios con una frontera.

**Consecuencia para planificar:** el techo ronda las 5.000 lineas y este proyecto produce cambios de ese
tamano por naturaleza. M4, C4 y C5 van a chocar igual, asi que conviene plantear cada tarea en cortes de
~2.000 lineas desde el principio en vez de descubrirlo al final.

**L10** (provider, evidencia — S1 aprobado, y dos cosas operativas que cuestan tiempo si se redescubren):
`81ce8e7` (cam-ops, 141 lineas) cerro **approved** con los cuatro lentes y **sin un solo hallazgo**;
lineage `review-1a4ea5cb2a426e2f`, acknowledgement quemado.

1. **Para revisar un commit del medio de la cadena hay que pararse en el.** La proyeccion calcula
   `base → HEAD`, no `base → commit`, asi que con el arbol en `dev` cualquier `baseRef` arrastraba las
   5.468 lineas de M2 y rebotaba. Se resolvio con `git switch --detach 81ce8e7`, revisar, y volver.
2. **El tier `high` de S1 lo disparo otra vez la senal `process_boundary` sobre `crates/cam-ops/src/lib.rs`**,
   que son catorce lineas de declaraciones de modulo y re-exports: sin proceso, sin shell, sin `std::fs`,
   sin `std::env`. Es la cuarta vez en el proyecto. Consecuencia practica: un movimiento de siete tipos
   costo cuatro corridas de modelo. La senal es del proveedor y no se discute, pero conviene tenerla
   presente al estimar el costo de revisar cambios chicos en `cam-ops`.

**L11** (provider, evidencia — dos CRITICAL reales, y un recibo que el proveedor no pudo emitir): el
review de `cc78d52` (el lector de Gerber) corrio cuatro lentes que trabajaron entre 140 y 277 segundos —el
doble que cualquier otro candidato— y el refuter confirmo **dos defectos CRITICAL deterministas,
introducidos por el commit**:

- **R1-001** (risk): `parse_fs` validaba la *forma* del formato (`len == 2`, X igual a Y) y despues
  desenvolvia `to_digit(10)` de cada caracter. `%FSLAXAAYAA*%` pasa todas las validaciones de forma y
  **paniquea**: negacion de servicio con entrada no confiable.
- **R4-001** (resilience): un archivo que termina dentro de un bloque extendido se **descartaba en
  silencio**. El bucle sale, nadie mira `in_block`, y `parse` devuelve `Ok` si ya vio `%FS` y `%MO`. El
  lector declaraba la regla "un archivo se lee entero o no se lee" y no la aplicaba en el ultimo lugar
  donde importa.

Los dos se verificaron leyendo el codigo y se corrigieron en `a8d2e55`, con un test por defecto: 11 tests
en el crate, 1264 en el workspace, gates verdes.

**Lo que no se pudo cerrar:** la validacion targetada del candidato corregido fue **rechazada dos veces en
admission** con el mismo motivo y resultados distintos (`targeted validation result does not bind the
provider-owned correction request [invalid_request]`, 2.410 y 2.135 caracteres, `mutation_performed:
false`, slot sin consumir). Se siguio la continuacion ofrecida —STATUS fresco y correr ese mismo slot— una
vez; un tercer intento seria reintento automatico, que la politica prohibe. El lineage
`review-a768d00350e5a18b` queda en `correction_required`.

**Estado honesto:** la correccion es real y la verifican los gates del proyecto; lo que falta es el veredicto
del proveedor sobre ella. Queda registrado en vez de disimulado.

**L12** (cierre del lineage colgado, y la leccion que costo): el lineage `review-a768d00350e5a18b`
no se cerro validando, se cerro **recuperando**. Al consultarlo despues de las dos admissiones
rechazadas, el proveedor devolvio `action: recover`, `disposition: scope_changed`, con
`current_candidate_tree = 701d7e77` distinto del candidato de la correccion. Diagnostico: el candidato se
**movio debajo del lineage** porque segui commiteando C1 mientras la validacion targetada estaba pendiente.

Lo que no puedo afirmar con certeza es que ese movimiento sea la causa de los dos rechazos: al momento de
la primera admission el arbol todavia era el de la correccion. Lo que si es cierto es que el proveedor
congela una identidad de candidato y que esa ventana no se puede tocar; una vez movida, la unica salida es
la que ofrecio. Queda como leccion operativa y no como sentencia sobre el relay.

**La recuperacion.** `review recover` con `disposition=scope_changed`, predecessor
`review-a768d00350e5a18b`, successor `review-3f5e6a8605ca2a0f`, base `a6e4d217` (el padre del commit de
Gerber). El proveedor pidio dos campos mas de los seis que habia emitido —`actor` y `reason`— y recien con
esos ocho commiteo. El predecessor quedo cerrado con su disposicion; el successor arranco en `reviewing`
sobre **2.043 lineas** (`9618ab24..701d7e77`): el lector de Gerber con su correccion mas C1 completo, en
un solo candidato. Es mas de lo que pedi y mejor: la correccion del Gerber por fin tiene veredicto, y
comparte candidato con el motor de conjuntos.

**El relay se cayo una vez y se reintento una vez.** La primera corrida del grupo murio con
`pi-host-relay-transport-failure`: el revisor de `review-readability` recibio
`[internal_server_error]` del proveedor a los 207 s, sin un solo token de salida y con
`prepared_reviewers: 0`. No se consumio nada. STATUS fresco reofrecio **el mismo grupo sin cambios** —misma
revision `497f7186`, mismos cuatro subject hashes— y esa reentrega es la continuacion exacta ofrecida, no
un replay. La segunda corrida termino las cuatro lentes en 52 s, 139 s, 212 s y 107 s.

**Resultado: aprobada**, con un unico hallazgo **no bloqueante**: `R2-001` (readability,
`crates/cam-gerber/src/lib.rs:506-512`, severidad WARNING, disposicion informativa). No abrio correccion ni
reofrecio transicion de correccion, asi que queda como trabajo aparte y no como motivo para volver a
revisar este candidato. La autoridad quedo **quemada** con `gentle-ai.review-acknowledged/v1`.

**L13** (regla, para no repetirlo): **no se commitea nada mientras una validacion de review esta
pendiente.** La ventana de una validation targetada es la identidad de un candidato congelado; un commit
ajeno a la correccion la invalida, y recuperar cuesta un ciclo de review entero. Si hay trabajo que
continuar mientras el proveedor valida, va a otra rama o no va.

**Pendiente por `R2-001`:** revisar `crates/cam-gerber/src/lib.rs:506-512` en la proxima pasada por el
lector. Es el bloque de `AD`/`ADD` que desenvuelve `strip_prefix(..).unwrap()` dos veces seguidas; el
`unwrap` es seguro por el `starts_with` que lo precede, pero la simetria con el bloque `ADD` invita a
unificarlos en una sola rama.

**L14** (E1 de Excellon, y un defecto de escala que aparecio al escribir el dialecto): `coordinates()`
hacia `number.parse::<f64>()` y lo pasaba a mm **sin aplicar la cantidad de decimales del archivo**. Un
`X11528623` de un Excellon metrico 2:6 se leia como 11528.623 mm en vez de 11.528623: el taladro a un
factor del conteo de decimales de distancia, en silencio, que es justo la clase de error que el `//!` del
crate dice rechazar. Los fixtures existentes tenian punto decimal y por eso nadie lo veia.

Ahora el formato se lee de dos lugares, que es como lo escriben las herramientas: un comentario
`;FILE_FORMAT=i:d` (KiCad) y un patron de ceros como `METRIC,000.000`, que declara 3 y 3. Un entero se
divide por `10^d`; un numero con punto decimal conserva su propia escala. **Un entero sin formato
declarado se rechaza nombrando la linea**, porque adivinar `d` es precisamente el error de colocar el
taladro en el lugar equivocado. La excepcion es el cero: `Y0` es como una linea de taladrado normal
escribe un agujero sobre el eje, y cero es cero a cualquier escala.

`METRIC,LZ` y `METRIC,TZ` declaran supresion de ceros pero **no** cuantos digitos hay, asi que no
alcanzan como formato — se aceptan como unidades y el entero posterior se rechaza si no hay otro dato.

Probado por mutacion: sacando la division muere el par de tests del formato; sacando el rechazo muere el
test del archivo sin formato. 13 tests en el crate, workspace en **1315 passed, 0 failed, 3 ignored**.

**L15** (E2): `G90` y `G91` se leen los dos. En modo relativo cada coordenada es un paso desde donde la
herramienta ya esta, y `G90` vuelve a absoluto; el estado arranca absoluto, que es el defecto del
formato. Esto reemplaza el rechazo por nombre que tenia el crate, y con el se fue el test que exigia ese
rechazo: `a_relative_program_is_refused_naming_the_line` paso a
`a_relative_program_accumulates_from_the_last_position`.

Probado por mutacion: olvidando el paso relativo muere el test de acumulacion; haciendo que `G90` **no**
vuelva a absoluto mueren dos —el test del modo y el del archivo KiCad, que usa `G90`—, que es la senal de
que el flag estaba haciendo trabajo real en un fixture que no escribi para esta tarea.

**Estado:** 14 tests en cam-excellon, workspace en **1316 passed, 0 failed, 3 ignored**. Con E1 (formato de
digitos) y E2 (modo relativo) cerrados, los dos dialectos que el crate rechazaba por nombre ya se leen.
