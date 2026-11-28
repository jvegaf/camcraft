---

## page-id: 32

# Análisis de gaps: campilot → camcraft

**Propósito.** Documentar la distancia real entre el código de campilot en este worktree y lo que
exige el [Plan de camcraft](old/Plan%20de%20camcraft.md), usando la lógica extraída de FlatCAM Plus en
[RUST_PORT_LOGIC.md](old/RUST_PORT_LOGIC.md) como especificación de referencia. Este documento es el
**diagnóstico**: las decisiones de arquitectura que se derivan están asentadas en §6 y el plan
ejecutable con roadmap vive en [ROADMAP_CAMCRAFT.md](ROADMAP_CAMCRAFT.md).

**Método.** Relevamiento read-only sobre el workspace: lectura dirigida de `Cargo.toml` y de los
módulos núcleo, y verificación cruzada por `grep` de las ausencias que este documento afirma. Toda
afirmación lleva `path:line`. Lo que no se pudo verificar en fuente está marcado **INFERIDO** o
**A VERIFICAR**.

**Dos mediciones, no una.** El relevamiento original es **historia de 2026-05-09**: midió «16 crates,
≈58k líneas en `src/`, ≈124k con tests» y esas cifras describen aquel árbol, no este. La medición
vigente es de **2026-11-28**, y cada número lleva su comando para que otro lo repita:

- `find src-tauri/crates -maxdepth 1 -mindepth 1 -type d | wc -l` → **17** crates hoy (más el paquete
  Tauri `src-tauri/`), frente a los 16 del relevamiento viejo: salieron `cam-toolpath` y `cam-program`
  (`fdb87cf`, [tarea 039](../odd/tasks/039-wood-out-sweep.md)) y entraron `cam-ops`, `cam-gerber` y
  `cam-excellon`.
- `find src-tauri/crates/*/src -name '*.rs' -type f -exec cat {} + | wc -l` → **57.786** líneas en `src/`.
- `find src-tauri/crates -name '*.rs' -type f -exec cat {} + | wc -l` → **107.014** líneas contando
  tests, benches incluidos (el «≈124k» de 2026-05-09 no es comparable: es de aquel árbol).

**Fecha del relevamiento.** La tabla del §1 y los conteos de `#[test]` de abajo son la medición de
**2026-11-28**. La de 2026-05-09 queda arriba, fechada, como historia: no se refresca en silencio.

---

## 1. Inventario del estado actual

Medición de **2026-11-28** sobre `src-tauri/crates/`: **17 crates**, **57.786** líneas en `src/`. El LOC
de cada fila es el de `find src-tauri/crates/<crate>/src -name '*.rs' -type f -exec cat {} + | wc -l`,
y el rol se verificó leyendo el `lib.rs` del crate que lo declara. La cifra «16 crates» del §Método de
2026-05-09 no describe este árbol: las dos filas que ya no aplican están al pie de la tabla.

| Crate          | LOC `src/` | Rol verificado (fuente)                                                                                                            | Estado                 |
| -------------- | ---------: | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------- |
| `cam-ui`       |     20.958 | Shell egui: paneles, canvas 2D/3D, input wiring; `handle_action` es el único punto que toca `AppState` (`src/lib.rs:1-9`)          | sigue siendo cierto    |
| `cam-geom`     |      7.780 | Kernel 2D: `Contour`, `Region`, `Arc`, chaining, offset y booleanas; mm `f64` y tolerancia explícita (`src/lib.rs:1-11`)           | sigue siendo cierto    |
| `cam-ops`      |      4.833 | Operaciones de mecanizado rescatadas de `cam-toolpath`: `follow`/profile, `peck`/drill, `rings`/pocket, más `isolation` y `ncc` (`src/lib.rs:1-4`) | cambió de forma |
| `cam-import`   |      4.484 | DXF/SVG → `Contour` mm + `ImportReport`; `folder` arma documento desde Gerber/Excellon; CSV de partes (`src/lib.rs:16-26`)          | cambió de forma        |
| `cam-cli`      |      4.246 | 16 subcomandos headless con contrato de exit codes (`src/lib.rs:230-245`)                                                          | cambió de forma        |
| `cam-store`    |      2.554 | `.cmc` (ZIP + `document.json`) y librería de herramientas SQLite con escalera de migraciones (`src/lib.rs:3-7`)                     | cambió de forma        |
| `cam-gerber`   |      2.256 | Lector Gerber RS-274X del dialecto que escribe KiCad: header, formato, aperturas, macros (`src/lib.rs:1-4`)                         | cambió de forma        |
| `cam-model`    |      2.071 | Documento `camcraft/document@1`: job, capas, entidades, toolpaths, operaciones (`src/lib.rs:27`)                                    | sigue siendo cierto    |
| `cam-app`      |      1.978 | `AppState`, `Command`, undo por snapshot acotado, selección (`src/lib.rs:1-12`)                                                     | sigue siendo cierto    |
| `cam-export`   |      1.371 | DXF (AC1015), SVG, y además Gerber/Excellon, deterministas (`src/lib.rs:1-12`, `src/gerber.rs`, `src/excellon.rs`)                  | cambió de forma        |
| `cam-post`     |      1.347 | Parser de `.pp` data-driven + emisor de G-code, con emisión fijada contra `tests/fixtures` (`src/lib.rs:1-11`)                     | sigue siendo cierto    |
| `cam-sim`      |      1.150 | Simulación por heightfield, remoción de material, `target_for`/`deviations` (gouge/uncut) (`src/target.rs:151`, `:165`)            | sigue siendo cierto    |
| `cam-view`     |      1.097 | Escena 2D/3D, cámara, mallas, picking (backend-agnóstico) (`src/lib.rs:1-12`)                                                       | sigue siendo cierto    |
| `cam-desktop`  |        619 | Host eframe + diálogos nativos `rfd` (`Cargo.toml:16-17`); se verifica compilando porque necesita display (`src/main.rs:1-8`)     | sigue siendo cierto    |
| `cam-excellon` |        503 | Parser Excellon: tabla de herramientas, coordenadas y ranuras, en mm (`src/lib.rs:1-10`)                                           | cambió de forma        |
| `cam-move`     |        311 | `Move::{Rapid,Feed,Arc,Helix,Retract}`, `Segment`, `MoveStream` (`src/lib.rs:1-6`)                                                 | sigue siendo cierto    |
| `cam-units`    |        228 | mm interno, conversión solo en frontera (`src/lib.rs:1-12`)                                                                        | sigue siendo cierto    |

**Filas de 2026-05-09 que ya no aplican.** `cam-toolpath` (3.459 líneas entonces) y `cam-program`
(652) no existen hoy en `src-tauri/crates/`: los retiró `fdb87cf`, el cuarto corte del borrado de la
[tarea 039](../odd/tasks/039-wood-out-sweep.md) (L21), que dejó su parte útil a `cam-ops`. Por eso el
conteo pasó de 16 a 17: −2 crates retirados, +3 creados (`cam-ops`, `cam-gerber`, `cam-excellon`).

**Dependencias externas del workspace** (`Cargo.toml:31-58`): `serde`, `serde_json`, `thiserror` 2,
`zip` 2, `rusqlite` 0.32, `proptest` 1, `egui`/`eframe` 0.37, `rfd` 0.15, `criterion` 0.5, `rayon` 1.
**No hay** ningún crate de geometría (`geo`, `i_overlay`, `clipper2`, `kurbo`, `lyon`,
`cavalier_contours`, `parry`), **ni** índice espacial (`rstar`, `kdtree`): verificado con
`grep -rnE '^(geo|i_overlay|clipper2|kurbo|lyon|cavalier_contours|parry|rstar|kdtree)[[:space:]]*=' --include=Cargo.toml .`
sobre todo el repositorio, cero coincidencias. Dos afirmaciones de 2026-05-09 ya no aplican: **sí hay**
`rayon` (`Cargo.toml:58`, usado en `src-tauri/crates/cam-ops/Cargo.toml:16`) y `flate2` salió del
workspace en `579194b` (2026-11-24), que también sumó `criterion` para los benches.

