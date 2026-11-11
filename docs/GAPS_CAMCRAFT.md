# Análisis de gaps: campilot → camcraft

**Propósito.** Documentar la distancia real entre el código de campilot en este worktree y lo que
exige el [Plan de camcraft](Plan%20de%20camcraft.md), usando la lógica extraída de FlatCAM Plus en
[RUST_PORT_LOGIC.md](RUST_PORT_LOGIC.md) como especificación de referencia. Este documento es el
**diagnóstico**: las decisiones de arquitectura que se derivan están asentadas en §6 y el plan
ejecutable con roadmap vive en [ROADMAP_CAMCRAFT.md](ROADMAP_CAMCRAFT.md).

**Método.** Relevamiento read-only sobre el workspace (16 crates, ≈58k líneas en `src/`, ≈124k con
tests), lectura dirigida de `Cargo.toml` y de los módulos núcleo, y verificación cruzada por `grep`
de las ausencias que este documento afirma. Toda afirmación lleva `path:line`. Lo que no se pudo
verificar en fuente está marcado **INFERIDO** o **A VERIFICAR**.

**Fecha del relevamiento.** 2026-05-09, estado del worktree tal cual (sin commit de este documento).

---

## 1. Inventario del estado actual

| Crate | LOC `src/` | Rol verificado |
| --- | ---: | --- |
| `cam-geom` | 9.227 | Kernel geométrico propio: `Contour`, `Region`, `Arc`, offset, normalización, medial axis, nesting, moulding, texto |
| `cam-import` | 7.363 | DXF / SVG / PDF / EPS / PNG → `Contour` en mm + `ImportReport` de fidelidad |
| `cam-toolpath` | 3.459 | Estrategias: profile, pocket, drill, chamfer, engrave, fluting, inlay, thread, vcarve, moulding |
| `cam-ui` | 19.061 | Shell egui: capas, ribbon, canvas 2D/3D, editor de estrategia, librería de herramientas, script, ayuda |
| `cam-cli` | 5.293 | 15 subcomandos headless con contrato de exit codes |
| `cam-store` | 2.516 | `.c2d` (ZIP + `document.json`) y librería de herramientas SQLite con escalera de migraciones |
| `cam-sim` | 2.506 | Simulación por heightfield, remoción de material, `target_for`/`deviations` (gouge/uncut) |
| `cam-model` | 2.424 | Documento `c2d/document@2`: job, capas, entidades, toolpaths, operaciones |
| `cam-post` | 1.176 | Parser de `.pp` data-driven + emisor de G-code, golden tests byte a byte |
| `cam-view` | 993 | Escena 2D/3D, cámara, mallas, picking (backend-agnóstico) |
| `cam-program` | 652 | Une documento + herramienta + estrategia → `MoveStream` |
| `cam-export` | 650 | DXF (AC1015) y SVG, deterministas |
| `cam-desktop` | 377 | Host eframe + diálogos nativos |
| `cam-move` | 311 | `Move::{Rapid,Feed,Arc,Helix,Retract}`, `Segment`, `MoveStream` |
| `cam-units` | 228 | mm interno, conversión solo en frontera |
| `cam-app` | 1.880 | `AppState`, `Command`, undo por snapshot, selección, cámara |

**Dependencias externas del workspace** (`Cargo.toml:27-57`): `serde`, `serde_json`, `thiserror`,
`zip`, `rusqlite`, `flate2`, `proptest`, `egui`/`eframe` 0.37, `rfd`. **No hay** ningún crate de
geometría (`geo`, `i_overlay`, `clipper2`, `kurbo`, `lyon`, `cavalier_contours`, `parry`), **ni**
`rayon`, **ni** índice espacial (`rstar`, `kdtree`). Verificado por `grep` sobre todos los
`Cargo.toml` y sobre `crates/`: cero coincidencias.

**Testing:** ≈1.687 atributos `#[test]` en `crates/`. **No hay** `benches/`, **no hay**
`.github/workflows/`, **no hay** corpus de paridad. El gate es `scripts/check.sh` (fmt + clippy +
test) invocado desde `justfile:10-37`.

---

## 2. Veredicto corto

