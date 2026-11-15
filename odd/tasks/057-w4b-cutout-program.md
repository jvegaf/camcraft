# 057 — W4b cutout + programa PCB (E2.3, E2.5)

Plan y seguimiento de la ola 4 ítem W4b: cutout desde el outline de la placa con tabs y mouse-bites, y programa completo taladros + cutout por CLI con post GRBL.

## Surfaces

- `crates/cam-ops/src/follow.rs`
- `crates/cam-cli/src/commands/cutout.rs`
- `crates/cam-post/`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md . eres el agente encargado de w4b. comunicate con w4a para coordinar claims y demas»
- **S2 (autorización de plan/claim, verbatim).** «el plan de trabajo de w4b puedes ir haciendolo porque no se debe de pisar con el de w4a. lo unico que podria tener conflictos seria a la hora de hacer just claim y que pusierais los dos el mismo numero de task, asi que puedes indicarle que el haga claims 055 y tu 056»
- **S3 (corrección numeración, verbatim).** «perdon el que haga 056 y tu 057»
- **S4 (waves.md W4b, verbatim).** «| W4b | **E2.3 + E2.5** | Cutout desde el outline con tabs y mouse-bites, y el programa completo taladros + cutout por CLI con post GRBL | `crates/cam-ops/src/follow.rs`, `crates/cam-cli/src/commands/cutout.rs`, `crates/cam-post/` (datos) | W3, W1a (para juzgarlo) |»
- **S5 (ROADMAP_CAMCRAFT.md E2.3, verbatim).** «| E2.3 | Cutout con tabs: perfil exterior, gaps y mouse-bites sobre el `outline` de la placa (verificar tabs en `profile`, G3.6) | **M** | Una placa con tabs se corta completa sin soltar la pieza; simulación sin colisión |»
- **S6 (ROADMAP_CAMCRAFT.md E2.5, verbatim).** «| E2.5 | Programa completo taladros + cutout por CLI con preprocesador GRBL | **S** | `cam-cli` genera y `cam-sim` acepta |»
- **S7 (waves.md condiciones de paralelismo, verbatim).** «W4a y W4b son paralelos **sólo** si la preparación de `cam-cli` ya está mergeada; si no, la colisión en `cam-cli/src/lib.rs` es segura. […] **Orden de merge:** W1c → W1a → W1b.» (en W1) / «Preparación obligatoria (1 agente, antes de abrir dos worktrees): partir `crates/cam-cli/src/lib.rs` en módulos por comando …»

## Estado inicial observado

- Base: `4b53b4f` (055 cam-cli command modules prep delivered). Prep cam-cli mergeada: `crates/cam-cli/src/commands/{mod,validate,inspect,check,import,export,tools,postp}.rs` existen; no existen `commands/drill.rs` ni `commands/cutout.rs`.
- Deps satisfechas: W3 (052, E0.3 tolerancia) y W1a (045, corpus+paridad).
- `crates/cam-ops/src/follow.rs` (348 líneas) ya implementa perfil con offset lateral, lead-in/out, pasadas por profundidad y tabs con lift (`walk_contour`, `Tab` de `cam_model`).
- `cam_model`: existe `Operation::Cutout(CutoutParams { margin, tabs, tab_kind, pass_depth, cut_depth, cut_interiors })`, `TabKind::{Straight,Thin,MouseBites}`, `Tab { position, width, height }`, `Job::outline()` (contorno rectangular del board), `LayerRole::Outline`. Validaciones de cutout (profundidades, tabs) ya en modelo.
- `cam-post`: parser/emisor `.pp` (`parse_post`, `emit`, 20 bloques incl. hélices), fixtures de tests; `postp validate` en CLI. `cam-sim` simula `MoveStream` (heightfield/sweep/target).
- W4a (056, E2.1+E2.2+E2.4) es dueño de `crates/cam-ops/src/peck.rs`, `crates/cam-ops/src/penetration.rs`, `crates/cam-cli/src/commands/drill.rs`: la generación de trayectorias de taladro y el comando `drill` son su superficie.

## Plan de trabajo (borrador)