**Testing:** `grep -rn '#\[test\]' src-tauri/crates --include='*.rs' | wc -l` → **1.395** atributos
`#[test]` (el «≈1.687» de 2026-05-09 ya no mide este árbol). Las tres ausencias que el relevamiento
viejo afirmaba hoy son **«ya no aplica»**, cada una con su commit: **sí hay** `benches/`
(`src-tauri/crates/cam-geom/benches/kernel.rs`, `src-tauri/crates/cam-ops/benches/`, con baselines
criterion commiteados; `cfb7feb`, 2026-11-13), **sí hay** CI (`.github/workflows/ci.yml`; `65d1499`,
2026-11-13) y **sí hay** corpus de paridad (`fixtures/corpus/` con seis tableros, `parity.json` y
provenance; `1e7cb07`, 2026-11-13, más el arnés `scripts/parity/run.sh`). Hallazgo del barrido, no
corregido acá: ese arnés quedó apuntando a la ruta previa a la migración —
`scripts/parity/measure/Cargo.toml:10-12` declara `path = "../../../crates/cam-*"` y no existe
`crates/` en la raíz (los crates viven en `src-tauri/crates/`). El gate local sigue siendo
`scripts/check.sh` (fmt + clippy + test), invocado desde `justfile:11-12`, y hoy se le suma el gate
documental `scripts/check-docs.sh` (`justfile:14-15`).

---

## 2. Veredicto corto

Re-medido el 2026-11-28: lo que hay es **un CAM 2.5D con el núcleo de PCB ya montado sobre un kernel
geométrico serio y propio**, no un esqueleto de PCB CAM. El proyecto **no parte de cero en nada de lo
que hace PCB a PCB** —el lector Gerber evalúa la imagen de una capa sobre el motor de conjuntos (G1.1),
el Excellon se lee y el drill sale de su tabla de herramientas (G1.2, G3.4), isolation y NCC tienen
ejecutor y comando (G3.1, G3.3), el cutout tiene tabs y mouse-bites (G3.6), el travel se desvía de las
`exc_areas` (G4.2), los escritores Gerber y Excellon existen (G4.3), y el corpus, el arnés de paridad,
los benches y el CI ya son gate (G1.4, G6.1-G6.3)—. Lo que queda no es el motor sino su **modelo y su
superficie**: la semántica Gerber no tiene IR de importación (G1.3), la placa no tiene stackup ni
espejado de bottom (G2.3), `Paint` y `Panelize` siguen siendo parámetros sin ejecutor (G3.7), no hay
autolevelling (G4.1) y la UI no genera G-code (G5.2); más dos bordes del kernel —el rechazo de
`Region::offset` (G3.2) y las primitivas de conjunto que faltan (G0.2)— y el supuesto de una sola placa
por documento.

| Veredicto               | Elementos                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Reutilizar tal cual** | `cam-units`, `cam-move` (contrato de stream), `cam-post` (parser/emisor `.pp` con tres dialectos y golden byte a byte, G4.4), `cam-app` (undo/commands), `cam-sim` y el predicado de `src-tauri/crates/cam-ops/src/penetration.rs` como oráculos, `cam-cli` (contrato headless: `isolate`, `ncc`, `drill`, `cutout`, `program`), `cam-store` (escalera de migraciones; ángulo y punta de V-bit medidos y usados, G2.5), `cam-geom` (booleanas G0.1 y `offset_contour`, que conserva arcos; el rechazo de `Region::offset` es el riesgo de abajo, G3.2), `cam-gerber` y `cam-excellon`, y los cuatro escritores de `cam-export` |
| **Adaptar**             | `cam-import` → dar a la semántica Gerber un IR propio en vez de aplanarla a contornos (G1.3); `cam-model` → stackup de placa y espejado de bottom sobre `Board`/`Layer`, que ya existen (G2.3); `cam-ui` → formularios de paint/panelize y el «Generate G-code» que hoy es sólo un hit-target (G5.2)                                                                                                                                                                                       |
| **Construir**           | Ejecutores de paint y panelize (G3.7); autolevelling (G4.1); índice espacial fuera de NCC y las primitivas de conjunto que faltan (G0.2, G0.3). Ya no se construyen: motor booleano (G0.1), isolation (G3.1), NCC (G3.3), drill Excellon (G3.4), ordenamiento por cercanía (G3.5), tabs (G3.6), ruteo de exclusiones (G4.2), escritores (G4.3), corpus y arnés (G1.4, G6.3), benches (G6.2) ni CI (G6.1)                                                                                 |
| **Riesgo de rediseño**  | El rechazo de `Region::offset` en cobre denso (G3.2); el supuesto de un solo `Board`/`Frame` por documento (G2.3); el `EntityKind` de tres variantes con 140 sitios en 41 archivos (G2.1, el tope de D3); autolevelling como retrofit del stream (G4.1)                                                                                                                                                                                                                                   |

---

## 3. Gaps por fase del plan

### Fase 0 — motor de booleanas (implícita en el plan, hoy con motor)

El plan menciona "la librería de booleanas elegida en la fase 0". Hoy el motor propio existe y está
cerrado (G0.1); lo que queda de la fase son bordes del arrangement, no el motor.

- **G0.1 — Motor booleano de regiones. Estado: «sigue siendo cierto» (cerrado; la reapertura del
  2026-11-11 la cerró `c3c1fb0`).**
  `combine(a, b, BooleanOp, TolerancePolicy) -> Result<Combined, GeomError>` sigue en
  `src-tauri/crates/cam-geom/src/boolean.rs:2265`, con `BooleanOp::{Union, Intersection, Difference}`
  (`:1298`) y `Combined::Empty` como respuesta y no como error (`:1320`). Las propiedades sobre pares
  aleatorios de regiones son hoy **siete** y no cinco
  (`src-tauri/crates/cam-geom/tests/boolean_properties.rs:147`: identidad de áreas, partición,
  conmutatividad, idempotencia, determinismo y las dos de De Morgan, con 128 casos).
  **La reapertura del 2026-11-11 y su lección siguen en pie.** Construyendo el flash de una apertura
  circular de Gerber, `combine` rechazaba con "the arrangement revisits a piece" **todo** círculo hecho
  de dos semicircunferencias que nada cortara: `dedup_pieces` (`boolean.rs:2613`) agrupaba las piezas
  por par de extremos, y las dos semicircunferencias de un mismo círculo comparten exactamente los dos
  puntos de costura, así que borraba una. El defecto era del motor y no del primitivo, y lo arregló
  `c3c1fb0`: dos piezas son la misma arista sólo si son la misma curva recorrida en el mismo sentido.
  Los generadores incluyen hoy un disco de dos arcos (`tests/boolean_properties.rs:108`, `any_disc`).
  **Límite conocido:** un disco que **toca** desde adentro el borde de una región sin cruzarlo sigue
  rechazándose a tolerancia 1e-9. Anotado, no disimulado.
  **Impacto (histórico):** isolation multipasada, unión de cobre, resolución de polaridad dark/clear,
  NCC con rest-machining y merge de capas son operaciones de conjunto, y hoy todas tienen motor.
- **G0.2 — Primitivas de conjunto que faltan. Estado: «cambió de forma».**
  El primer tercio del hallazgo dejó de aplicar: la reparación de un contorno que se cruza a sí mismo
  **ya no se rechaza**, se cura en el arrangement (`19fce08`, 2026-11-11): `Region::new`
  (`src-tauri/crates/cam-geom/src/region.rs:74`) normaliza y un contorno que se cruza se corta en sus
  propios cruces. Lo demás se re-midió: **sí hay** clipeo de una línea de raster contra la región
  (`Region::raster_spans`, `src-tauri/crates/cam-geom/src/region_offset.rs:205`, A7), pero **no hay**
  una operación general de clipeo contra ventana, y el manejo determinista de coincidencias y overlaps
  parciales sigue siendo el caso especial del arrangement (`dedup_pieces`,
  `src-tauri/crates/cam-geom/src/boolean.rs:2613`). El hallazgo es hoy un borde del kernel, no una
  ausencia de motor.