Lo que hay es **un CAM 2.5D de mecanizado general con un kernel geométrico serio y propio**, no un
esqueleto de PCB CAM. La consecuencia más importante: **el proyecto no parte de cero en toolpaths ni
en modelo de documento, pero sí parte casi de cero en todo lo que hace PCB a PCB** — importación
Gerber/Excellon, semántica de cobre/aperturas, y el motor de operaciones booleanas sobre el que se
apoyan isolation y NCC.

| Veredicto | Elementos |
| --- | --- |
| **Reutilizar tal cual** | `cam-units`, `cam-move` (contrato de stream), `cam-post` (parser/emisor `.pp` + golden tests), `cam-app` (undo/commands), `cam-sim` (oráculo de aceptación), `cam-cli` (contrato headless y exit codes), y las primitivas de `cam-geom` como base |
| **Adaptar** | `cam-import` → sumar Gerber/Excellon sobre el mismo contrato `ImportReport`; `cam-model` → nueva versión de schema con entidades/capas PCB; `cam-toolpath` → el `pocket` por anillos de offset es la semilla de isolation; `cam-export` → sumar escritores Gerber/Excellon; `cam-ui` → capas + formularios por operación |
| **Construir** | Motor booleano de regiones (∪/∩/∖); operaciones isolation, NCC, paint, panelize; drill desde tablas Excellon; autolevelling; corpus y arnés de paridad; benchmarks |
| **Riesgo de rediseño** | Kernel de booleanas/offset; modelo de entidades (solo `Contour`); supuestos de un solo `Job`/`Frame`; performance (nada de paralelismo ni índices) |

---

## 3. Gaps por fase del plan

### Fase 0 — motor de booleanas (implícita en el plan, hoy vacía)

El plan menciona "la librería de booleanas elegida en la fase 0" pero no existe tal fase ni tal
decisión. Es el gap **más caro de todos** y conviene tratarlo como fase propia, porque condiciona
las fases 1 y 3.

- **G0.1 — No existe motor booleano de regiones.** `cam-geom` expone exactamente un operador de
  conjunto: `Region::subtract(&self, hole: &Contour)` (`crates/cam-geom/src/region.rs:152`), más la
  normalización por arrangement `normalize(region, tolerance)` (`crates/cam-geom/src/boolean.rs:99`).
  No hay `union`, `intersection` ni `difference` entre regiones arbitrarias. Verificado: `grep "pub fn"` sobre
  `boolean.rs` y `region.rs` devuelve solo esas entradas.
  **Impacto:** isolation multipasada, unión de cobre, resolución de polaridad dark/clear, NCC con
  rest-machining, subtract de exclusiones y merge de capas son todos operaciones de conjunto. Sin
  esto, la fase 3 completa está bloqueada.
- **G0.2 — Faltan primitivas de conjunto que sí existen en cualquier kernel de referencia:**
  self-intersection repair de contornos de entrada (`crates/cam-geom/src/region.rs:162-165` lo
  rechaza explícitamente), clipeo contra ventana, y manejo determinista de coincidencias/overlaps
  parciales más allá del caso especial actual (`boolean.rs:129-141`).
- **G0.3 — Nada de indexación espacial.** Verificado por `grep`: cero `rstar`/`kdtree`/grilla en
  `cam-geom` y `cam-model`. FlatCAM usa STRtree (`AppRTree`, `camlib.py:8504`) para hit-testing,
  `find_polygon` y `nearest` en paint. Con placas grandes el costo pairwise no escala.

### Fase 1 — Importadores

- **G1.1 — No hay parser Gerber.** Cero coincidencias de `gerber|excellon|rs-274x|aperture` en
  `crates/` (`grep -rniE` sobre `--include=*.rs`: vacío). El `Format` de importación y su detección
  cubren solo DXF/SVG/PDF/EPS/PNG (`crates/cam-import/src/lib.rs:46-55,72-80`).
- **G1.2 — No hay parser Excellon.** Mismo grep. No existe tabla de herramientas, ni slots, ni
  manejo de ceros omitidos, ni unidades de archivo.