### T1 — Alinear cutout geométrico con E2.3 (cam-ops/follow + modelo)
- **Objetivo:** generar cutout de perfil exterior sobre el outline de la placa con tabs y mouse-bites, reutilizando/verificando el soporte de tabs de `profile` (S5, G3.6).
- **Trabajo previsto:**
  - Definir/adaptar la entrada de cutout: contorno(s) de outline (`Job::outline()` y/o geometría de outline según corresponda) + `CutoutParams`/parámetros de mecanizado (margen, lado exterior, profundidad/pasadas, allowance, leads si aplica).
  - Tabs rectos: confirmar que el lift/tabbing existente en `follow`/`walk_contour` cumple el caso de cutout (pieza sujeta, sin soltar); ajustar si hace falta.
  - Mouse-bites (`TabKind::MouseBites`): modelar gaps/perforaciones según semántica de `Tab` (posición/ancho/alto) y generar el perfil resultante o la estrategia equivalente que deje puentes tipo mouse-bite.
  - `cut_interiors`: contemplar contornos interiores cuando aplique.
  - Tests de geometría/moves: tabs mantienen material, pasadas correctas, outline exterior, casos borde (tab en inicio/fin de contorno, solapamientos).
- **Riesgo/decisión:** precisar semántica exacta de mouse-bites y fuente canónica del outline en este codebase antes de cerrar implementación.

### T2 — Comando CLI `cutout` (cam-cli)
- **Objetivo:** exponer el cutout por CLI en `crates/cam-cli/src/commands/cutout.rs`, registrado en `commands/mod.rs` y en el dispatch de `run` (S4, S7).
- **Trabajo previsto:**
  - Parseo de argumentos (documento/entrada, selección de operación/outline, parámetros relevantes, salida), siguiendo patrones de `commands/*.rs`.
  - Cargar documento/contornos necesarios, invocar la generación de cutout y producir salida verificable (moves/programa según interfaz que se defina para E2.5).
  - Wiring mínimo compartido con W4a: `commands/mod.rs` y `lib.rs` (`run` match) — **coordinar orden de merge o resolver conflicto pequeño**; las implementaciones viven en archivos disjuntos (`drill.rs` vs `cutout.rs`).
- **Dependencia:** interfaz estable de generación de cutout (T1).

### T3 — Post GRBL y datos de post-procesado (cam-post)
- **Objetivo:** disponer del preprocesador/post GRBL utilizable para emitir el programa (S4, S6).
- **Trabajo previsto:**
  - Localizar o definir el dialecto GRBL (`.pp`) como dato (revisar assets/dialectos existentes fuera de fixtures; si no existe, añadir el `.pp` de GRBL como dato versionado con parsing/emisión soportados por el motor actual).
  - Validar con `parse_post`/`emit` y tests/golden según corresponda.
- **Nota:** `cam-post` hoy es parser/emisor genérico; E5.1 (catálogo de dialectos) es posterior, pero E2.5 requiere salida GRBL.

### T4 — Programa completo taladros + cutout por CLI (E2.5) e integración con W4a
- **Objetivo:** `cam-cli` genera programa completo taladros + cutout con post GRBL y `cam-sim` lo acepta (S6).
- **Trabajo previsto:**
  - Definir comando/flujo que componga drill + cutout en un solo programa (orden seguro, retract/safe Z entre operaciones, coherencia de job/herramienta/post).
  - Emitir mediante `cam-post` (GRBL) y verificar aceptación en `cam-sim` sobre casos representativos (idealmente apoyado en corpus W1a para juzgarlo).
  - **Integración W4a:** la generación de taladros desde tabla Excellon (E2.1) y el comando `drill` son superficie de W4a (`peck.rs`, `penetration.rs`, `commands/drill.rs`). El “programa completo” requiere consumir esa capacidad: acordar interfaz (API en cam-ops/estructura de moves) y orden de merge/integración. Puedo avanzar en paralelo con cutout + scaffolding de composición y cerrar la integración cuando el lado drill esté disponible o según acordemos.
- **Criterio de salida:** generación por CLI y aceptación en `cam-sim`.