- **G0.3 — Indexación espacial. Estado: «cambió de forma».**
  Dejó de ser «nada»: `cam-ops` tiene una grilla propia —`Aabb` y `SpatialGrid` con `query_aabb`,
  `src-tauri/crates/cam-ops/src/ncc_index.rs:62`— construida para NCC por `8d0ad9d` (2026-11-21). Lo
  que sigue siendo cierto es su **alcance**: `cam-geom` y `cam-model` no tienen índice, sigue sin
  haber `rstar` ni `kdtree` en ningún `Cargo.toml` del workspace
  (`grep -rn 'rstar\|kdtree' src-tauri/crates --include='*.rs' --include='Cargo.toml'` → cero), y el
  hit-testing y el `nearest` de paint —el uso que FlatCAM le da a su STRtree, `camlib.py:8504`— no
  pasan por la grilla. Con placas grandes el costo pairwise **fuera de NCC** no escala. (El uso del
  STRtree de FlatCAM se cita desde `camlib.py:8504`, **fuente upstream de FlatCAM Plus que no está en
  este árbol**; el resumen en árbol es [RUST_PORT_LOGIC.md](old/RUST_PORT_LOGIC.md).)

### Fase 1 — Importadores

- **G1.1 — Parser Gerber. Estado: «cambió de forma» (lector cerrado, evaluación avanzada).**
  `cam-gerber` lee el dialecto medido del proyecto real (`cc78d52`) y evalúa geometría:
  `Gerber::image` (`src-tauri/crates/cam-gerber/src/lib.rs:206`) construye la imagen de una capa, con
  material dark sumando y clear restando sobre el motor de conjuntos (`c45a0f9`, `fb206c8`), y la
  polaridad es `Command::Polarity` en el flujo de comandos (`:165`, `Polarity` en `:97`) y no texto en
  `attributes` (`2026335`). Se midió qué de lo que el hallazgo listaba como faltante ya está:
  **las aperturas `O`** se construyen como flash y como trazo (`obround_region`/`obround_stroke`,
  `:534`, `:1167`), **las que nombran una macro** se expanden en el punto del flash o a lo largo del
  trazo (`macro_region`, `:926`, `:1156`), y **un `D02` dentro de `G36`/`G37`** cierra el contorno
  dibujado y empieza otro de la misma región (`:264-270`). Lo que sigue faltando está medido: el trazo
  de un `D01` **a lo largo de un arco** se rechaza (`:417`), un trazo con una plantilla que no sea `C`,
  `O` ni una macro definida se rechaza por nombre (`:1150-1153`), y `RoundRect` no tiene constructor
  (`grep -in 'roundrect' src-tauri/crates/cam-gerber/src/lib.rs` → cero coincidencias; cae en el
  rechazo por nombre). Ninguna se aproxima a otra forma. Detalle de
  [odd/tasks/036-gerber-geometry.md](../odd/tasks/036-gerber-geometry.md).
- **G1.2 — Parser Excellon. Estado: «ya no aplica».** El crate `cam-excellon` existe y lee el archivo
  que KiCad escribe (`a6e4d21`, 2026-11-11): tabla de herramientas (`Tool`,
  `src-tauri/crates/cam-excellon/src/lib.rs:36`), coordenadas —incluidas las relativas, `d26427b`—,
  ceros omitidos (`533dddc`) y ranuras `G85` con su prueba de corpus (`861d6cb`, 2026-11-14). El
  entry point es `parse` (`:146`). El trabajo no es construir el parser sino consumirlo (G3.4).
- **G1.3 — El modelo de importación no soporta la semántica Gerber. Estado: «sigue siendo cierto».**
  Se re-midió y el hallazgo se sostiene: `ImportedLayer` transporta `name` y `contours` y nada más
  (`src-tauri/crates/cam-import/src/lib.rs:93`, `:97`), e `Imported` lleva formato, capas y reporte
  (`:102`). El importador de carpeta ya consume `cam-gerber` —`parse_gerber` llama a `cam_gerber::parse`
  y a `image_reporting`— pero **aplana** el resultado a `Vec<EntityKind>` de contornos
  (`src-tauri/crates/cam-import/src/folder.rs:576`, `:583`): no hay lugar para aperturas, macro de
  apertura, polaridad dark/clear, step & repeat ni D-code semantics. Un importador Gerber con esa
  semántica necesita un IR nuevo, no un parser de contornos más.
- **G1.4 — Corpus y oráculo de paridad. Estado: «ya no aplica».** **Sí hay** corpus: seis tableros
  reales con sus capas, `parity.json` y provenance (`fixtures/corpus/`, `1e7cb07`, 2026-11-13) y el
  arnés que los mide y falla al divergir (`0061226`, `scripts/parity/run.sh`). **Sí hay** además el
  oráculo externo que el hallazgo pedía: `scripts/parity/compare-flatcam.py` compara los goldens
  contra lo que FlatCAM midió (`fixtures/baseline/`, tarea 059) con el `status` exacto y el área a
  tolerancia relativa, y `scripts/parity/README.md` documenta el contrato. La advertencia de T1 sigue
  en pie y es de infraestructura, no del hallazgo: `scripts/parity/measure/Cargo.toml:10-12` declara
  `path = "../../../crates/cam-*"` y esa raíz ya no existe.
- **Activo reutilizable:** el patrón `ImportReport`/`Skipped { construct, reason, location }`
  (`src-tauri/crates/cam-import/src/report.rs:7`, `:22`) es exactamente el "informe de importación" que
  pide el plan para Gerber y Excellon, y hoy lo llenan los dos parsers. Se hereda el diseño, no el
  trabajo.

### Fase 2 — Modelo de datos PCB

- **G2.1 — `EntityKind` con una sola variante: Estado: «ya no aplica».** El relevamiento leyó el enum con
  `Contour` como única variante; hoy es `Contour` | `Drill` | `Slot`
  (`src-tauri/crates/cam-model/src/lib.rs:303`), y las dos variantes nuevas entraron con `2c29e36`
  (2026-11-17, «complete E1.1 board model with drill/slot entities»; E1.1 es un criterio del ROADMAP y
  no tiene documento de tarea propio). Es **exactamente** el tope que fijó D3 (§6):
  «`EntityKind` crece como mucho con `Drill` y `Slot`» —alcanzado, no excedido— y la semántica PCB vive
  en **metadata**, no en variantes: `Layer.role: LayerRole` (`:274`, los seis roles de fabricación más
  keepout) y `Entity.source: Provenance` (`:297`, con `#[serde(default, skip_serializing_if)]`). El
  tope se sostiene por costo re-medido, no por falta de diseño:
  `grep -rn 'EntityKind::' src-tauri/crates --include='*.rs' | wc -l` → **140** sitios
  (82 `Contour`, 30 `Drill`, 27 `Slot`) repartidos en **41** archivos
  (`grep -rc 'EntityKind::' src-tauri/crates --include='*.rs' | grep -v ':0' | wc -l`), así que una
  cuarta variante es una edición de todo eso a la vez. Lo que la preocupación de fondo todavía cubre no
  es G2.1 sino la geometría de área con agujeros para isolation, NCC y paint: `Region` ya existe en el
  modelo y los parámetros de operación la usan (`Document.exc_areas: Vec<Region>`, `:581`), y
  `Operation` (`:523`) lleva el parámetro de cada operación (`IsolationParams` en `:421`,
  `PanelizeParams` en `:505`): isolation y NCC ya tienen ejecutor
  (`cam_ops::isolation::isolation_moves`, `src-tauri/crates/cam-ops/src/isolation.rs:638`;
  `cam_ops::ncc::ncc_moves`, `src-tauri/crates/cam-ops/src/ncc.rs:561`; alcanzables como `cam-cli
  isolate` y `cam-cli ncc`, `src-tauri/crates/cam-cli/src/lib.rs:239`, `:240`), mientras `Paint` y
  `Panelize` siguen siendo structs de parámetros sin ejecutor. Esa ausencia está registrada como
  E7.1–E7.4 y F4 en [odd/roadmap/index.md](../odd/roadmap/index.md), no acá.
