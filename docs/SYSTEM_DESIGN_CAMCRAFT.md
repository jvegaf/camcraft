# System design: camcraft (v0.1 — para iterar)

**Estado.** Propuesta de arquitectura. Este documento **reemplaza**:

- el orden de fases y la estructura de crates de [ROADMAP_CAMCRAFT.md](ROADMAP_CAMCRAFT.md) §2 y §14;
- la decisión **D3** y **D4** de [GAPS_CAMCRAFT.md](GAPS_CAMCRAFT.md) §6, por las razones de §2 de este documento.

Sigue vigente: el alcance funcional de [Plan de camcraft.md](old/Plan%20de%20camcraft.md) §3–§4, las decisiones
**D1, D2, D5 y D6** de GAPS §6, y [RUST_PORT_LOGIC.md](old/RUST_PORT_LOGIC.md) como especificación de
referencia de la lógica de FlatCAM Plus.

**Qué es este documento.** El contrato arquitectónico del producto: qué invariantes no se negocian,
cómo se ejecuta una operación, cómo se divide el workspace, cómo se mide, cómo se desarrolla en paralelo
y dónde están las puertas. No es el roadmap de tareas (eso sigue viviendo en `odd/tasks/`) ni la
especificación de formatos: `format.md` describe `c2d/document@2`, el formato que este producto ya
no escribe; la especificación de `camcraft/document@1` (§9.1) es trabajo del hito C3 y todavía no existe.

---

## 1. El pivote, asentado

| # | Decisión | Consecuencia arquitectónica |
| --- | --- | --- |
| 1 | **camcraft es el producto; campilot es legado.** | El workspace se llama `camcraft`. El nombre `campilot` sobrevive solo como historia del repositorio, y su documentación histórica no se
mantiene. |
| 2 | **Solo PCB, al estilo de FlatCAM.** Todo el dominio de madera y mecanizado general **sale**. | `cam-toolpath` desaparece como crate. Se rescatan tres capacidades internas, no once estrategias. |
| 3 | **Calidad sobre plazo, desarrollado con ODD, paralelizable en varios worktrees.** | La arquitectura se elige por **aislamiento de superficies de edición**, no por velocidad de arranque. Contratos congelados antes de fan-out. |
| 4 | **`cam-app` se rediseña** (undo incluido). | Undo deja de ser snapshot del documento completo. §10. |

**La consecuencia en una línea:** camcraft es un CAM de PCB. Todo lo que no sirva a importar, aislar,
taladrar, cortar, rellenar, panelizar o emitir una placa, sale del producto, del binario y de la UI.

### 1.1 Por qué el pivote habilita una arquitectura mejor, no solo más chica

El GAPS §5 identificaba seis riesgos que "fuerzan rediseño, no solo código nuevo". Tres de ellos eran
consecuencia de sostener dos dominios a la vez: `EntityKind` con una sola variante compartida por madera
y PCB, un solo `Job`/`Frame` documento↔máquina, y undo por snapshot justificado "por tamaño de
documento". Con la madera afuera, el modelo puede ser **nativo de PCB** en lugar de aditivo sobre un
modelo genérico. Eso elimina la migración `@2→@3` (nadie va a importar documentos de madera) y elimina
la metadata-como-parche de D3: el rol de capa, la apertura, la polaridad y la herramienta son **campos**,
no un diccionario de strings.

---

## 2. Alcance

### 2.1 Dentro

Importación Gerber RS-274X, Excellon y DXF/SVG (geometría general, A1); importación de carpeta de
fabricación; aislamiento (isolation)
multipasada; *non-copper clearing* (NCC) por offsets con islas, paralelo y con rest machining; taladrado
desde tabla Excellon con slots y ordenamiento determinista; cutout con tabs y mouse-bites; paint;
panelize; autolevelling como transformación del `MoveStream`; ruteo alrededor de áreas de exclusión;
catálogo de dialectos `.pp`; exportación Gerber, Excellon, DXF y SVG; simulación volumétrica; previsualización de
toolpaths; CLI headless; UI completa; corpus, paridad, benchmarks y CI.

### 2.2 Fuera (el "afueraaa")

Decisión consciente, no postergación. Nada de esto se implementa, se testea ni se muestra:

| Sale | Dónde vivía | Qué se pierde |
| --- | --- | --- |
| Las 11 estrategias de mecanizado general | `cam-toolpath` (3.459 líneas) | profile, pocket, drill, chamfer, vcarve, engrave, fluting, texture, inlay, thread, moulding como *operaciones de usuario* |
| Eje medial y V-carve | `cam-geom/src/medial.rs` (1.038 líneas) | nada del dominio PCB |
| Nesting de piezas | `cam-geom/src/nest.rs`, `nest_profile.rs` | nada: panelize es un problema distinto (matriz rows×cols), no aprovechamiento de material |
| Moulding, texto de un solo trazo, autoría vectorial avanzada | `cam-geom`, `cam-ui` | nada |
| PDF / EPS / PNG raster trace | `cam-import` | nada del dominio PCB |
| Cut list, print sheet HTML, command language de part lists | `cam-cli`, `cam-ui` | nada |
| Formularios de las 11 estrategias | `cam-ui` | nada |

**Lo que se rescata de `cam-toolpath`, como capacidad interna y sin nombre de operación de usuario:**

1. **Anillos de offset con islas** (`pocket.rs:124-158,332-337`) → motor de isolation y NCC.
2. **Peck / retract / drill bank** (`drill.rs:85-132`) → ejecución de taladrado.
3. **Perfil con tabs** (`lib.rs:483`) → cuerpo del cutout. *(Su soporte de tabs hay que verificarlo
   antes de asumirlo: GAPS §3 G3.6 lo marca A VERIFICAR.)*

Estimado de código que sale del producto: **≈15.000 líneas** entre `cam-toolpath`, el subconjunto de
`cam-geom`, `cam-import`, `cam-model`, `cam-cli` y `cam-ui`. Es el mayor recorte de deuda que el pivote
regala, y se ejecuta antes de C0 (§2.4): cada semana que la madera siga compilando es una semana de
CI, tests y formularios que nadie pidió.

### 2.3 A1 resuelto — DXF/SVG dentro, geometría general

Se conservan **import y export de DXF/SVG** con paridad de producto respecto de FlatCAM. La decisión
es de alcance, no de esfuerzo: portar los importadores a `cam-prim` es churn de `use` más un campo de
provenance, porque `Contour`/`Arc` solo cambian de crate.

**Subconjunto soportado, declarado:** `LINE`, `LWPOLYLINE`, `POLYLINE`, `ARC`, `CIRCLE`, agrupados por
capa, con la capa mapeada a `Provenance`.

**Fuera del subconjunto, reportado como `Skipped { construct, reason, location }` (I4):** `SPLINE`,
`TEXT`, `INSERT`/bloques, cotas, `HATCH`, y todo EPS/PDF/PNG.

Tres flujos PCB que esto habilita y que Gerber no cubre: contorno de panel desde CAD mecánico, fresado
de una forma arbitraria en la placa, y round-trip a CAD para revisar lo que el producto generó. El
costo asumido es **dos formatos más en el corpus** y un camino de import más en la UI.

### 2.4 A5 — el barrido de borrado, primera tarea del proyecto

**Rescates primero, árbol verde, después borrar.** Un PR de borrado no cambia comportamiento ni agrega
features; si un rescate necesita una decisión de diseño, va al PR siguiente.

