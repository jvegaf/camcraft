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