- **G2.2 — `Operation` es exclusivamente de mecanizado general: Estado: «ya no aplica».** El relevamiento leyó
  el enum con `Profile, Pocket, Drilling, Chamfer, VCarve, Engrave, Fluting, Texture, Inlay, Thread,
  Moulding` y sin ninguna operación PCB. Hoy `Operation` es `Isolation(IsolationParams)` |
  `NonCopperClear(NonCopperClearParams)` | `Cutout(CutoutParams)` | `Paint(PaintParams)` |
  `Drilling(DrillingParams)` | `Panelize(PanelizeParams)`
  (`src-tauri/crates/cam-model/src/lib.rs:523`), con los structs de parámetros en `:421`, `:439`,
  `:460`, `:477`, `:494` y `:505`. Las variantes de mecanizado general se fueron con el dominio de
  madera en la [tarea 039](../odd/tasks/039-wood-out-sweep.md), que también retiró el crate
  `cam-toolpath` (`fdb87cf`), y la generación de G-code con selección de preprocesador existe en
  `cam-post` (`cam-cli postp`). Lo que resta del hallazgo no es el modelo sino el ejecutor: isolation y
  NCC lo tienen; `Paint` y `Panelize` siguen siendo parámetros sin ejecutor, y el tramo parcial de NCC
  es E4.1–E4.6 (F4). Esa ausencia está registrada como E7.1–E7.4 en
  [odd/roadmap/index.md](../odd/roadmap/index.md), no acá.
- **G2.3 — Capa de placa. Estado: «cambió de forma».** El modelo ya no es un `Job` de ancho/alto/espesor
  y nada más: `Board` (`src-tauri/crates/cam-model/src/lib.rs:117`) lleva espesor (`:123`), el origen
  de la placa con tres esquinas (`OriginCorner`, `:32`; `pub origin`, `:125`) y qué cara mira arriba
  (`BoardSideUp`, `:45`; `pub side_up`, `:127`), con `Frame` (`:63`) como único mapa documento↔máquina
  —incluida la reflexión en Y cuando el origen es `top_left` (`is_reflection`, `:109`)—, y `Layer`
  (`:270`) declara `role: LayerRole` (`:274`), `side: Side` (`:276`, `Top`/`Bottom`/`Both`) y
  `color: String` (`:278`). Eso entró con `7add080` (2026-11-11), `2c29e36` (2026-11-17, E1.1) y
  `5a95d49` (2026-11-22, E6.1). Lo que **sigue faltando** es lo que el hallazgo nombraba y la medición
  no encuentra: no hay stackup de placa (ningún conteo de cobre ni orden de capas), `BoardSideUp` sólo
  admite `Top`, y no hay espejado de bottom —la reflexión de `Frame` es del sistema de coordenadas, no
  de la geometría de una cara—. El hallazgo es más chico y más preciso, no está cerrado.
- **G2.4 — Subida de schema: Estado: «ya no aplica».** `DOCUMENT_SCHEMA = "camcraft/document@1"`
  (`src-tauri/crates/cam-model/src/lib.rs:27`) y los documentos legacy `c2d/*` se rechazan al leer: no
  queda lector legacy que construir ni subida aditiva que diseñar. Registrado en `7aeba69` (2026-11-28) y
  en D3 (§6).
- **G2.5 — Librería de herramientas. Estado: «cambió de forma» (lo que estaba A VERIFICAR quedó
  medido).** SQLite con escalera append-only de migraciones sobre
  `tool_geometry/tool_cutting_data/material/machine/tool_entity/tool_tree_entry`
  (`src-tauri/crates/cam-store/src/tools.rs:39`, con `tool_name_format` en la migración 2, `:108`).
  **Ángulo y punta de V-bit: verificados y en uso** —`tool_geometry` guarda `included_angle`,
  `flat_diameter` y `tip_radius` (`:44-63`, más `shank_diameter`/`overall_length` en la migración 3),
  `ToolKind::{VBit, Drill, IsolationMill}` los clasifica (`:160-192`) y `ToolShape::VBit`
  (`src-tauri/crates/cam-move/src/lib.rs:124`) deriva el radio a cada profundidad con `radius_at`
  (`:165`) y `rise_at_radius` (`:185`), que es lo que barre la simulación—. **Perfil de broca: no
  verificado como relación** —el ángulo de punta de una broca se guarda igual que el de un V
  (`src-tauri/crates/cam-store/src/tool_edit.rs:171`), pero `DrillingParams`
  (`src-tauri/crates/cam-model/src/lib.rs:494`) sólo lleva `cut_depth`, `peck_depth` y `dwell`: no hay
  relación diámetro/profundidad efectiva derivada. Mapear la Tools DB de FlatCAM necesita esa decisión
  de modelo, no una migración de datos.

### Fase 3 — Operaciones PCB (el núcleo, y donde está el riesgo)

- **G3.1 — Isolation. Estado: «ya no aplica».** Hay operación, comando CLI, tipo y ejecutor:
  `cam_ops::isolation::isolation_moves` (`src-tauri/crates/cam-ops/src/isolation.rs:638`),
  `IsolationParams` en el modelo (`src-tauri/crates/cam-model/src/lib.rs:421`) y `cam-cli isolate`
  (`src-tauri/crates/cam-cli/src/lib.rs:239`), con la ejecución y su aceptación por simulación entrando
  en `e987296` (2026-11-20) y la polaridad de cobre derivada del anidamiento en `7714fc1` (2026-11-25).
  Lo que el hallazgo pedía además —unión previa del cobre, multipasada con offset acumulado y la guarda
  anti-gouge— también está medido: la unión de cobre se balancea en `2c2a453` y el juez de penetración de
  D5 vive en `src-tauri/crates/cam-ops/src/penetration.rs:54` (`cut_model_overlaps_copper`, `:71`).
- **G3.2 — El offset y isolation. Estado: «sigue siendo cierto» (el aplanado está cerrado; queda el
  rechazo).** `offset_contour` sigue en `src-tauri/crates/cam-geom/src/offset.rs:78`, con fallback a
  puente recto y `OffsetWarning::MiterLimited` (`:120`). El cleanup de self-intersection no aplana
  arcos: `Region::offset` (`src-tauri/crates/cam-geom/src/region_offset.rs:123`) conserva arcos
  («Offsetting preserves arcs wherever `offset_contour` does», `:47-49`) y conserva `flattened` como
  bandera con su warning (`:79`, `:169-170`). **Lo que sigue abierto del bloqueante** es exactamente lo
  que el hallazgo dice: un offset que atraviesa la forma se rechaza
  (`GeomError::EditRefused`, `region_offset.rs:181`) en vez de devolver un resultado; también se
  rechaza un cruce entre dos fronteras antes de calcular ningún offset (`:268`). Ese es el borde que
  isolation en cobre denso todavía paga.
- **G3.3 — NCC. Estado: «ya no aplica».** Hay operación, ejecutor y comando:
  `cam_ops::ncc::ncc_moves` (`src-tauri/crates/cam-ops/src/ncc.rs:561`), `NonCopperClearParams`
  (`src-tauri/crates/cam-model/src/lib.rs:439`) y `cam-cli ncc`
  (`src-tauri/crates/cam-cli/src/lib.rs:240`). Entró con `8d0ad9d` (2026-11-21), que también trajo el
  índice (G0.3) y el rest machining. **Las palancas de performance que el hallazgo daba por ausentes también existen:** `rayon`
  es dependencia de `cam-ops` (`src-tauri/crates/cam-ops/Cargo.toml:16`) y se usa en el fill
  (`ncc.rs:141`, `par_iter`) con determinismo probado entre números de hilos
  (`src-tauri/crates/cam-ops/tests/ncc.rs:503`), el índice espacial de NCC vive en
  `ncc_index.rs:62` y el rest machining entró con `8d0ad9d`. Lo que no se midió en este barrido son los
  offsets incrementales entre pasadas — §4 es de T3 — y su borde conocido es el rechazo de G3.2.