Los tres rescates de §2.2 aterrizan en un **`cam-ops` mínimo creado en el mismo PR 1** (tres módulos
públicos con sus tests; el trait `Node` llega en C4). Es la semilla del crate y evita código muerto.

| PR | Borra | Rescata | Verificación |
| --- | --- | --- | --- |
| **1** | `cam-toolpath` (3.459); el editor de estrategia de `cam-ui`; los subcomandos de estrategia de `cam-cli` | `rings` (`pocket.rs:124-158,332-337`), `peck` (`drill.rs:85-132`), `follow` (`lib.rs:483`) → `cam-ops` | los tests de las tres capacidades se mudan y quedan verdes; los tests de las 11 estrategias se borran con su código; `cam-ui` y `cam-cli` compilan |
| **2** | `cam-geom`: `medial.rs` (1.038), `nest.rs`, `nest_profile.rs`, moulding, texto | nada | el kernel queda con primitivas, arrangement, offset y regiones; `just check` verde |
| **3** | `cam-import`: PDF, EPS y PNG raster trace | DXF/SVG sin cambios | `Format` pierde tres variantes; los tests de DXF/SVG verdes |
| **4** | `cam-model`: variantes de `Operation` de madera; `cam-cli`: `cutlist`, `parts`, `report`; `cam-ui`: autoría vectorial avanzada y paneles de part list | nada | el modelo queda listo para el schema PCB de §9.1 |

**Riesgo del barrido:** borrar algo que un hito posterior quería. Mitigación: cada PR lista lo que
borra y lo que conserva, y los tres rescates están nombrados con su rango de líneas de origen.

---

## 3. Invariantes

Nueve reglas. Cada una responde a un modo de falla concreto, y cada una tiene un test que la sostiene.
Si una decisión futura viola una, la discusión es sobre el invariante, no sobre el caso.

- **I1 — El kernel primero, la operación después.** Ninguna operación PCB se considera terminada sin el
  instrumento que la juzga ya existente y en verde. Escribir NCC antes del predicado de penetración es
  escribir código que no se puede aprobar ni rechazar.
- **I2 — Nada se aplana en silencio.** Los arcos son *first-class* de punta a punta. Toda operación
  declara `ArcPolicy::Preserve` o `ArcPolicy::Flatten { chord }`, y el aplanado explícito es visible en
  el artefacto que lo produce. Un aplanado implícito es un bug, no una optimización.
- **I3 — La tolerancia es un parámetro, nunca un literal ni un ambiente.** No hay `thread_local`, no hay
  constantes por defecto dentro de una función geométrica, no hay "el epsilon de siempre". Toda
  operación geométrica pública recibe la política.
- **I4 — Nada se descarta en silencio.** El patrón `ImportReport`/`Skipped { construct, reason,
  location }` (`cam-import/src/report.rs:24-115`) se hereda para Gerber, Excellon, carpeta de
  fabricación y para cualquier geometría que el kernel rechace.
- **I5 — Cero reloj en el pipeline.** Ninguna etapa decide por tiempo transcurrido. Los presupuestos son
  de **iteraciones**, no de segundos. Es la razón por la que FlatCAM no es reproducible (usa OR-Tools
  con presupuesto de tiempo, `camlib.py:3040,3104`) y la razón por la que nosotros sí podemos serlo.
- **I6 — El resultado es contenido, no orden de llegada.** Todo artefacto se identifica por el hash de
  contenido de sus entradas y parámetros. Dos corridas producen bytes iguales con cualquier número de
  hilos.
- **I7 — CLI primero, UI al final.** Toda operación se ejercita por `cam-cli` con exit codes estables
  antes de tener formulario. La UI es una capa sobre operaciones probadas, nunca el lugar donde se
  descubre si funcionan.
- **I8 — Geometría de kernel ≠ geometría de pantalla.** El kernel guarda arcos y regiones exactas; la
  pantalla consume una malla teselada, indexada y cacheada. Nunca se degrada el kernel para que la UI
  dibuje rápido. §5.4.
- **I9 — La cache no cambia el resultado.** Un artefacto servido desde disco es indistinguible de uno
  recomputado: el pipeline con cache fría y con cache caliente produce bytes idénticos. La cache es
  una optimización de tiempo, nunca una fuente de verdad. §4.5.

---

## 4. Modelo de ejecución: el grafo de artefactos

Esta es la decisión arquitectónica central del pivote, y la que habilita el desarrollo en paralelo.

### 4.1 El problema con "documento + lista de operaciones"

El modelo actual es plano: un documento tiene operaciones, cada operación lee el documento y emite un
`MoveStream`. Eso funciona para once estrategias independientes sobre geometría chica. **No funciona
para PCB**, por cuatro razones concretas:

1. **Rest machining** (grueso→fino) necesita que una operación consuma *lo que otra dejó*: un artefacto
   intermedio, no una posición en una lista.
2. **NCC con islas** quiere paralelizarse por isla y reducir con orden estable: necesita nodos, no un
   bucle.
3. **Caching y recomputación incremental** son la diferencia entre una UI usable y una que congela el
   shell cada vez que movés un stepover. Sin identidad de contenido no hay cache.
4. **Medición por etapa** (la promesa de performance) requiere benchmarks por nodo, no por programa
   final.

### 4.2 La forma

```
                   ┌──────────────┐
  fab/ ── gerber ──► copper_top   ├──┐
        excellon ──► drill_table  │  │
                   └──────────────┘  │
                                     ▼
              ┌─────────────────────────────────────────────┐
              │  nodos de operación (DAG, content-addressed) │
              │                                             │
              │  union ──► iso_passes ─────┐                │
              │    │                       │                │
              │    └──► clearance_env ──► ncc_rings ──┐      │
              │                              rest ───┘      │
              │  drill_table ──► sorted_hits ──────────┤     │
              │  outline ──► cutout ───────────────────┤     │
              └────────────────────────────────────────┼─────┘
                                                       ▼
                                              moves ──► gcode
                                                       │
                                            oracle ────┘  (L1 / L2)
```

Un **nodo** es una operación con entradas nombradas y salidas tipadas. Un **artefacto** es un valor
inmutable identificado por el hash de contenido de lo que lo produjo.

```rust
// cam-graph — contratos, no implementaciones
pub struct ArtifactKey(u128);          // hash de contenido canónico

pub enum Artifact {
    Regions(Arc<RegionSet>),           // cobre, envolvente, resto, exclusiones
    Contours(Arc<ContourSet>),         // contornos abiertos, tabs, outline
    Holes(Arc<HoleTable>),             // taladros agrupados por herramienta
    Moves(Arc<MoveStream>),            // programa parcial
}

pub trait Node: Send + Sync {
    fn kind(&self) -> NodeKind;
    fn inputs(&self) -> &[InputSlot];                  // (nombre, ArtifactKind, requerido)
    fn outputs(&self) -> &[OutputSlot];                // (nombre, ArtifactKind)
    fn params(&self) -> ArtifactKey;                   // parámetros canónicos, sin orden de mapa
    fn evaluate(&self, cx: &mut EvalCx<'_>) -> Result<NodeOutcome>;
}

pub struct EvalCx<'a> {
    store: &'a ArtifactStore,          // cache: ArtifactKey → Arc<Artifact>
    policy: &'a TolerancePolicy,
    cancel: &'a CancellationToken,
    progress: &'a dyn Fn(NodeId, Progress),   // islas hechas / islas totales
}
```