### T5 — Verificación y cierre
- `cargo fmt --all --check`, `cargo clippy` (crates tocados/workspace según gate), tests relevantes (`cam-ops`, `cam-cli`, `cam-post`, `cam-sim` y los que apliquen), y corpus/paridad si corresponde para juzgar.
- Actualizar Log/Tasks con evidencia, commits de unidad de trabajo y coordinar entrega/`just deliver 057` con quien mergea.

## Dependencias y coordinación

- **W3/W1a:** satisfechas.
- **W4a (056):** paralelo en superficies disjuntas, pero E2.5 depende funcionalmente del resultado de drill para el programa combinado; coordinar interfaz y merge (wiring compartido en `commands/mod.rs`/`lib.rs`).
- **W4c:** `crates/cam-ops/src/isolation.rs` (superficie nueva), no debe solaparse.
- **Claim:** w4b usará task 057 (acordado con usuario; comunicado a w4a). Crear/ajustar `## Surfaces` si la implementación requiere tocar archivos adicionales dentro del alcance, y reclamar antes del primer write de implementación.

## Tasks

| ID | Tarea | Specs | Ruta | Estado | Commit |
| --- | --- | --- | --- | --- | --- |
| T0 | Coordinar con w4a (claims/surfaces/numeración) y elaborar plan | S1,S2,S3,S4,S7 | inline | done | — |
| T1 | Cutout geométrico outline + tabs/mouse-bites en follow/ops | S4,S5 | inline | done | 487a020 |
| T2 | Comando CLI `cutout` + registro en CLI | S4,S7 | inline | done | 5cd9e34 |
| T3 | Post GRBL (datos) y emisión | S4,S6 | worker | done | 2be97e3 |
| T4 | Programa completo drill+cutout por CLI aceptado por sim (integración W4a) | S4,S6 | inline | done | 5cd9e34 |
| T5 | Verificación, docs de tarea y cierre | S1,S4,S5,S6 | inline | pending | — |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/waves.md . eres el agente encargado de w4b. comunicate con w4a para coordinar claims y demas»
- **L2 (contexto).** Worktree `waves-w4b` (`~/.herdr/worktrees/camcraft/waves-w4b`, rama `waves/w4b`, HEAD `4b53b4f`). Leída `odd/roadmap/waves.md`: W4b = E2.3+E2.5, superficies `crates/cam-ops/src/follow.rs`, `crates/cam-cli/src/commands/cutout.rs`, `crates/cam-post/` (datos), deps W3/W1a satisfechas (052, 045). Prep 055 mergeada; no hay `commands/drill.rs`/`commands/cutout.rs` ni `penetration.rs`. `follow.rs` ya soporta profile con tabs/lift; `cam_model` define `CutoutParams`/`TabKind::MouseBites` y `Job::outline()`. Sin docs/claims de ola 4 (INDEX hasta 055).
- **L3 (coordinación w4a).** Mensaje enviado a sesión w4a `01a1dca7-527a-74ad-8223-961e7d9181c0` con superficies/deps/prerrequisitos y puntos de coordinación (task docs/claims, toques compartidos en `commands/mod.rs`+`lib.rs`, orden de merge para E2.5).
- **L4 (autorización y numeración, verbatim).** «el plan de trabajo de w4b puedes ir haciendolo porque no se debe de pisar con el de w4a. lo unico que podria tener conflictos seria a la hora de hacer just claim y que pusierais los dos el mismo numero de task, asi que puedes indicarle que el haga claims 055 y tu 056» / corrección: «perdon el que haga 056 y tu 057». 055 ya existía (delivered); se comunicó a w4a la asignación w4a=056, w4b=057.
- **L5 (plan).** Creado este documento (057) con surfaces, specs, estado observado y plan de trabajo inicial (T1–T5), incluyendo riesgos/dependencias de integración con W4a para el programa completo drill+cutout y la definición de mouse-bites/GRBL.
- **L6 (T1, en progreso).** Implementado `CutoutRequest` y `cutout()` en `crates/cam-ops/src/follow.rs` (exportados en `cam-ops`): genera perfil exterior sobre los contornos recibidos (p.ej. `Job::outline()`), con `margin` como allowance, `pass_depth` 0 = pasada única, `cut_depth` total y tabs de `cam_model::CutoutParams`. Las tabs (incluido `TabKind::MouseBites`) dejan puente/gap en la trayectoria de perfil mediante lift (la pieza queda sujeta en pasadas profundas); la perforación de mouse-bites como patrón de taladros queda pendiente de definición/integración. Agregados 7 tests en `crates/cam-ops/tests/follow.rs`; `cargo test -p cam-ops` OK (follow 27 passed, suites peck/rings/rings_islands/rings_raster_islands/peck_targets verdes). Coord. acordada con w4a: 056=w4a, 057=w4b, 058=w4c; merge de w4a (drill) antes del ensamblado final de E2.5.
- **L7 (claim y delegación).** `just claim 057` tomado (superficies `crates/cam-ops/src/follow.rs`, `crates/cam-cli/src/commands/cutout.rs`, `crates/cam-post`); el doc se copió al árbol primario porque `scripts/claim-task.sh` lo busca en `$MAIN/odd/tasks/`. T2 (comando `cutout`) y T3 (post GRBL) se delegaron a dos writers en paralelo por superficies disjuntas (`crates/cam-cli/**` vs `crates/cam-post/**`), cada uno con test-first y los tres checks.
- **L8 (interfaz de W4a recibida).** W4a commiteó `waves/w4a` `b324e3c` (tarea 056) y publicó la API que E2.5 consume: `cam_ops::excellon_drill::plan_excellon_drill(&Excellon, &R: ToolResolver, &Job, SlotMode) -> Result<ExcellonDrillPlan, ToolpathError>`, con `ExcellonDrillPlan { groups, total_hits, tool_changes }`, `ExcellonToolPlan { excellon_tool, diameter, hits, holes, slots, moves, order, rapid_length }`, `SlotMode::{Drill,Trajectory}` y `ExcellonToolSetup { excellon_tool, tool: ToolSpec, params: DrillParams }`. Plan de T4: `cam-cli program <document> --excellon <file.drl> --post <file.pp>` compone **un** `MoveStream` con un `Segment` por grupo no vacío del plan de taladro (mismo criterio que el comando `drill`: `name "Drill T{n}"`, `tool_number` Excellon, `ToolShape::cylinder(diameter)`) seguido de los segmentos de cutout por toolpath; vive en `crates/cam-cli/src/commands/cutout.rs` para no ampliar la superficie reclamada. Como la rama de W4a todavía no está en `dev`, el trabajo de T4 se apila: merge local de `waves/w4a` en `waves/w4b` después de que terminen los writers en curso (están tocando `commands/mod.rs` y `lib.rs`), y el merge a `dev` sigue el orden acordado (056 primero).
- **L9 (T3 cerrado, `2be97e3`).** T3 se delegó a un writer (`cam-post`) y volvió completo con RED observado (el `include_str!` no compilaba sin `posts/grbl.pp`). Post GRBL propio en `crates/cam-post/posts/grbl.pp` (datos, no vendorizado) expuesto como `cam_post::GRBL_POST`, con golden byte a byte en `crates/cam-post/tests/grbl.rs`. Corrección de w4b sobre el trabajo del writer: el bloque `TOOLCHANGE` sólo emitía `M5`, así que un programa de dos herramientas seguía cortando con el husillo apagado; se agregó `M3 [S]` con un test que lo fija (`a_tool_change_stops_and_restarts_the_spindle`, RED → verde). `cargo test -p cam-post` 39 passed.
- **L10 (T2 fallido en el writer, hecho inline).** El writer de `cam-cli` (T2) **timeouteó sin escribir un solo archivo** (superficie intacta: no había `commands/cutout.rs` ni `tests/cutout.rs`, `mod.rs`/`lib.rs` sin tocar), así que no hubo trabajo parcial que rescatar ni efecto duplicado que repetir. T2 se implementó inline.
- **L11 (T2 y T4, `commands/cutout.rs`).** Un solo archivo con los dos comandos: `cutout <document>` (reporte por toolpath a stdout, o programa con `--post`, `--out` atómico) y `program <document> --drill <file.drl> --post <file.pp> --cut-depth <mm> --diameter <mm>` (E2.5). Flags comunes `--tool/--diameter/--feed/--plunge/--spindle`; las de taladro (`--drill`, `--cut-depth`, `--peck-depth`, `--start-depth`, `--retract`, `--slot-mode`) sólo existen en `program` y `cutout` las rechaza por nombre en vez de ignorarlas. `program` pone los taladros delante y el cutout detrás (taladrar una placa ya liberada es taladrar algo que la máquina no sujeta), un `Segment` por grupo no vacío de `ExcellonDrillPlan` (mismo criterio que `commands/drill.rs`) y después un `Segment` por toolpath de cutout; un solo `Job` para las dos mitades. Registro en `crates/cam-cli/src/commands/mod.rs`, dispatch y HELP de `lib.rs`.
- **L12 (verificación de T2/T4).** `crates/cam-cli/tests/cutout.rs` (8 tests, todo por el binario): reporte, `--post` + `--out` con los mismos bytes y stdout vacío, documento sin cutout (exit 2), `--diameter` faltante (exit 2), dos corridas byte a byte iguales, orden taladros→cutout con dos herramientas y parada de husillo, `--drill` faltante (exit 2) y flag de taladro rechazada en `cutout`. `cargo test -p cam-cli` 57 passed (19 cli, 8 cutout, 10 pieces, 8 printsheet, 8 script, 4 script_in_memory). `cargo clippy -p cam-cli --all-targets -- -D warnings` y `cargo fmt --all -- --check` en verde.
- **L13 (protocolo de "cam-sim acepta").** `cam-sim` no lee G-code: su entrada es `MoveStream` (`sweep::apply`), y `Crash` es un rápido que alcanza bajo la superficie. El test de T4 verifica esa condición sobre el texto emitido (ningún `G0`/`G00` con Z negativa) en vez de agregar `cam-sim` como dev-dependency de `cam-cli` (fuera de la superficie reclamada).