- **G3.4 — Drilling. Estado: «ya no aplica».** El drill sale de la tabla Excellon:
  `plan_excellon_drill` agrupa por herramienta, ordena cada grupo y produce los moves
  (`src-tauri/crates/cam-ops/src/excellon_drill.rs:87`, `:84-85`), y `peck::drill` conserva el
  peck/retract y el drill bank con compensación de cabeza
  (`src-tauri/crates/cam-ops/src/peck.rs:75`). Entró con `b324e3c` (2026-11-15, W4a E2.1–E2.4) y se
  endureció en `e045c43` y `59b55dc`. La entrada ya no son sólo agujeros geométricos: hay tabla de
  herramientas, agrupado por diámetro, ranuras y ordenamiento (G3.5).
- **G3.5 — Ordenamiento por cercanía. Estado: «ya no aplica».** Hay tour heurístico, y no es
  OR-Tools: `peck.rs` documenta y ejecuta nearest-neighbour con 2-opt de presupuesto determinista
  (`src-tauri/crates/cam-ops/src/peck.rs:225`, `:230`, `:67-72`), y `excellon_drill.rs:85` lo aplica
  por grupo de herramienta. El nearest-neighbour entró con `3f1a524` (2026-11-11, el rescate del drill
  bank) y el pase de 2-opt con `b324e3c` (2026-11-15, W4a). Ya no hace falta el **A VERIFICAR** del
  hallazgo: sí hay ordenamiento por cercanía, con presupuesto fijo en vez de por tiempo.
- **G3.6 — Cutout. Estado: «ya no aplica».** Los tabs son feature de primera clase y los mouse-bites
  también: `CutoutParams` lleva `tabs: Vec<Tab>` y `tab_kind: TabKind`
  (`src-tauri/crates/cam-model/src/lib.rs:460`, `:464`, `:466`), `TabKind` en `:398` y `Tab` en `:410`;
  `cutout` y `profile` cortan con tabs y levantan sobre ellos
  (`src-tauri/crates/cam-ops/src/follow.rs:50`, `:231`), y `mouse_bite_drills` perfora las
  perforaciones de retención (`:100`, `:47-49`). Entró con `487a020` (2026-11-15) y `dc3196e`
  (2026-11-20, E2.3 con prueba de simulación). El perfil de `crates/cam-toolpath` que el hallazgo
  citaba ya no existe.
- **G3.7 — Sin paint y sin panelize. Estado: «sigue siendo cierto».** Se re-midió: `PaintParams`
  (`src-tauri/crates/cam-model/src/lib.rs:477`) y `PanelizeParams` (`:505`) existen como structs de
  parámetros y `Operation` las lleva (`:523`), pero **no hay ejecutor** en `cam-ops`
  (`grep -rn 'fn paint\|paint_moves\|panelize_moves' src-tauri/crates --include='*.rs'` no devuelve
  ninguno de operación; sólo utilidades de pintado de la UI) ni comando propio en `cam-cli`: lo único
  que las nombra es el mapa de nombres (`src-tauri/crates/cam-cli/src/lib.rs:201`, `:203`) y el
  printsheet (`src-tauri/crates/cam-cli/src/printsheet.rs:296`, `:309`). Y el nesting de piezas no
  vuelve: el barrido de [odd/tasks/039-wood-out-sweep.md](../odd/tasks/039-wood-out-sweep.md) se lo
  llevó con `moulding` y el texto, así que no hay `nest`/`nest_profile` que reutilizar. Panelizar
  placas (matriz rows×cols con spacing, `ToolPanelize.py:309` — **fuente upstream de FlatCAM Plus
  que no está en este árbol**, citada desde [RUST_PORT_LOGIC.md](old/RUST_PORT_LOGIC.md)) es **otro
  problema** que el nesting de piezas.

### Fase 4 — Post-procesadores y G-code

- **Activo más valioso del proyecto (`cam-post`).** Parser `.pp` data-driven + emisor con 20 clases de
  bloque (incluido helix), modos de arco `A`/`C`, decimales y escala, rechazo explícito de lo no
  soportado (`src-tauri/crates/cam-post/src/lib.rs`), y **golden tests byte a byte**
  (`src-tauri/crates/cam-post/tests/emit.rs`). El plan pide "portar los preprocesadores de FlatCAM al
  dialecto `.pp` como datos": la infraestructura ya está y los datos empezaron (G4.4).
- **G4.1 — Autolevelling. Estado: «sigue siendo cierto».** `Move` lleva Z por move y `Arc` es de Z
  constante (`src-tauri/crates/cam-move/src/lib.rs:53`); ni `Segment` (`:204`) ni `MoveStream` (`:227`)
  tienen mapa de alturas —sus campos son nombre, herramienta, tasas y `safe_z`/`home`—, y
  `grep -rn 'autolevel' src-tauri/crates src-tauri/src` → **cero coincidencias**. El contrato da forma
  para la transformación, pero la segmentación de arcos y movimientos largos hay que escribirla.
- **G4.2 — Ruteo alrededor de áreas de exclusión. Estado: «ya no aplica».** El concepto existe en el
  modelo (`Document.exc_areas: Vec<Region>`, `src-tauri/crates/cam-model/src/lib.rs:581`, con su
  validación en `:1428`) y el ruteo existe en `cam-ops`: `route_travel_around_exc_areas` y su variante
  con herramienta para el clearance del V-bit
  (`src-tauri/crates/cam-ops/src/exc_travel.rs:47`, `:58`), que desvía el travel y falla el que no
  puede rutear. Entró con `3d4c012` y `39c6fb3` (2026-11-21) y se endureció en `8347fd6` y `d4d4668`.
- **G4.3 — Escritores Gerber/Excellon. Estado: «ya no aplica».** `cam-export` emite los cuatro
  formatos: DXF, SVG, Gerber (`src-tauri/crates/cam-export/src/gerber.rs`) y Excellon
  (`src-tauri/crates/cam-export/src/excellon.rs`), con pruebas de round-trip, desde `a949ebe`
  (2026-11-21).
- **G4.4 — Catálogo de dialectos. Estado: «ya no aplica».** Hay tres `.pp` versionados y fijados con
  golden byte a byte: `src-tauri/crates/cam-post/posts/grbl.pp` (`2be97e3`, 2026-11-15, W4b E2.5),
  `marlin.pp` y `repetier.pp` (`bb7a342`, 2026-11-21), más el estilo de toolchange M6 (`18af779`).

### Fase 5 — Interfaz

La UI es el crate más grande del workspace (20.958 líneas en `src/`, §1) y ya tiene infraestructura
real: el estado del shell y el ribbon (`Shell`, `src-tauri/crates/cam-ui/src/shell/mod.rs:57`, con
categoría de ribbon en `:219` y bloque de capas en `:229`), el bloque de capas
(`src-tauri/crates/cam-ui/src/chrome/layers.rs:5`), editor de estrategia, modal de librería de herramientas, panel de script,
preview 3D integrado (las cuatro capas del preview, `src-tauri/crates/cam-ui/src/view3d/mod.rs:102`), cámara 2D y
picking en `cam-view`.

- **G5.1 — Capas con identidad de placa. Estado: «ya no aplica».** La capa ya es una capa de placa:
  `Layer` declara `role: LayerRole` (`src-tauri/crates/cam-model/src/lib.rs:274`) con los seis roles
  de fabricación y keepout aparte (`:177`, y el comentario de `:198` que los cuenta), cada uno con su
  color canónico (`LayerRole::color_rgb`, `:200`) y su
  etiqueta (`:213`), `side: Side` (`:276`) y `color: String` (`:278`). El rango `:208-218` que el
  hallazgo citaba ya no es donde vive `Layer`. Entró con `7add080` (2026-11-11) y se completó como
  identidad de placa en `5a95d49` (2026-11-22, E6.1). Lo que la UI haga con ese rol es de E6, no un
  hallazgo de modelo.