- **G1.3 — El modelo de importación no soporta la semántica Gerber.** `Imported`/`ImportedLayer`
  transportan contornos en mm y contadores de fidelidad (`cam-import/src/report.rs:4-35`), pero no
  hay lugar para aperturas, macro de apertura, polaridad dark/clear, step & repeat ni D-code
  semantics. Un importador Gerber necesita un IR semántico nuevo, no un parser de contornos más.
- **G1.4 — No hay corpus ni oráculo de paridad.** Nada de comparar contra geometrías exportadas de
  FlatCAM, que es el único criterio de aceptación real de la fase 1 en el plan.
- **Activo reutilizable:** el patrón `ImportReport`/`Skipped { construct, reason, location }`
  (`cam-import/src/report.rs:24-115`) es exactamente el "informe de importación" que pide el plan
  para Gerber/Excellon. Se hereda el diseño, no el trabajo.

### Fase 2 — Modelo de datos PCB

- **G2.1 — `EntityKind` tiene una sola variante: `Contour`.**
  `crates/cam-model/src/lib.rs:230-236`. No hay entidad de taladro, pad, apertura, vía, texto de
  serigrafía ni pour. Todo lo que no sea una cadena de segmentos no se puede guardar.
- **G2.2 — `Operation` es exclusivamente de mecanizado general** (`cam-model/src/lib.rs:845-870`):
  `Profile, Pocket, Drilling, Chamfer, VCarve, Engrave, Fluting, Texture, Inlay, Thread, Moulding`.
  No hay `Isolation`, `NonCopperClear`, `Cutout`, `Paint`, `Panelize` ni operación de generación de
  G-code con selección de preprocesador.
- **G2.3 — Falta la capa de placa.** El modelo guarda un `Job` con ancho/alto/espesor y un `Frame`
  documento↔máquina (`cam-model/src/lib.rs:64-206`), pero no hay stackup, cara superior/inferior,
  espejado de bottom, origen de placa ni orientación. El plan pide exactamente eso.
- **G2.4 — Subida de schema sin diseñar.** `DOCUMENT_SCHEMA = "c2d/document@2"` con lectura legacy
  de `@1` y `migrate()` (`cam-model/src/lib.rs:26-41,1594-1611`). La `@3` que pide el plan no está
  diseñada; el precedente indica que una subida de versión es obligatoria cuando cambia la
  semántica, así que el diseño de la `@3` es trabajo real, no un bump.
- **G2.5 — Librería de herramientas: base sólida, cobertura PCB a verificar.** SQLite con escalera
  append-only de migraciones sobre `tool_geometry/tool_cutting_data/material/machine/tool_entity`
  (`cam-store/src/tools.rs:29-120`). Hay geometría de herramienta y datos de corte, lo que alcanza
  para fresas y V-bits. **A VERIFICAR:** ángulo y punta de V-bit, y perfil de broca con su relación
  diámetro/profundidad efectiva, antes de asumir que la Tools DB de FlatCAM se mapea sin migración.

### Fase 3 — Operaciones PCB (el núcleo, y donde está el riesgo)

- **G3.1 — No existe isolation.** No hay operación, ni comando CLI, ni tipo. Lo que sí existe es la
  maquinaria parcial: `pocket` calcula el inset y genera anillos de offset una vez y después corta a
  cada profundidad (`crates/cam-toolpath/src/pocket.rs:124-158,332-337`), con islas tratadas como
  fronteras de región y paridad (`pocket.rs:4-10,81-89`). Es la semilla correcta, pero le falta lo
  específico de PCB: unión previa del cobre, multipasada con offset acumulado, profundidad derivada
  del V-bit, y la guarda anti-gouge.
- **G3.2 — El offset actual no soporta isolation confiable.**
  `offset_contour` (`cam-geom/src/offset.rs:69`) hace offset por curva y resuelve vértices
  intersectando curvas soporte, con fallback a puente recto + `OffsetWarning::MiterLimited`
  (`offset.rs:246-279`); el cleanup de self-intersection **tessela, parte y se queda con el loop más
  grande, reportando `Flattened`** (`offset.rs:403-491`); y `Region::offset` puede rechazar el
  resultado si el offset atraviesa la forma (`region_offset.rs:120-185,244-344`).
  **Impacto:** en cobre denso y concavo, aclaraciones finas de PCB, este camino o pierde exactitud
  (aplana arcos) o rechaza. Es el segundo bloqueante duro después de G0.1.