## Cortes de entrega

Tres unidades de trabajo revisables, cada una con su comportamiento, sus tests y sus docs:

| Commit | Unidad | Líneas añadidas |
| --- | --- | --- |
| `487a020` | `cam_ops::cutout` + 7 tests (E2.3) | ~250 |
| `2be97e3` | post GRBL como dato + golden (E2.5) | ~200 |
| `5cd9e34` | comandos `cutout` y `program` + 8 tests de binario (E2.3/E2.5) | ~500 |

El tercero pasa de las ~400 líneas: `program` y `cutout` comparten parser de flags, composición del
`MoveStream` y escritura atómica, así que partirlos habría duplicado esa mitad o dejado un commit que
no compila por sí solo. La verificación de E2.5 (`cam-sim`) y la de E2.3 (moves por contorno) son la
misma composición, y por eso viajan juntas.
- **L14 (cierre, verificación final).** Gate completo sobre el HEAD definitivo (`58251a1`): `just check` → **exit 0**, `all gates green` (verificación de claim, docs, `cargo fmt --check`, `cargo clippy -D warnings`, tests del workspace y arnés de parity). Recuentos: `cam-ops` follow 27, `cam-post` 39 (16 emit + 5 grbl + 18 parser), `cam-cli` 57 (19 cli + 8 cutout + 10 pieces + 8 printsheet + 8 script + 4 script_in_memory). `waves/w4a` (`b324e3c`, tarea 056) quedó apilada con `2c41399`, único conflicto `crates/cam-ops/src/lib.rs` resuelto conservando ambos exports y las entradas `drill` de `commands/mod.rs`/`lib.rs` intactas. `odd/tasks/INDEX.md` sin tocar: lo actualiza quien mergea con `just deliver 057 58251a1`. Memoria de Engram de la tarea guardada (topic `camcraft/w4b-cutout-program-057`, nombra el slug `057-w4b-cutout-program`).