- **G5.2 — Formularios por operación PCB. Estado: «sigue siendo cierto» (hechos; el hueco es «generar
  G-code»).** `src-tauri/crates/cam-ui/src/shell/panels.rs` edita las seis variantes de `Operation` —
  `OperationKind::ALL` (`:522`), con su bloque por variante desde `:534`—, según E6.3 y la
  [tarea 073](../odd/tasks/073-e6-3-operation-forms.md). Lo que sigue sin formulario es «generar
  G-code»: `src-tauri/crates/cam-ui/src/chrome/app_bar.rs:153` dibuja «Generate G-code» y sólo lo
  registra como hit-target de test (`:156`), sin comando detrás; y el botón «Post…» sigue al lado sin
  comando tampoco: se dibuja con `post: Option<String> = None` y sólo se registra como hit
  (`:161-167`). La emisión con selección de preprocesador vive en la CLI (`cam-cli program --post`,
  `cam-cli postp`), así que lo que falta es el camino de la UI hacia ella.
- **G5.3 — Atajos y Ctrl+S. Estado: «ya no aplica».** El mapa de atajos existe y es explícito:
  `src-tauri/crates/cam-ui/src/shell/input.rs:308-321` lee undo/redo, `Ctrl+S`, `Ctrl+Shift+S`, delete
  y fit, con el comentario que separa Save de Save As. Entró con la
  [tarea 075](../odd/tasks/075-e6-5-shortcuts-save.md) (E6.5); el plumbing de diálogos nativos que el
  hallazgo marcaba **INFERIDO** también está (`src-tauri/crates/cam-desktop/src/main.rs:296-320`).

### Fase 6 — Endurecimiento y publicación

- **G6.1 — CI. Estado: «ya no aplica».** `.github/workflows/ci.yml` existe desde `65d1499`
  (2026-11-13) y hoy es un solo tronco con gate de unitarios y `parity`/`bench` a pedido (`a06484b`,
  `25f5a83`). `grep -n 'parity\|bench' .github/workflows/ci.yml` → jobs `parity` (`:97`) y `bench`
  (`:110`), dispatch-only por costo.
- **G6.2 — Benchmarks. Estado: «ya no aplica».** **Sí hay** `benches/` con baselines criterion
  commiteados: `src-tauri/crates/cam-geom/benches/kernel.rs` y
  `src-tauri/crates/cam-ops/benches/toolpaths.rs`, con
  `check_regression.sh` y `baselines/criterion` en cada crate. Entraron con `cfb7feb` (2026-11-13, W1c
  E0.7) y `8d0ad9d` (2026-11-21, los de NCC).
- **G6.3 — Arnés de paridad geométrica. Estado: «ya no aplica».** `scripts/parity/run.sh` mide el
  corpus y falla por divergencia con el contrato que el documento de port pide: polígonos y puntos
  exactos, área a epsilon **relativo 1e-9**, `status` exacto (`scripts/parity/README.md`), y
  `compare-flatcam.py` compara además contra el oráculo externo. Entró con `1e7cb07` y `0061226`
  (2026-11-13) y se corrigió en `9c1334d`, `e4ea77b`, `32f2fa1` y `3f1abe1`.
- **G6.4 — Empaquetado y corpus. Estado: «cambió de forma».** El corpus de placas reales existe
  (`fixtures/corpus/`, `1e7cb07`), así que la segunda mitad del hallazgo ya no aplica. El empaquetado
  ya no es «nada», pero tampoco está entregado: `src-tauri/tauri.conf.json:24-26` declara
  `bundle.active = true` con `targets: "all"` y `justfile` expone `tauri-build`, y **no hay release ni
  artefacto publicado** (`git tag` vacío y ningún job de release en `ci.yml`). Queda el acto de
  empaquetar y publicar, no el andamiaje.

---

## 4. El gap transversal: rendimiento

La premisa del pivote es "mejor rendimiento que FlatCAM". Re-medido el 2026-11-28: el proyecto ya no
tiene **cero** de las cuatro palancas, pero ninguna de las que entraron es del kernel. La columna de
2026-05-09 conserva la afirmación vieja; el estado de cada fila es uno de los tres del §3, y cada
cifra lleva el comando que la produjo.

| Palanca                                               | 2026-05-09 (historia)                                                                           | Evidencia y medición de 2026-11-28                                                                                                                                                                                                                                                                                                                                                                                              | Estado           |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- |
| Paralelismo                                           | Ausente: «Sin `rayon` en ningún `Cargo.toml`; cero usos en `cam-geom`, `cam-toolpath`, `cam-sim`» | `rayon` está en el workspace (`Cargo.toml:58`) y en `cam-ops` (`src-tauri/crates/cam-ops/Cargo.toml:16`), con **un solo** punto de uso: `src-tauri/crates/cam-ops/src/ncc.rs:18`, `:141` (`par_iter` sobre las islas del relleno NCC, `8d0ad9d`). `cam-geom` y `cam-sim` siguen sin ningún uso (`grep -rn 'rayon' src-tauri/crates --include='*.rs'` → 4 líneas, todas en `cam-ops`). Medido: el bench de tres islas **no gana con hilos** (3,72 ms a 1 hilo vs 3,83 ms a 16). `cam-toolpath` no existe (§1).                                            | cambió de forma  |
| Índice espacial                                       | Ausente: «Sin `rstar`/`kdtree`; sin grilla propia»                                                | `rstar`/`kdtree` siguen sin entrar (`grep -rn 'rstar' --include=Cargo.toml .` y `grep -rn 'kdtree' --include=Cargo.toml .` → sin coincidencias, código 1), pero **sí hay grilla propia**, y una por operación: `SpatialGrid` en `src-tauri/crates/cam-ops/src/ncc_index.rs:62` (E4.3, `8d0ad9d`) y `SegmentGrid` en `src-tauri/crates/cam-geom/src/boolean.rs:1368` («local to a single boolean operation», `b6f6643`), más la grilla de vértices de `boolean.rs:925` (`139f299`). No hay índice compartido ni uso del mismo en `cam-model`. Medido en el bench del propio índice: 200 consultas AABB sobre el relleno NCC, **83,9 µs** recorriendo todos los segmentos vs **16,5 µs** con la grilla. | cambió de forma  |
| Algoritmos preparados para rest/offsets incrementales | Parcial: «Anillos de offset existen en `pocket`, sin acumulación multipasada ni rest entre herramientas» | Los anillos siguen en `pocket` (`src-tauri/crates/cam-ops/src/rings.rs:1-2`, `:90`) y ahora hay además ley de offset multipasada acumulada en isolation (`src-tauri/crates/cam-ops/src/isolation.rs:5-8`; `isolation_passes`, `:236`; `e91882b`) y rest machining de NCC (`src-tauri/crates/cam-ops/src/ncc.rs:345`; `ncc_rest_region`, `:349`; `8d0ad9d`), con su prueba en `src-tauri/crates/cam-ops/tests/ncc.rs:575`. Sigue siendo **por operación**: no hay un contrato de rest fuera de NCC ni de offset multipasada fuera de isolation. | cambió de forma  |
| Medición                                              | Ausente: «Sin `benches/`, sin CI, sin baseline»                                                  | `benches/` con baselines criterion comprometidos entró con `cfb7feb` (2026-11-13): `src-tauri/crates/cam-geom/benches/kernel.rs` (2 benches) y `src-tauri/crates/cam-ops/benches/toolpaths.rs` (5, los dos de NCC desde `8d0ad9d`, 2026-11-21); el CI entró con `65d1499` (2026-11-13). Límites medidos: el job `bench` es un placeholder que **no corre ningún bench** (`.github/workflows/ci.yml:110-121`), el baseline comprometido cubre 5 de los 7 benches —los dos de NCC no lo tienen— y `src-tauri/crates/cam-ops/benches/check_regression.sh:6` copia el baseline a `src-tauri/target/criterion` mientras cargo lee `target/criterion`, así que el script **aborta** sin comparar. | ya no aplica     |