- **G3.3 — No existe NCC.** Es la operación que define el rendimiento del producto y no hay nada.
  Peor: **no hay ninguna palanca de performance instalada** — `cam-toolpath/Cargo.toml` no trae
  `rayon` ni índice espacial, y `grep` sobre el crate no encuentra uso de ninguno. El plan pide
  paralelismo por isla, índice espacial, offsets incrementales y rest machining: los cuatro hay que
  construirlos.
- **G3.4 — Drilling: hay mecánica, falta semántica PCB.**
  `cam-toolpath/src/drill.rs:33-59,85-132` implementa peck/retract y un drill bank con compensación
  de cabeza, pero la entrada son agujeros geométricos (`DrillHole`), no una tabla de herramientas
  Excellon con agrupado por diámetro, ni slots, ni ordenamiento por cercanía.
- **G3.5 — Sin ordenamiento TSP por herramienta.** FlatCAM ordena los puntos de perforación con
  OR-Tools (`camlib.py:3040,3104`) con presupuesto de tiempo acotado. No hay equivalente;
  **A VERIFICAR** si algún toolpath actual ordena por cercanía para otro fin.
- **G3.6 — Cutout: el perfil existe, los tabs no están verificados.**
  `profile` está en `crates/cam-toolpath/src/lib.rs:483`. El plan asume "el perfil con tabs que ya
  existe"; la existencia de tabs/puentes como feature de primera clase **no está verificada** y es
  requisito duro de cutout PCB, junto con mouse-bites y las variantes de gaps de FlatCAM
  (`ToolCutOut.py`, §12.2 del documento de port).
- **G3.7 — Sin paint y sin panelize.** `cam-geom` tiene `nest`/`nest_profile`
  (`crates/cam-geom/src/nest.rs`, `nest_profile.rs`), que es nesting de piezas para
  aprovechamiento de material — **otro problema** que panelizar placas (matriz rows×cols con
  spacing, según `ToolPanelize.py:309`). No confundirlos al planificar.

### Fase 4 — Post-procesadores y G-code

- **Activo más valioso del proyecto (`cam-post`).** Parser `.pp` data-driven + emisor con 20 clases
  de bloque (incluido helix), modos de arco `A`/`C`, decimales y escala, rechazo explícito de lo no
  soportado (`cam-post/src/lib.rs:27-83,137-209,268-353`), y **golden tests byte a byte**
  (`cam-post/tests/emit.rs`). El plan pide "portar los preprocesadores de FlatCAM al dialecto `.pp`
  como datos": la infraestructura ya está, el trabajo es de datos.
- **G4.1 — No hay autolevelling ni gancho de compensación Z.** `Move` lleva Z fijo y `Arc` es de Z
  constante (`cam-move/src/lib.rs:55-119`); `Segment`/`MoveStream` no tienen mapa de alturas
  (`cam-move/src/lib.rs:249-324`); `cam-sim` interpreta la Z del move directamente
  (`cam-sim/src/sweep.rs:224-255`). El plan lo imagina como "transformación del move stream": el
  contrato da forma para eso, pero la segmentación de arcos y movimientos largos hay que escribirla.
- **G4.2 — No hay ruteo alrededor de áreas de exclusión.** FlatCAM desvía el travel si el segmento
  cruza un área prohibida (`Common.py:707`, usado en `camlib.py:6912-6946`), y es requisito de
  seguridad física. No hay concepto de `exc_areas` en el modelo ni en el emisor.
- **G4.3 — No hay escritores Gerber/Excellon.** `cam-export` solo emite DXF y SVG
  (`cam-export/src/lib.rs:27-114`). FlatCAM los necesita como salida (panelize, film, punching,
  exportación de taladros).
- **G4.4 — Falta el catálogo de dialectos.** No hay `.pp` para GRBL/Marlin/etc. Cero trabajo hecho,
  pero de riesgo bajo: es la parte más mecánica del port.

### Fase 5 — Interfaz

La UI es el crate más grande del workspace (19k líneas) y ya tiene infraestructura real: shell con
capas y ribbon (`cam-ui/src/shell/mod.rs:45-118`, `chrome/layers.rs:4-17`), editor de estrategia,
modal de librería de herramientas, panel de script, preview 3D integrado
(`cam-ui/src/view3d/mod.rs:98-189`), cámara 2D y picking en `cam-view`.

