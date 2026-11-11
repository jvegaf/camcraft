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