**Clave de cache:**

```
key = H( node_kind ‖ params_canónicos ‖ ordenadas(key(input)) ‖ kernel_version ‖ tolerance_policy )
```

`kernel_version` es deliberadamente parte de la clave: cambiar el algoritmo de offset invalida la cache
en lugar de servir geometría vieja con código nuevo. Es el bug clásico de toda cache geométrica.

### 4.3 Lo que el grafo compra, uno por uno

| Necesidad | Cómo la resuelve el grafo |
| --- | --- |
| Rest machining (E4.5) | Nodo `ncc` declara `rest` como salida; la herramienta siguiente lo declara como entrada. Sin lógica ad hoc. |
| Paralelismo determinista (E4.4) | Las islas son trabajo interno del nodo (§4.6), reducidas en orden estable; el orden de terminación no toca la salida. |
| Recomputación incremental (UI) | El editor cambia el stepover → se invalida ese nodo y sus descendientes, y nada más. |
| Benchmark por etapa (E0.7) | Cada nodo es medible por separado. "NCC es más rápido" se descompone en qué etapa lo es. |
| Oráculo por etapa (D5) | El predicado L1 corre sobre el artefacto `Moves` de un nodo, no solo sobre el programa final. |
| Cancelación y progreso (UI) | `EvalCx` lleva token y callback: NCC sobre una placa grande reporta isla por isla y se puede abortar. |
| Desarrollo en paralelo (§13) | Cada operación es un nodo contra un trait congelado. Seis worktrees sin tocarse. |

### 4.4 Lo que el grafo **no** hace

- **No se persiste en el documento.** El documento guarda operaciones y parámetros; el grafo es una
  proyección derivada (`Document → Graph`). Persistir el grafo sería persistir una cache con el riesgo
  de servir un resultado inconsistente con la versión del kernel.
- **No es un lenguaje de scripting.** No hay nodos arbitrarios creados por el usuario. El conjunto de
  `NodeKind` es cerrado y versionado.
- **No reemplaza al `MoveStream`.** El grafo produce `Moves`; `cam-move` y `cam-post` no se tocan.
- **No cachea la topología ni el documento.** Solo salidas de nodo. El grafo se deriva siempre.

### 4.5 A3 resuelto — cache en disco

La cache persiste entre sesiones. Eso deja de ser una optimización y pasa a ser un subsistema con
superficie propia: un bug de cache es el bug más difícil de reproducir del proyecto, porque depende del
estado del disco de alguien. Siete reglas, todas con test.

```
$CAMCRAFT_CACHE_DIR  ó  $XDG_CACHE_HOME/camcraft  (Linux)  ó  %LOCALAPPDATA%\camcraft\cache  (Windows)
  artifacts/<key>.bin      payload con su propia versión de formato
  artifacts/<key>.sha256   checksum del payload
```

1. **La versión del formato de artefacto entra en la clave.** `kernel_version` cubre el algoritmo *y*
   el encoding. Cambiar cualquiera de los dos invalida todo sin escribir una línea de eviction.
2. **Escritura atómica** (`.tmp` + rename, como el documento). Un archivo a medio escribir nunca es
   visible con su nombre final.
3. **Verificación al leer:** sha256 contra el `.sha256`; cualquier discrepancia es *cache miss* más
   borrado del par. **Una cache corrupta nunca es un error para el usuario.**
4. **I9 se testea, no se espera:** correr el pipeline con cache fría y con cache caliente y exigir
   bytes idénticos (§8, regla 7).
5. **La eviction puede mirar el reloj; la evaluación no.** LRU por `mtime` con un tope de tamaño total
   (2 GB por defecto, A6). Esa distinción es la que evita reintroducir no-determinismo por la puerta de
   atrás (I5).
6. **Los tests nunca tocan la cache real:** `CAMCRAFT_CACHE_DIR` a un temporal por test, o
   `CAMCRAFT_CACHE=off`. Una cache compartida entre tests es una máquina de tests dependientes del
   orden.
7. **Sin locks:** content-addressed más escritura atómica significa que dos procesos —o dos
   worktrees— que calculan la misma clave escriben el mismo contenido. Perder la carrera es un rename
   de más.

Más `camcraft cache info|prune` en el CLI: un directorio de cache sin comando de limpieza es basura
que el usuario no sabe que existe.

**A7 — compartida entre documentos.** Al ser content-addressed, dos `.cmc` distintos con la misma capa
de cobre comparten artefactos: panelizar variantes e iterar un diseño se vuelve casi gratis.
**Costo asumido:** borrar un `.cmc` no libera su geometría; solo `prune` lo hace.

### 4.6 Reglas del nodo

**`NodeKind` es cerrado y es un subconjunto de lo que el usuario edita.** Cada variante corresponde a
una operación del documento o a una derivación que el usuario nombra —una herramienta de NCC *es* un
nodo—. Las islas de NCC **no** son nodos: son trabajo interno del nodo, porque el grafo es una
proyección del documento y las islas no se editan.

```rust
pub enum NodeKind {
    ImportLayer  { role: LayerRole, format: Format },
    Union        { layer: LayerId },
    Offset       { distance: f64, side: Side },
    Isolation    { tool: ToolId, passes: u8, overlap: f64, keep_pours: bool },
    Clearance    { margin: f64 },
    Ncc          { tool: ToolId, method: Fill },   // uno por herramienta; `rest` los encadena
    Drill        { tool: ToolId },
    Cutout       { tool: ToolId, tabs: TabSpec },
    Paint        { tool: ToolId, region: ArtifactRef },
    Panelize     { rows: u32, cols: u32, spacing: f64 },
}
```

Seis reglas duras:

1. **Un nodo no toca el mundo exterior.** Sin filesystem, sin reloj, sin RNG, sin red, sin variables de
   entorno: solo `EvalCx`. Verificable en CI — `cam-ops`, `cam-op-*` y `cam-graph` no dependen de
   `std::fs`, `rand` ni `std::env`.
2. **Ids estables desde el documento, nunca por índice.** `NodeId` sale de un id que la operación ya
   tiene en el `.cmc`, así reordenar la lista de operaciones **no** invalida la cache.
3. **Una herramienta de NCC es un nodo.** `ncc(A) ──rest──► ncc(B) ──rest──► ncc(C)`. Quitar la
   herramienta intermedia es un error de validación que nombra las dos operaciones y el artefacto que
   deja de existir, no una recomputación silenciosa.
4. **Orden topológico con desempate por `NodeId`.** Los nodos independientes se evalúan en paralelo; la
   recolección es ordenada y la escritura de cache no influye en el resultado (I9).
5. **Un error de parámetros nombra el parámetro y sugiere un valor.** No `isolation failed`, sino
   `isolation: la fresa de 0.2 mm deja 0.07 mm de cobre entre pasadas; bajá el overlap a 0.4 o usá una
   fresa de 0.15 mm`. Es la diferencia con FlatCAM, que produce basura en silencio.
6. **El registro es una línea por operación.**
   `registry.register(NodeKind::Isolation, || Box::new(Isolation::default()));` — lo que permite seis
   worktrees sin conflictos (§13.3).

### 4.7 Encoding de artefactos

Con cache en disco, el encoding deja de ser un detalle interno y pasa a ser **formato**: su versión
integra la clave (§4.5.1), así que cambiarlo invalida artefactos de todos los usuarios.