- **G5.1 — Faltan las capas con identidad de placa.** Hay capas genéricas (id/nombre/visible/
  entidades, `cam-model/src/lib.rs:208-218`), no capas tipadas con color por capa y modo cobre.
- **G5.2 — Faltan los formularios por operación PCB.** El editor de estrategia existe para las
  operaciones actuales; isolation/NCC/drill/cutout/paint y "generar G-code" no tienen formulario
  porque no tienen operación (depende de G2.2).
- **G5.3 — A VERIFICAR:** guardado con Ctrl+S y mapa de atajos completo, que el plan marca como
  faltantes hoy. **INFERIDO:** el desktop ya enlaza open/import/save-as vía diálogos nativos
  (`cam-desktop/src/main.rs:85-119`), así que la brecha probablemente sea de atajos, no de
  plumbing.

### Fase 6 — Endurecimiento y publicación

- **G6.1 — No hay CI.** No existe `.github/workflows/`. Solo `scripts/check.sh` + `justfile`.
- **G6.2 — No hay benchmarks.** No hay directorio `benches/` en ningún crate. Sin benchmarks no hay
  forma de sostener la promesa central del producto ("mejor rendimiento que FlatCAM") ni de
  detectar regresiones de performance.
- **G6.3 — No hay arnés de paridad geométrica.** El documento de port propone comparar número de
  polígonos, área total (ε relativo 1e-9), cantidad de puntos tras `flatten()` y texto exacto de
  G-code, sobre un corpus fijo. No existe nada de eso. Es el instrumento que convierte "parece
  bien" en un test.
- **G6.4 — No hay empaquetado ni corpus de placas reales.**

---

## 4. El gap transversal: rendimiento

La premisa del pivote es "mejor rendimiento que FlatCAM". Verificado en fuente, hoy el proyecto no
tiene **ninguna** de las cuatro palancas que esa promesa requiere:

| Palanca | Estado | Evidencia |
| --- | --- | --- |
| Paralelismo | Ausente | Sin `rayon` en ningún `Cargo.toml`; cero usos en `cam-geom`, `cam-toolpath`, `cam-sim` |
| Índice espacial | Ausente | Sin `rstar`/`kdtree`; sin grilla propia |
| Algoritmos preparados para rest/offsets incrementales | Parcial | Anillos de offset existen en `pocket`, sin acumulación multipasada ni rest entre herramientas |
| Medición | Ausente | Sin `benches/`, sin CI, sin baseline |

Es importante decirlo con precisión: **el código actual no es lento porque esté mal escrito; es
lento porque todavía no hay una sola operación PCB donde la performance importe.** El riesgo real no
es la performance de hoy, es que el diseño (kernel de booleanas, modelo de entidades, simulación por
celdas) no admita paralelizar después sin rehacerlo.

---

## 5. Riesgos arquitectónicos (fuerzan rediseño, no solo código nuevo)

1. **El kernel de booleanas/offset es el cimiento, y hoy es parcial.** Construir isolation y NCC
   sobre `normalize` + `subtract` + offset que aplana arcos lleva a un producto que pierde exactitud
   justo donde el PCB la necesita. La decisión "kernel propio vs. `i_overlay`/`clipper2`" tiene que
   tomarse **antes** de la fase 1, porque el importador Gerber debe emitir en el formato que el
   kernel consume.
2. **`EntityKind` con una sola variante.** Todo el modelo, la UI y el store asumen contornos. Sumar
   taladros, aperturas y pours toca `cam-model`, `cam-store` (schema `@3`), `cam-export` y `cam-ui`
   a la vez.
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
  *Trabajo:* capa de selección de tramos, normalización de salida y robustez ante degeneraciones
  (bordes coincidentes, vértices de contacto, contornos auto-intersectados hoy rechazados en
  `region.rs:162-165`). `proptest` ya está en el workspace.
  *Umbral de abandono:* si el arrangement propio no pasa un corpus de 200 operaciones booleanas
  contra áreas conocidas, reevaluar `i_overlay` con aplanado explícito.