**Qué mide el arnés y qué no.** `kernel.rs` mide dos operaciones del kernel sobre dos rectángulos de
20×10 mm y un círculo de 10 mm; `toolpaths.rs` mide profile, pocket y drill sobre geometría fija y el
relleno NCC sobre una placa de 100×100 mm con tres islas de cobre, más la consulta AABB ingenua
contra la que usa la grilla. **No** mide un Gerber real de una placa, ni isolation sobre cobre
denso, ni la simulación por heightfield, ni el undo por snapshot del §5. Y el único número de este
documento contra FlatCAM no sale de estos benches sino de
[odd/reports/f4-ncc-benchmark.md](../odd/reports/f4-ncc-benchmark.md), que sí tiene un comando
repetible: `cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours --
--ignored --nocapture`.

**Medición de 2026-11-28** (`nproc` → **16**; las cifras son el intervalo de confianza de criterion,
con la mediana en el medio):

- `cargo bench -p cam-geom --bench kernel` → `geom/combine_union_rect_overlap` 66,5–69,6 µs
  (mediana 67,9 µs); `geom/offset_circle` 292,6–303,4 µs (mediana 297,5 µs).
- `cargo bench -p cam-ops --bench toolpaths` → `ops/profile_rectangle` 814 ns;
  `ops/pocket_offset_rectangle` 31,6 µs; `ops/drill_6_holes` 1,14 µs; `ops/ncc_fill_rings` 4,66 ms;
  `ops/ncc_index_naive_aabb` 83,9 µs; `ops/ncc_index_grid_aabb` 16,5 µs.
- `RAYON_NUM_THREADS=1 cargo bench -p cam-ops --bench toolpaths -- ncc_fill_rings` → 3,72 ms;
  `RAYON_NUM_THREADS=16 …` → 3,83 ms (criterion contra la corrida de un hilo: sin cambio, p = 0,06).
  Ninguna de las dos lecturas multihilo bajó de la secuencial: el paralelismo está instalado y, en
  este bench, todavía no compra tiempo.
- `cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored
  --nocapture`, tres corridas → `total_ms` **7260,08**, **7120,70** y **7101,21** (`unite_ms` ≈ 6100,
  `clip_ms` ≈ 1000). El reporte de F4 registró **2088,64 ms** para la misma placa y el mismo comando
  el 2026-11-16, así que hoy la operación está ~3,4× por encima de su propia cifra y el «~23,9× más
  rápido que FlatCAM» del reporte no se sostiene como está escrito: contra los 49,87 s de FlatCAM
  medidos entonces serían ~7×, y ese 49,87 s es **historia** (FlatCAM-Plus no está en este árbol, no
  se re-midió). Registrado, no corregido (S7).
- `./src-tauri/crates/cam-geom/benches/check_regression.sh` → `panicked … Baseline 'committed' must
  exist before comparison is allowed`; `./crates/cam-ops/benches/check_regression.sh` (la ruta que
  documentan los `README.md` de `benches/`) → `No such file or directory`, código 127, porque esa
  ruta es la anterior a la migración a `src-tauri/`.

Es importante decirlo con precisión: **el código medido no es lento porque esté mal escrito; lo que
falta es alcance.** Un combine de dos rectángulos cuesta decenas de microsegundos y un pocket de
60×40 mm, 31 µs; el único relleno que recorre una placa entera (100×100 mm con tres islas) cuesta
milisegundos, y la placa grande del corpus, segundos. El riesgo real no es la performance de hoy, es
que las palancas que existen viven **dentro de una operación**: un `par_iter` sobre islas de NCC, una
grilla que se reconstruye en cada llamada al kernel y un rest que sólo NCC conoce. Eso es lo que el
diseño (kernel de booleanas, modelo de entidades, simulación por celdas) tiene que sostener antes de
que una placa real con miles de polígonos de Gerber pase por acá.

---

## 5. Riesgos arquitectónicos (fuerzan rediseño, no solo código nuevo)

1. **El kernel de booleanas/offset es el cimiento, y hoy es parcial.** Construir isolation y NCC
   sobre `normalize` + `subtract` + offset que aplana arcos lleva a un producto que pierde exactitud
   justo donde el PCB la necesita. La decisión "kernel propio vs. `i_overlay`/`clipper2`" tiene que
   tomarse **antes** de la fase 1, porque el importador Gerber debe emitir en el formato que el
   kernel consume.
2. **`EntityKind` — sumar una cuarta variante es una edición transversal.** Las variantes de hoy
   son `Contour`, `Drill` y `Slot` (`cam-model/src/lib.rs:303`) y ahí está el tope de D3 (§6): la
   semántica PCB —rol de capa, apertura, herramienta, archivo de origen— vive en metadata
   (`Layer.role: LayerRole`, `:274`; `Entity.source: Provenance`, `:297`), no en variantes. Agregar
   una cuarta toca `cam-model`, `cam-app`, `cam-import`, `cam-export` (DXF, SVG, Gerber y Excellon),
   `cam-ui` y `cam-view` a la vez: 142 sitios de `match`/construcción en 41 archivos. Esa es la
   razón por la que las pseudo-variantes —pads, aperturas, pours— **no** se modelan como
   `EntityKind`.
3. **`Arc` de Z constante y moves de Z fijo.** Autolevelling obliga a segmentar el stream; si se
   posterga mucho, aparece como retrofit en todas las estrategias.
4. **Un solo `Job`/`Frame` centrado en el documento.** Panelizado y registro de placa (múltiples
   orígenes, espejado, matriz) ponen presión sobre un contrato que hoy mapea 1:1 documento↔máquina
   (`cam-model/src/lib.rs:57-88`). **INFERIDO** pero con consecuencias de diseño.
5. **Undo por snapshot del `Document` completo** (`cam-app/src/lib.rs:31-95,126-206`). Con 128
   niveles y documentos PCB con miles de polígonos y buffers derivados, el costo de clonar y
   validar deja de ser despreciable. **INFERIDO:** hoy está justificado por tamaño de documento; el
   pivote lo cambia.
6. **Fragmentación de tolerancias.** Hay `DEFAULT_TOLERANCE = 1e-6` y `MAX_TESSELLATION_STEPS`
   (`cam-geom/src/lib.rs:52-58`), más `chord 0.001` en offset de contorno (`offset.rs:26-43`) y en
   `region_offset.rs:62-74`, más tolerancias pasadas explícitamente por casi toda la API. No hay una
   política única. Para paridad geométrica y determinismo hace falta una.

---

## 6. Decisiones tomadas

Confirmadas el 2026-05-09. Cada una registra el hecho que la decide, porque ese hecho es lo que
hay que revisar si alguna vez se quiere reabrir.

- **D1. Motor booleano — extender el kernel propio.**
  Hecho decisivo: `boolean.rs` (596 líneas) ya es un arrangement completo que corta por fracciones,
  clasifica el lado material con probing y **preserva arcos** (`cut_fractions:152`,
  `material_side:306`, `assemble:391`, `survives_the_bulge:588`). Un motor booleano es ese mismo
  arrangement seleccionando tramos con otro predicado: unión = algún input tiene material,
  intersección = todos, diferencia = A sí y B no. Alternativa descartada: `i_overlay`/`clipper2`
  operan solo sobre polígonos de líneas y obligarían a aplanar arcos, que son first-class de punta a
  punta (`contour.rs:8-22`, `cam-post/src/lib.rs:112-177`).
  _Trabajo:_ capa de selección de tramos, normalización de salida y robustez ante degeneraciones
  (bordes coincidentes, vértices de contacto, contornos auto-intersectados hoy rechazados en
  `region.rs:162-165`). `proptest` ya está en el workspace.
  _Umbral de abandono:_ si el arrangement propio no pasa un corpus de 200 operaciones booleanas
  contra áreas conocidas, reevaluar `i_overlay` con aplanado explícito.