- **Canónico antes que compacto.** JSON con claves ordenadas (`BTreeMap`, nunca `HashMap`) y `f64` en su
  forma de ida y vuelta exacta —el workspace ya configura `serde_json` con `float_roundtrip`, y esa
  decisión ahora se vuelve load-bearing—. Bytes canónicos, diffables y depurables a mano.
- **Lo que se hashea es lo que se escribe.** No hay re-encoding después de calcular la clave: un *hit* es
  copiar el archivo. Si el canónico y el escrito difirieran, el hash no probaría nada.
- **`blake3` truncado a 128 bits** para `ArtifactKey`. Hashear decenas de MB de geometría con `sha256`
  cuesta la mitad del presupuesto de un *hit*; `blake3` es SIMD y de un solo archivo (§6.2).
- **El payload lleva su propia versión** en la primera clave (`"format": 3`), redundante a propósito: un
  archivo cuyo nombre diga una clave y cuyo contenido diga otra se descarta, no se interpreta.
- **Sin referencias entre artefactos.** Al serializar, `Arc` se desarma; al leer, se rearma. Un artefacto
  autocontenido no puede corromperse en cascada por otro.

---

## 5. El kernel

### 5.1 Tres capas, topológicamente acíclicas

```
cam-prim        Point · Arc · Segment · Contour (bulged) · Region · winding exacto
                teselación con tolerancia declarada · TolerancePolicy · ArcPolicy
   │
   ├──► cam-boolean     arrangement: cortar por fracciones, clasificar lado material, ensamblar
   │                    ∪ ∩ ∖ △ por predicado de selección de tramos
   │
   ├──► cam-offset      offset arrangement-based: crecer, resolver auto-intersección por
   │                    arrangement, probar distancia contra todo contorno original
   │                    (depende de cam-boolean)
   │
   └──► cam-index       índice espacial (rstar) como servicio: vecindad, intersección,
                        nearest, consulta por ventana
```

`cam-index` depende solo de `cam-prim`, así que se desarrolla en paralelo con todo. `cam-offset`
depende de `cam-boolean`, y por eso está después en el camino crítico (§14).

**Qué se conserva de `cam-geom`:** el arrangement existente (`boolean.rs`: `cut_fractions:152`,
`material_side:306`, `assemble:391`, `survives_the_bulge:588`) que **preserva arcos** — es la razón de
D1 y sigue siendo válida. La prueba de distancia del offset ("nunca conserva un tramo que no midió") se
conserva como invariante testeable.

**Qué se reemplaza:** el *cleanup* de auto-intersección del offset, que hoy tessela, parte y se queda
con el loop más grande reportando `Flattened` (`offset.rs:403-491`), y el rechazo de
`Region::offset` cuando el offset atraviesa la forma (`region_offset.rs:120-185`). Ese cleanup pasa a
ser una operación de arrangement, y el rechazo pasa a ser un diagnóstico accionable. **Esta es la tarea
más costosa del proyecto después del motor de conjuntos, y no tiene hoy ningún ID en el roadmap:**
GAPS la marca BLOQUEANTE (G3.2) y ROADMAP la trata como "riesgo de F3". Acá se eleva a puerta propia
(C2, §14).

### 5.2 Política de tolerancia

Un solo objeto, explícito, por operación:

```rust
pub struct TolerancePolicy {
    pub chord: f64,             // error de cuerda al teselar, mm
    pub epsilon: f64,           // comparación de puntos y áreas
    pub arc: ArcPolicy,         // Preserve | Flatten { chord }
    pub simplify: Simplify,     // Off | Collinear { epsilon } | DouglasPeucker { epsilon }
}
```

Nada dentro del kernel crea una política por su cuenta. La política viaja en la clave de cache — dos
políticas distintas producen artefactos distintos, que es exactamente lo que queremos.

### 5.3 Área de paridad y el problema del aplanado

FlatCAM aplana todo al pasar por shapely. Nosotros preservamos arcos. La comparación con FlatCAM es de
**área (ε relativo 1e-9), cobertura y conteo de polígonos**, nunca de conteo de vértices. Se implementa
en `cam-oracle` (§11), no en los tests de cada operación.

### 5.4 La frontera kernel/pantalla

Una placa grande tiene decenas de miles de polígonos. Si la UI dibuja geometría de kernel, el shell se
congela. Se define explícitamente:

```
Region/Contour (kernel, arcos, exacto)
        │  teselado con chord dependiente del zoom
        ▼
DisplayMesh (vértices, índices, cacheado por (artifact_key, zoom_bucket))
        │
        ▼
cam-view (escena, picking, culling por índice espacial) ──► cam-ui (egui)
```

Consecuencias: `cam-view` cachea mallas por `(ArtifactKey, zoom_bucket)`; el picking usa el índice
espacial del kernel, no la malla; un cambio de zoom re-tesela en background con cancelación. El estado
de aplicación guarda `Arc<DisplayMesh>`, no geometría duplicada.

---

### 5.5 Preview volumétrico: qué tomamos de `gsim-rs` y qué no

`gsim-rs` (2.0.0, MIT/Apache, ~6.500 líneas) es un simulador de G-code Fanuc para fresado vertical, con
stock volumétrico en WGPU y TUI en ratatui. **No entra como dependencia**, por tres razones
estructurales: parsearía el G-code que nosotros emitimos para volver a simular, invirtiendo el pipeline
(el contrato es `MoveStream`, y `cam-post` es lo único que produce texto); corre en GPU con batching por
frame, así que depende del frame y no tiene CI; y es un visor, no un oráculo — no reporta gouge ni uncut
con cota. Además metería `wgpu`, `winit` y `ratatui` debajo de `cam-ui`, contra §7.

**Sí entra como referencia para C8**, porque sus tres técnicas de render son las correctas para una
placa, que es 2.5D y tiene muchísimos movimientos:

1. **Caras internas ocultas al inicio**, reveladas cuando el vecino se remueve: menos geometría en
   pantalla y sin *popping*.
2. **Actualizaciones parciales de buffer** por frame, en lugar de re-subir el stock entero.
3. **Batching de movimientos chicos por frame**, con modo paso a paso aparte para inspección.

Y como **herramienta externa opcional**, nunca en el grafo de crates: una receta `just preview out.nc`
que invoque el binario si está instalado y, si no, diga cómo instalarlo. Un humano mira el programa
emitido con un visor maduro; `cam-sim` sigue siendo el que mide.

**Límite explícito:** nada de esto reemplaza al predicado L1 de §11. `gsim-rs` dibuja; si un aislamiento
penetró se decide con una intersección de regiones, y eso lo escribe `cam-oracle`.

---

## 6. Mapa de crates

Regla de granularidad, en este orden: **una crate propia cuando aporta (a) dependencias que las demás no
quieren, (b) benchmarks y goldens propios, o (c) una superficie de edición que justifica un worktree
paralelo.** Si no, es un módulo. LOC son del relevamiento de GAPS o estimación marcada con ≈.