- **D2. Criterio de aceptación — híbrido por capa, no un criterio único.**
  Hecho decisivo: FlatCAM ordena taladros y paint con OR-Tools con presupuesto de tiempo
  (`camlib.py:3040,3104`), así que dos corridas suyas no emiten el mismo G-code. La paridad byte a
  byte es imposible por diseño, no por limitación nuestra.
  Importación → paridad geométrica (conteo de polígonos, área con ε relativo 1e-9, puntos tras
  aplanar). Operaciones → corrección geométrica (cubre el mismo cobre, sin gouge). Emisión → golden
  propio byte a byte, como ya hace `cam-post/tests/emit.rs`.

- **D3. Modelo de placa — `c2d/document@3` aditivo, sin contenedor nuevo.**
  Hecho decisivo: el kernel conoce una primitiva de área (`Region`) y una de camino (`Contour`), y
  `Job` + `Frame` ya modelan dimensiones, espesor, origen y mapeo documento↔máquina
  (`cam-model/src/lib.rs:57-88`). Un contenedor paralelo duplicaría carga/guardado/undo.
  *Regla:* la semántica PCB (apertura, polaridad, herramienta, rol de capa, archivo de origen) vive
  en **metadata**, no en variantes de `EntityKind` — el mismo principio con el que FlatCAM aplana a
  shapely y guarda aperturas y herramientas aparte. `Operation` sí crece con `Isolation`,
  `NonCopperClear`, `Cutout`, `Paint` y `Panelize`. `EntityKind` crece como mucho con `Drill` y
  `Slot`. Migración `@2→@3` = campos nuevos con default.

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

| ID | Gap | Severidad | Bloquea |
| --- | --- | --- | --- |
| G0.1 | Sin motor booleano de regiones | **BLOQUEANTE** | Fases 1, 2, 3 |
| G3.2 | Offset que aplana/rechaza | **BLOQUEANTE** | Isolation, NCC |
| G1.1 | Sin parser Gerber | **BLOQUEANTE** | Todo el flujo PCB |
| G1.2 | Sin parser Excellon | **BLOQUEANTE** | Drilling, cutout |
| G2.1 | `EntityKind` solo `Contour` | **BLOQUEANTE** | Modelo PCB, UI, store |
| G3.3 | Sin NCC y sin palancas de performance | **BLOQUEANTE** | La promesa del producto |
| G2.2 | Sin operaciones PCB en el modelo | ALTO | UI, CLI, G-code |
| G3.1 | Sin isolation | ALTO | Hito 2 del plan |
| G4.1 | Sin autolevelling | ALTO | Fase 4 |
| G4.2 | Sin ruteo de exclusiones | ALTO | Seguridad física |
| G3.4 | Drilling sin semántica Excellon | ALTO | Fase 3 |
| G6.3 | Sin arnés de paridad | ALTO | Aceptación de todo el port |
| G2.3 | Sin capa de placa/stackup | ALTO | Fase 2 |
| G0.3 | Sin índice espacial | MEDIO | Performance |
| G3.6 | Tabs de perfil no verificados | MEDIO | Cutout |
| G3.7 | Sin paint / panelize | MEDIO | Fase 3 tardía |
| G4.3 | Sin escritores Gerber/Excellon | MEDIO | Panelize, film, punch |
| G6.1 | Sin CI | MEDIO | Fase 6 |
| G6.2 | Sin benchmarks | MEDIO | Promesa de performance |
| G5.1 | Capas sin identidad de placa | MEDIO | Fase 5 |
| G2.5 | Librería de herramientas a verificar para V-bit | BAJO | Fase 2 |
| G5.3 | Atajos / Ctrl+S a verificar | BAJO | Fase 5 |

---

## 9. Próximo paso

El diagnóstico está cerrado y las decisiones D1–D6 están tomadas (§6). El plan ejecutable con fases,
criterios de salida medibles, benchmarks y roadmap de hitos vive en
[ROADMAP_CAMCRAFT.md](ROADMAP_CAMCRAFT.md), que reordena las fases de `Plan de camcraft.md` para que
el motor de conjuntos (G0.1), el arnés de paridad (G6.3) y los benchmarks sean **fase 0 explícita**
en lugar de supuesto.