- **D2. Criterio de aceptación — híbrido por capa, no un criterio único.**
  Hecho decisivo: FlatCAM ordena taladros y paint con OR-Tools con presupuesto de tiempo
  (`camlib.py:3040,3104`), así que dos corridas suyas no emiten el mismo G-code. La paridad byte a
  byte es imposible por diseño, no por limitación nuestra.
  Importación → paridad geométrica (conteo de polígonos, área con ε relativo 1e-9, puntos tras
  aplanar). Operaciones → corrección geométrica (cubre el mismo cobre, sin gouge). Emisión → golden
  propio byte a byte, como ya hace `cam-post/tests/emit.rs`.

- **D3. Modelo de placa — schema `camcraft/document@1`, sin contenedor nuevo.**
  Hecho decisivo: el kernel conoce una primitiva de área (`Region`) y una de camino (`Contour`), y
  `Job` + `Frame` ya modelan dimensiones, espesor, origen y mapeo documento↔máquina
  (`cam-model/src/lib.rs:57-88`). Un contenedor paralelo duplicaría carga/guardado/undo.
  _Regla:_ la semántica PCB (apertura, polaridad, herramienta, rol de capa, archivo de origen) vive
  en **metadata**, no en variantes de `EntityKind` — el mismo principio con el que FlatCAM aplana a
  shapely y guarda aperturas y herramientas aparte. `Operation` sí crece con `Isolation`,
  `NonCopperClear`, `Cutout`, `Paint` y `Panelize`. `EntityKind` crece como mucho con `Drill` y
  `Slot`. Decisión: **c2d está obsoleto**; el schema en vigor es `camcraft/document@1` en
  contenedor `.cmc` (`cam-model/src/lib.rs:27`), los documentos legacy `c2d/*` se rechazan al leer
  y no hay migración: nada que migrar.

- **D4. Dominio general — conservar, congelar, sacarlo de la UI principal.**
  Hecho decisivo: el core de las 11 operaciones es barato (~1.900 líneas fuera de `lib.rs`), pero
  cada variante de `Operation` es un formulario de UI y un bloque de tests. El costo está en la
  superficie, no en el algoritmo. Cero inversión nueva en madera; funcional en core y CLI; movido a
  un modo avanzado de la UI. `medial.rs` (1.038 líneas) no se toca: es activo compartido.

- **D5. Oráculo de la fase 3 — dos niveles.**
  `cam-sim` sirve a nivel de celda (`target_for` `target.rs:217`, `deviations` `:1632`) pero es caro
  a resolución de clearance PCB. En CI corre un predicado geométrico rápido portado de FlatCAM
  (`_cut_model_overlaps_copper`: buffer de caminos por radio de herramienta ∩ cobre erosionado,
  ratio de penetración > ~1e-9); `cam-sim` queda como juez volumétrico en la UX y en los hitos.
  Depende de D1: el predicado es una intersección de regiones.

- **D6. Leer `.FlatPrj` — diferido, fuera de alcance.**
  Hecho decisivo: el formato comprime con XZ/LZMA en su ruta opcional (`appIO.py:2691`), que el
  workspace no puede descomprimir hoy, y migrar derivados heredaría la geometría de la versión de
  shapely del usuario — justo lo que este proyecto quiere mejorar. El flujo real de migración es
  reabrir los Gerber originales. En su lugar: importador de **carpeta de fabricación** (top, bottom,
  mask, silk, outline, drill) que es el patrón real de KiCad/EasyEDA/Altium.

---

## 7. Lo que NO es un gap

Para que el plan no rehaga lo que ya está:

- **Emisión de G-code y dialectos:** `cam-post` con `.pp` data-driven y golden tests byte a byte es
  más de lo que FlatCAM tiene como estructura, y es la fase 4 casi resuelta en infraestructura.
- **Contrato `MoveStream`:** `Move::{Rapid,Feed,Arc,Helix,Retract}` + `Segment` con forma de
  herramienta y tasas (`cam-move/src/lib.rs:42-324`) es un punto de extensión limpio para
  autolevelling, y aísla a las estrategias del emisor.
- **Informe de importación:** `ImportReport`/`Skipped` con motivo y ubicación ya cumple el "nada se
  descarta en silencio" del plan.
- **Determinismo de base:** mm interno con conversión solo en frontera (`cam-units/src/lib.rs:1-24`),
  DXF/SVG deterministas, IDs canónicos, ordenamiento por `(min.y, min.x, order)` en el chaining
  (`cam-geom/src/boolean.rs:358-370`) — la cultura de determinismo existe y es un activo para la
  paridad.
- **Superficie headless:** `cam-cli` con 15 subcomandos, `script` y exit codes documentados
  (`cam-cli/src/lib.rs:45-239,323-354`) permite desarrollar y testear toda la fase 3 sin UI.
- **Simulación:** `cam-sim` con heightfield, `target_for` y `deviations` es un oráculo funcional
  para validar operaciones (con las salvedades de D5).

---

## 8. Resumen de severidad

| ID   | Gap                                             | Severidad      | Bloquea                    |
| ---- | ----------------------------------------------- | -------------- | -------------------------- |
| G0.1 | Sin motor booleano de regiones                  | **BLOQUEANTE** | Fases 1, 2, 3              |
| G3.2 | Offset que aplana/rechaza                       | **BLOQUEANTE** | Isolation, NCC             |
| G1.1 | Sin parser Gerber                               | **BLOQUEANTE** | Todo el flujo PCB          |
| G1.2 | Sin parser Excellon                             | **BLOQUEANTE** | Drilling, cutout           |
| G2.1 | `EntityKind` cubre `Contour`, `Drill`, `Slot`   | —              | Nada: lo cerró D3          |
| G3.3 | Sin NCC y sin palancas de performance           | **BLOQUEANTE** | La promesa del producto    |
| G2.2 | `Operation` cubre las operaciones PCB           | —              | Nada: E7.1–E7.4 lo cubre   |
| G3.1 | Sin isolation                                   | ALTO           | Hito 2 del plan            |
| G4.1 | Sin autolevelling                               | ALTO           | Fase 4                     |
| G4.2 | Sin ruteo de exclusiones                        | ALTO           | Seguridad física           |
| G3.4 | Drilling sin semántica Excellon                 | ALTO           | Fase 3                     |
| G6.3 | Sin arnés de paridad                            | ALTO           | Aceptación de todo el port |
| G2.3 | Sin capa de placa/stackup                       | ALTO           | Fase 2                     |
| G0.3 | Sin índice espacial                             | MEDIO          | Performance                |
| G3.6 | Tabs de perfil no verificados                   | MEDIO          | Cutout                     |
| G3.7 | Sin paint / panelize                            | MEDIO          | Fase 3 tardía              |
| G4.3 | Sin escritores Gerber/Excellon                  | MEDIO          | Panelize, film, punch      |
| G6.1 | Sin CI                                          | MEDIO          | Fase 6                     |
| G6.2 | Sin benchmarks                                  | MEDIO          | Promesa de performance     |
| G5.1 | Capas sin identidad de placa                    | MEDIO          | Fase 5                     |
| G2.5 | Librería de herramientas a verificar para V-bit | BAJO           | Fase 2                     |
| G5.3 | Atajos / Ctrl+S a verificar                     | BAJO           | Fase 5                     |

---

## 9. Próximo paso

El diagnóstico está cerrado y las decisiones D1–D6 están tomadas (§6). El plan ejecutable con fases,
criterios de salida medibles, benchmarks y roadmap de hitos vive en
[ROADMAP_CAMCRAFT.md](ROADMAP_CAMCRAFT.md), que reordena las fases de `Plan de camcraft.md` para que
el motor de conjuntos (G0.1), el arnés de paridad (G6.3) y los benchmarks sean **fase 0 explícita**
en lugar de supuesto.