| Crate | LOC | Veredicto | Rol |
| --- | ---: | --- | --- |
| `cam-units` | 228 | **intacto** | mm interno, formateo fixed-decimal |
| `cam-prim` | ≈2.500 | **nuevo** (extraído de `cam-geom`) | primitivas arco-native, región, winding, teselado, política |
| `cam-boolean` | ≈1.500 | **nuevo** (de `boolean.rs`) | arrangement + ∪ ∩ ∖ △ |
| `cam-offset` | ≈1.200 | **nuevo** | offset arrangement-based, anillos, envolventes de clearance |
| `cam-index` | ≈400 | **nuevo** | índice espacial como servicio |
| `cam-move` | 311 | **intacto** | `Move`/`Segment`/`MoveStream` |
| `cam-post` | 1.176 | **intacto** | parser `.pp` + emisor, golden byte a byte |
| `cam-sim` | 2.506 | **adaptar** | keyed por artefacto; sin cambios de algoritmo |
| `cam-model` | ≈2.000 | **reescrito** | documento PCB nativo, capas con rol, operaciones |
| `cam-store` | ≈2.000 | **adaptar** | archivo `.cmc`; `tools.sqlite` y su escalera de migraciones intactos |
| `cam-gerber` | ≈3.500 | **nuevo** | RS-274X: aperturas, macros, polaridad, G36/G37, G74/G75 |
| `cam-excellon` | ≈1.200 | **nuevo** | herramientas, unidades, ceros omitidos, slots |
| `cam-fabric` | ≈800 | **nuevo** | carpeta de fabricación → capas con rol, + informe |
| `cam-import` | ≈2.000 | **conservado, recortado** | DXF/SVG import con subconjunto geométrico declarado; PDF/EPS/PNG se borran (A1) |
| `cam-export` | ≈1.600 | **ampliado** | + escritores Gerber y Excellon; conserva y angosta DXF/SVG |
| `cam-ops` | ≈600 | **nuevo** | trait `Node`, registro, ops livianas: drill, cutout, panelize, paint |
| `cam-op-isolation` | ≈1.100 | **nuevo** | crate propia: depende de `cam-offset`, benchmarks propios |
| `cam-op-ncc` | ≈2.500 | **nuevo** | crate propia: `rayon`, benchmarks propios, la promesa del producto |
| `cam-graph` | ≈1.100 | **nuevo** | evaluador del DAG, cache en memoria y en disco, encoding versionado, cancelación, progreso |
| `cam-oracle` | ≈900 | **nuevo** | predicado L1, paridad geométrica, corpus |
| `cam-cli` | ≈3.000 | **reescrito** | 15→≈14 subcomandos PCB, exit codes estables, `script` conservado |
| `cam-app` | ≈1.500 | **reescrito** | estado, comandos, undo estructural (§10) |
| `cam-view` | 993 | **adaptar** | escena, picking, cache de mallas, culling |
| `cam-ui` | ≈14.000 | **recortado y reestructurado** | shell egui: capas, árbol, formularios, preview, simulación |
| `cam-desktop` | 377 | **intacto** | host eframe + diálogos |
| ~~`cam-toolpath`~~ | 3.459 | **borrar** | se rescatan 3 capacidades internas (§2.2) |
| ~~`cam-program`~~ | 652 | **reemplazar** | lo reemplaza `cam-graph` |

Neto: **≈18.500 líneas que se borran o se reemplazan, ≈24.800 nuevas.** El 70% de lo nuevo está en el
kernel, los importadores y NCC — exactamente donde GAPS ubica los bloqueantes.

### 6.1 Lo que no se toca, y por qué

`cam-units`, `cam-move`, `cam-post`, `cam-desktop` y `cam-sim` son activos verificados: contrato de
stream limpio, emisor con goldens byte a byte, mm interno con conversión en frontera, simulación con
`target_for` y `deviations`. Reemplazarlos sería rehacer lo que el GAPS ya declara "no es un gap" (§7).

### 6.2 Dependencias nuevas que el diseño justifica

La regla del repositorio es "ninguna dependencia nueva salvo que una tarea demuestre que es necesaria".
Estas cuatro vienen demostradas por el diseño, no por preferencia:

| Dependencia | Para qué | Por qué no se escribe propia | Dónde |
| --- | --- | --- | --- |
| `rayon` | paralelismo por isla y evaluación de nodos independientes | un *work-stealing scheduler* correcto no es el problema que este producto resuelve | `cam-op-ncc`, `cam-graph` |
| `rstar` | índice espacial (R-tree) | estructura probada en producción, con vecindad y consulta por ventana ya resueltas | `cam-index` |
| `blake3` | hash de contenido de artefactos | velocidad SIMD, un solo archivo, sin la ceremonia de una librería criptográfica | `cam-graph` |
| `criterion` | benchmarks por nodo (§12) | estándar de facto para benchmarks comparables en CI | `benches/` |

Lo que **no** se agrega: ningún crate de geometría (`geo`, `i_overlay`, `clipper2`, `kurbo`, `lyon`,
`cavalier_contours`, `parry`). D1 decidió kernel propio con arcos preservados, y el kernel **es** el
producto.

---

## 7. Contratos de dependencia

Dirección única. La violación se detecta con un test de CI sobre `cargo metadata` (regla mecánica, no
convención).

```
cam-units
  └─ cam-prim ──┬─ cam-index
                ├─ cam-boolean ──┬─ cam-offset
                │                └─ cam-index
                └────────────────────┐
cam-move ◄─────────────────────────┤
cam-post ◄── cam-move              │
cam-sim  ◄── cam-prim, cam-move, cam-model
cam-model ◄─ cam-prim, cam-move
cam-store ◄─ cam-model
cam-gerber, cam-excellon, cam-fabric ◄─ cam-prim, cam-model
cam-ops ◄─ cam-prim, cam-move, cam-model, cam-index
cam-op-isolation ◄─ cam-ops, cam-offset
cam-op-ncc ◄─ cam-ops, cam-offset, cam-index, rayon
cam-graph ◄─ cam-ops, cam-store
cam-oracle ◄─ cam-prim, cam-boolean, cam-move, cam-sim
cam-cli ◄─ todo lo anterior
cam-app ◄─ cam-model, cam-prim
cam-view ◄─ cam-app, cam-prim, cam-index
cam-ui ◄─ cam-view, cam-app
cam-desktop ◄─ cam-ui
```

Tres reglas duras:

1. **Nada debajo de `cam-ui` sabe que existe `egui`.** Ya es cierto hoy; se conserva.
2. **Ninguna operación importa otra operación.** `cam-op-ncc` no conoce `cam-op-isolation`; si necesita
   lo que la otra deja, lo pide como artefacto en el grafo.
3. **`cam-graph` no conoce operaciones concretas por tipo.** Ve `dyn Node` y un registro. Agregar una
   operación toca un archivo nuevo y **una línea** de registro: es lo que evita conflictos entre
   worktrees.

---

## 8. Contrato de determinismo

La promesa del producto es performance *con* reproducibilidad. Estas siete reglas son la definición
operativa de I5 e I6, y hay un test que las verifica.

1. **Sin `HashMap` en el camino geométrico.** Todo mapa que afecte geometría u orden es `BTreeMap` o se
   ordena explícitamente antes de usar.
2. **Sin reducciones de `f64` a través de hilos.** Las sumas y los mínimos se hacen en un *fold*
   secuencial sobre índices ordenados, después del trabajo paralelo.
3. **Orden canónico, un solo lugar.** `(min.y, min.x, order)` para entidades y tramos — la convención
   que `boolean.rs:358-370` ya usa. Se extrae a `cam-prim` y se reusa.
4. **IDs por contenido.** `ArtifactKey`, `NodeId`, y cualquier orden estable derivan de contenido, nunca
   de dirección de memoria, ni de `Instant`, ni de un contador global.
5. **Cero reloj.** Ni presupuestos por tiempo, ni "timeout de seguridad", ni heurísticas que dependan de
   la velocidad de la máquina. Presupuesto = iteraciones (el bug de diseño de FlatCAM, evitado en I5).
6. **Tolerancia explícita.** Dos corridas con políticas distintas producen artefactos distintos; dos
   corridas con la misma política producen el mismo hash.
7. **Test de buzón:** correr el pipeline completo del corpus con `RAYON_NUM_THREADS ∈ {1,4,8}` y exigir
   G-code **byte a byte** idéntico, y repetir con cache fría y cache caliente (§4.5, I9) exigiendo el
   mismo resultado. Falla el build si diverge.

---

## 9. Documento y store

### 9.1 Schema nativo, no aditivo

Se abandona `c2d/document@2` y la migración `@2→@3` que pedía D3: **no hay documentos de madera que
importar**. Nace `camcraft/document@1`, nativo de PCB:

```
Document
├─ schema: "camcraft/document@1"
├─ board: { width, height, thickness, origin, side_up }
├─ layers: [ { id, role: LayerRole, side, color, visible, entities, source: Provenance } ]
│           role ∈ { CopperTop, CopperBottom, MaskTop, MaskBottom, SilkTop, SilkBottom, Outline, Drill, Keepout, Geometry }
├─ operations: [ { id, kind, params, inputs: [NodeRef], enabled } ]
├─ tools_ref: Option<PathBuf>
└─ frame: { origin_corner, handedness }        // documento ↔ máquina (cam-model::Frame, se conserva)
```

**El lado vive en el rol, no en un campo `side` aparte** (corrección a este boceto, tomada en M4a). Con
`role: CopperTop` y `side: Bottom` como campos independientes esa combinación es representable y no
significa nada; con el lado dentro del rol, no se puede escribir. El precio es un enum más largo, el
beneficio es que el estado incoherente no existe. `Keepout` y `Geometry` son los únicos roles que pueden
repetirse: una herramienta se mantiene fuera de varias áreas y un dibujo puede llegar en varias capas. Los
demás son singletons y un segundo los rechaza nombrando el rol.

Diferencias deliberadas con D3:

- **`LayerRole` es un campo, no metadata.** El kernel sigue conociendo una sola primitiva de área; lo
  que cambia es que el modelo deja de ser genérico.
- **`Provenance` por capa** (archivo origen, formato, fecha, hash del archivo) es campo, no diccionario.
  Es lo que permite decir "esta capa vino de `board-F_Cu.gbr`, y su geometría no coincide con el archivo"
  — el diagnóstico que FlatCAM nunca da.
- **`EntityKind`** solo crece a `Drill` y `Slot` (lo que D3 ya permitía). Todo lo demás sigue siendo
  `Region`/`Contour`. No se inventan variantes para pads ni vías: un pad es una región con provenance.

Extensión de archivo: **`.cmc`** (A2 resuelto). Contenedor: ZIP + `document.json`, igual que hoy, para
no reescribir `cam-store`. Requiere asociación de archivo e icono en Windows y filtro en los diálogos;
un `.c2d` de campilot se rechaza con "schema desconocido", no se migra.

### 9.2 Lo que se conserva de `cam-store`

El contenedor ZIP, la escritura atómica por `.tmp` + rename, la preservación de entradas no
interpretadas (`assets/…`), `validate()` en cada mutación, y sobre todo la **librería de herramientas
SQLite con escalera append-only de migraciones** (`tools.rs:29-120`) — con V-bit (ángulo + punta),
fresa de aislamiento y broca como tipos de primera clase.

### 9.3 Lo que se agrega

- `Restore defaults` del catálogo de fábrica, pero con herramientas **de PCB**: V-bits, fresas de
  aislamiento, brocas. Las 22 herramientas actuales son de madera.
- Validación de load-time que incluye: roles de capa únicos, `drill` sin entidades de área, operación
  que referencia un artefacto inexistente.

---

## 10. Estado de aplicación y undo (rediseño confirmado)

### 10.1 El problema

Snapshot del `Document` completo con 128 niveles (`cam-app/src/lib.rs:31-95,126-206`). Con geometría
genérica y documentos chicos estaba justificado. Con miles de polígonos por capa y mallas derivadas, el
costo de clonar deja de ser despreciable — exactamente el riesgo 5 de GAPS §5.

### 10.2 El diseño

**Inverso por comando, con subtrees inmutables como red de seguridad.**

```
Document (inmutable)
├─ layers: Vec<Arc<Layer>>          ← Arc: una capa sin tocar se comparte entre snapshots
│    └─ Layer { entities: Vec<Arc<Entity>> }
└─ operations: Vec<Arc<Operation>>
```

- Cada comando implementa `undo(&self, doc: &Document) -> Document` y `redo`.
- Toda `Entity`, `Layer` y `Operation` es inmutable detrás de `Arc`, así que un comando toca solo las
  rutas que cambian; el resto se comparte por puntero. Clonar un `Document` es clonar vectores de
  punteros.
- Un comando que no puede nombrar su inverso (import de capa, migración de schema) **no falla**: deja el
  `Arc<Document>` anterior en la pila. Con sharing, eso cuesta bytes, no megabytes.
- La pila guarda `(comando, etiqueta, Arc<Document>)` — la etiqueta es la que la UI muestra ("Isolation
  paso 3"). No hay re-clonado completo en ningún caso.

### 10.3 El test que lo sostiene

Además del "undo restaura el documento exactamente" que ya existe:

- **Property test sobre secuencias aleatorias de comandos**: aplicar N comandos, deshacer todos,
  comparar por igualdad estructural (incluido `ArtifactKey` de cada capa). Es el test que hace que
  "inverso por comando" sea seguro y no un acto de fe.
- **Test de compartición**: un comando que toca una entidad de una capa no debe clonar el `Vec` de las
  otras capas (se asserta con `Arc::ptr_eq` / `Arc::strong_count`).

### 10.4 Recomputación

`cam-app` no evalúa el grafo. Emite un `Document` nuevo; `cam-graph` decide qué nodos se invalidan por
`ArtifactKey`. El shell pide `evaluate(until: NodeId)` con cancelación y progreso (§4.3).

---

## 11. Oráculos

Cuatro instrumentos, cada uno con su costo y su lugar. Esta es la implementación de D2 e I1.

| Instrumento | Qué juzga | Costo | Dónde corre |
| --- | --- | --- | --- |
| **Predicado L1** (penetración) | `sweep(caminos, radio) ∩ cobre_erosionado` → ratio de penetración > ~1e-9 | barato | CI, **cada** operación geométrica |
| **Paridad geométrica** (`cam-oracle`) | conteo de polígonos, área (ε rel 1e-9), puntos tras aplanar, cobertura de cobre | medio | CI, sobre corpus versionado |
| **Volumétrico L2** (`cam-sim`) | removed / gouge / uncut / crashes, con cota de cobertura documentada | caro | hitos y UX, no cada test |
| **Golden G-code** (`cam-post`) | bytes exactos | barato | CI, por dialecto |

**Corpus** (`cam-oracle/corpus/`): 6 placas versionadas — KiCad 2 capas, KiCad 4 capas con pours,
EasyEDA, Eagle, Altium, y una grande >100×100 mm con pours. Cada una con `top/bottom/mask/silk/outline/drill`
y su baseline de FlatCAM como golden. El corpus es **el instrumento de aceptación de todo el port**, y es
lo primero que se crea (C0).

---

## 12. Performance

La promesa: NCC más rápido que FlatCAM, medido, reproducible y determinista. Cuatro palancas, todas
instaladas desde el kernel, no retrofitadas en NCC.

1. **Paralelismo por isla** con `rayon`, reducción en orden estable, ensamblado posterior (§8).
2. **Índice espacial** (`cam-index`) usado por import, booleano, NCC, simulación y picking. En el kernel
   desde C0, no en F4.
3. **Offsets incrementales y rest machining** vía artefactos (`rest` es una salida, no una idea).
4. **Medición desde el día uno**: `criterion` + baseline versionado + CI que falla ante regresión +
   baseline de FlatCAM ejecutado en entorno Python separado (los números son goldens versionados).

**Números que el diseño debe poder mostrar, y hoy no puede:** tiempo por nodo por placa, movimientos por
programa, memoria pico, speedup por `RAYON_NUM_THREADS`. Si un reporte de performance no puede nombrar
esos cuatro, no es un reporte útil.

---

## 13. Desarrollo en paralelo (requisito 3)

### 13.1 La regla que hace posible el paralelo

**Contrato primero, fan-out después.** Cada superficie compartida se mergea **antes** de que cualquier
worktree la use, como PR chico y cerrado: `cam-move` (ya existe), el trait `Node` (§4.2), el esquema del
documento (§9.1), la firma del kernel (§5.1) y la política de tolerancia (§5.2). Un worktree nunca
inventa una interfaz que otro va a usar.

### 13.2 Carriles

```
Carril      C0 ──────► C1 ──► C2 ──► C5 ──► C6
Kernel      prim    boolean offset  iso    ncc        ← camino crítico, serial hasta C2

Carril      ┌──────────────────────────────────────────────────►
Entrada     gerber ─ polarity/step-repeat ─ excellon ─ fabric ─┐
            (sólo necesita cam-prim; la polaridad necesita boolean)│

Carril      ┌──────────────────────────────────────────────────►
Modelo      schema @1 ─ store .cmc ─ tools PCB ─ validación ────┤

Carril      ┌──────────────────────────────────────────────────►
Oráculo     corpus ─ parity ─ L1 ─ CI ─ benches ─ baseline FlatCAM

Carril      ┌──────────────────────────────────────────────────►
Salida      .pp catálogo ─ autolevel ─ exc_areas ─ gerber/exc export

Carril      ┌──────────────────────────────────────────────────►
Ops         drill ─ cutout ─────────────────────────► (esperan C2)
                                                        iso/ncc/paint/panelize

Carril      ┌──────────────────────────────────────────────────►
UI/app      undo ─ view mesh cache ─ árbol ─ capas ─ formularios ─ preview
```

Observaciones que importan:

- **Taladrado y cutout no esperan al kernel.** Necesitan `cam-prim` + `cam-move` + `cam-post`, que ya
  están. Se pueden hacer en paralelo con el motor de conjuntos y dan el primer G-code real del producto
  (puerta C4) sin depender de `cam-boolean`. Es la mejor forma de tener valor entregable mientras el
  camino crítico avanza.
- **El camino crítico es serial y hay que asumirlo:** `cam-prim` (congelar) → `cam-boolean` → `cam-offset`.
  Paralelizar eso no es posible sin rehacerlo, y fingir lo contrario es cómo se produce un kernel
  inconsistente.
- **La UI arranca por el undo y el cache de mallas**, no por formularios. Los formularios necesitan
  operaciones existentes; la infraestructura no.

### 13.3 Superficies de colisión

Archivos que **no** se tocan desde dos worktrees a la vez. Si un carril los necesita, mergea un cambio
mínimo y avisa.

| Superficie | Dueño | Regla |
| --- | --- | --- |
| `Cargo.toml` del workspace | kernel (C0/C1) | los carriles agregan miembros en PRs propios, no en un PR de feature |
| `cam-model/src/lib.rs` | modelo | congelado tras C1 |
| `cam-prim` (firma pública) | kernel | congelado tras C0; cambios = versión + invalidación de cache |
| Registro de nodos (`cam-ops/src/registry.rs`) | cada op | **una línea por operación**; conflicto trivial, resolución trivial |
| `cam-cli/src/lib.rs` | salida | subcomando nuevo = archivo nuevo + una línea de registro |
| `cam-ui/src/shell/mod.rs` | UI | un carril por panel |
| `.github/workflows` | oráculo | cambios en PR propio |

### 13.4 Práctica ODD

- Un `odd/tasks/<feature>.md` por carril u operación; `L1` = la decisión arquitectónica que lo autoriza
  (referencia a la sección de este documento).
- Work-unit commits de ~400 líneas con test y doc en el mismo commit; alto riesgo en commit propio.
- El corpus y el contrato de determinismo corren en CI desde C0, así ningún carril puede regresionar a
  otro sin que se vea.
- Los worktrees se registran con la herramienta de sesión para que los cambios no commiteados se vean en
  conjunto.

---

## 14. Hitos y puertas

Prefijo `C` (camcraft). Los identificadores `E*` de ROADMAP se conservan como tareas dentro de su hito
cuando siguen siendo válidos; los que cambian se listan como delta.

| Hito | Puerta medible | Qué desbloquea |
| --- | --- | --- |
| **C0 — Fundamentos** | Barrido de borrado completo (§2.4); `cam-prim` congelado; corpus de 6 placas versionado; `cam-oracle` con paridad geométrica; cache en disco con test fría/caliente (§4.5); `criterion` + CI (fmt, clippy, test, paridad, bench); baseline FlatCAM medido; contrato de determinismo en verde | Todo el fan-out |
| **C1 — Motor de conjuntos** | 200 operaciones booleanas contra áreas conocidas con error < tolerancia; `proptest` sobre regiones aleatorias incluidas degeneradas; corpus sin errores de normalización | Gerber con polaridad, isolation, NCC, exclusiones |
| **C2 — Offset** | Offset de cobre real sin aplanado silencioso y sin rechazo; anillos multipasada sin colisión entre sí; prueba de distancia contra todo contorno original; 3 pasadas con overlap 0.5 en el corpus | Isolation, NCC, clearance envelopes |
| **C3 — Entrada y modelo** | 6 placas abren, guardan y recargan como `camcraft/document@1`; capas con rol correcto; Excellon con herramienta/diámetro/hits exactos; ninguna capa pierde geometría sin reporte | Todo el pipeline |
| **C4 — Primer G-code** | Drilling + cutout por CLI con GRBL, aceptado por `cam-sim`, en las 6 placas; dos corridas byte a byte iguales | Producto usable |
| **C5 — Isolation** | Isolation sin gouge en las 6 placas con predicado L1 en verde; V-bit con profundidad derivada verificada | El flujo que el usuario de PCB quiere primero |
| **C6 — NCC** | **NCC más rápida que FlatCAM en la placa grande**, determinista con 1/4/8 hilos, reproducible desde CI; rest machining medido | La tesis del producto |
| **C7 — Salida** | Catálogo `.pp` con golden por dialecto; autolevelling sobre warp simulado dentro de tolerancia; travel que cruza exclusión se desvía; export Gerber/Excellon con round-trip | G-code listo para máquina |
| **C8 — UI** | Flujo completo sin tocar `cam-cli`: importar, configurar, generar, simular, exportar; undo estructural con proptest; shell usable con la placa grande; preview volumétrico según §5.5 (referencia `gsim-rs`) | Producto |
| **C9 — Extensión** | Paint, panelize, film/punch según demanda | Alcance FlatCAM |
| **C10 — Endurecimiento** | 20+ placas reales en CI bajo umbral; benchmarks publicados y reproducibles; empaquetado Windows+Linux firmado; guía de migración desde FlatCAM; alfa | Publicación |

**Reglas de puerta.** Una puerta que no abre detiene el hito siguiente, no todos los carriles. La regla
de recorte es la del plan original y no cambia: **si algo se alarga, se recortan funciones, nunca la
calidad de la geometría ni los criterios de salida.**

### 14.1 Delta respecto de ROADMAP_CAMCRAFT.md

| Cambio | Motivo |
| --- | --- |
| `cam-toolpath` fuera de alcance; 11 estrategias eliminadas | Pivote a solo PCB |
| E0.1 se muda a C1 como hito propio, con `cam-boolean` como crate | Granularidad de worktree |
| **Nuevo C2 (offset)** con puerta propia | G3.2 es BLOQUEANTE y no tenía tarea |
| **Nuevo C0 incluye `cam-oracle` como crate**, no como script | El arnés de paridad es criterio de aceptación, no utilidad |
| **Índice espacial sube de E4.3 a C0/C1** | Lo necesitan import, booleano, sim, NCC y picking |
| Drilling + cutout se adelantan y se desacoplan del kernel | No dependen de `cam-boolean`; entregan G-code real antes |
| Se elimina la migración `@2→@3` y el schema aditivo | No hay documentos de madera que importar |
| Se corrige la inconsistencia de ROADMAP §2 (`E0.3`/`E0.4` mal mapeados contra la tabla) | §2 decía "E0.3 arnés de paridad, E0.4 benchmarks"; la tabla dice E0.3 política, E0.4 paridad, E0.7 benchmarks |
| Nesting, cutlist, print sheet, PDF/EPS/PNG fuera | Dominio distinto |

---

## 15. Riesgos

| Riesgo | Prob. | Impacto | Mitigación / punto de decisión |
| --- | --- | --- | --- |
| Robustez del arrangement propio ante degeneraciones | Alta | Bloquea C5/C6 | Corpus + proptest desde C0. Umbral de abandono de D1: si no cierra con 200 operaciones, se reevalúa un kernel polígono-exacto con aplanado declarado |
| **Offset no cierra sin aplanar** | Media-Alta | Isolation y NCC | Puerta C2 propia. La salida es mejorar el cleanup arrangement-based, **nunca** heurísticas dentro de la operación |
| Paralelismo rompe determinismo | Media | Promesa del producto | §8 completo; test de buzón con 1/4/8 hilos |
| NCC no alcanza a FlatCAM | Media | Tesis del producto | Baseline medido en C0; si C6 no abre, el problema es de algoritmo y hay que verlo antes de C8 |
| Booleanas con Gerber reales (la fuente de errores más frecuente de FlatCAM) | Alta | C3 y C5 | Corpus grande desde C0; es la única defensa real |
| **Superficies compartidas se pisan entre worktrees** | Media | Velocidad de desarrollo | §13.3 + contrato primero + una línea de registro por operación |
| Rediseño de `cam-app`/undo introduce regresiones invisibles | Media | Corrección del editor | Proptest de round-trip inverso (§10.3) antes de migrar comandos |
| UI con geometría grande congela el shell | Media | UX | §5.4: cache de mallas, cancelación, picking por índice |
| Cache en disco corrupta o de otra versión sirve geometría mala | Media | Corrección silenciosa | §4.5: versión de formato en la clave, sha256 al leer, miss + borrado ante cualquier duda |
| Cache real compartida entre tests los vuelve dependientes del orden | Media | Determinismo del CI | §4.5 regla 6: `CAMCRAFT_CACHE_DIR` por test o cache apagada |
| La cache crece sin control en el disco del usuario | Media | UX | §4.5 regla 5 + `cache prune`; tope por defecto a definir (A6) |
| La madera vuelve por la ventana (un "aprovechemos `pocket`") | Media | Alcance | §2.2 es una decisión, no una postergación. Reabrirlo requiere reabrir este documento |

---

## 16. Decisiones

### 16.1 Resueltas

| ID | Decisión | Consecuencia | Dónde |
| --- | --- | --- | --- |
| **A1** | DXF/SVG import **y** export, geometría general (paridad FlatCAM) | `cam-import` sobrevive; subconjunto geométrico declarado; PDF/EPS/PNG se borran | §2.3 |
| **A2** | Extensión **`.cmc`** | Asociación de archivo e icono en Windows; `.c2d` se rechaza, no se migra | §9.1 |
| **A3** | Cache de artefactos **en disco desde v1** | Nuevo subsistema: invariante I9, siete reglas, `cache info` / `cache prune`, riesgos propios | §4.5 |
| **A4** | Derivada de A1: `cam-import` es **crate propia**, `cam-fabric` no la absorbe | El import de carpeta de fabricación y el de geometría general evolucionan por separado | §6 |
| **A5** | El barrido de borrado va **ahora, antes de C0**, en 4 PRs | Primer trabajo del proyecto; `cam-ops` nace con los tres rescates | §2.4 |
| **A6** | Tope de la cache: **2 GB** por defecto, configurable por comando y por env, **no por UI** | Una preferencia más en un panel no paga su costo; `cache prune --max` es suficiente | §4.5 |
| **A7** | La cache **se comparte entre documentos** | Panelizar variantes e iterar un diseño es casi gratis; borrar un `.cmc` no libera geometría, solo `prune` | §4.5 |
| **A8** | El barrido de borrado va **antes que `cam-prim`** dentro de C0 | No depende de nada y reduce el diff de todo lo que venga después | §2.4 |

**No quedan decisiones abiertas al cierre de v0.1.**

### 16.2 Abiertas

- **A6 — Tope de la cache en disco.** Recomendación: 2 GB por defecto, configurable por comando y por
  env, no por UI (una preferencia más en la UI no paga su costo).
- **A7 — ¿La cache se comparte entre documentos?** Content-addressed ya lo permite y el beneficio es
  real (panelizar, iterar). Confirmar que es deseado, porque implica que borrar un `.cmc` no libera su
  geometría.
- **A8 — Orden dentro de C0.** Recomendación: barrido de borrado (§2.4) antes que `cam-prim`, porque no
  depende de nada y reduce el diff de todo lo que venga después.

---

## 17. Qué reemplaza este documento, y qué sigue

**Reemplaza:** el orden de fases y el mapa de crates de `ROADMAP_CAMCRAFT.md`; D3 y D4 de
`GAPS_CAMCRAFT.md` §6; el orden de las fases 1–6 de `Plan de camcraft.md` donde difieren (§14.1).

**Sigue vigente:** D1 (kernel propio, arcos preservados), D2 (aceptación híbrida), D5 (oráculo de dos
niveles), D6 (`.FlatPrj` fuera), y todo `RUST_PORT_LOGIC.md` como especificación de la lógica a portar.

**Próxima iteración de este documento, en este orden:**

1. Detallar `cam-prim`: qué tipos exactos, qué API pública, qué se congela en C0 y con qué test.
2. Traducir el barrido de borrado (§2.4) y C0–C2 en `odd/tasks/` con tareas de ~400 líneas y sus
   criterios de salida.
3. Recién entonces, tocar código.

**Cerrado en v0.1:** A1–A8 (§16.1), contrato del nodo (§4.6), encoding de artefactos (§4.7),
dependencias justificadas (§6.2).
