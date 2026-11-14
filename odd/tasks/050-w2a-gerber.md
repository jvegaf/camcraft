# w2a-gerber

Complete the W2a slice of E1.4: Gerber RS-274X constructs still rejected after task 036, so corpus and real fabrication files can be imaged.

## Surfaces

- `crates/cam-gerber/`

## Specs

- **S1 (D01 fuera de región).** Un comando `D01` con apertura seleccionada fuera de `G36/G37` es un trazo: el barrido de la apertura a lo largo del segmento, aplicado con la polaridad vigente. Lo que 036 entregó para aperturas `C` y `R` se extiende a las aperturas que este slice soporta; un trazo con una apertura que este lector no sabe barrer se sigue rechazando por nombre y con el índice del comando. (W2a, waves.md)

- **S2 (apertura obround `O`).** La plantilla estándar `O` (obround, ancho × alto) se soporta en `D03` (flash, centrado en el punto) y en `D01` fuera de región (trazo). Un `O` con ancho igual a alto es un círculo; en otro caso es un estadio (rectángulo con extremos semicirculares en el eje corto). Los arcos son de primera clase: no se aplanan. (W2a, waves.md; E1.4)

- **S3 (aperturas por macro).** Una apertura cuya plantilla nombra una macro definida (`AD` con nombre de macro) se soporta en `D03` (flash) —ya entregado en 036 T10— y en `D01` fuera de región (trazo) cuando la forma de la macro puede barrerse sin aproximar arcos. Si la forma no puede barrerse con las construcciones disponibles, el trazo se rechaza por nombre. (W2a, waves.md; 036 L13)

- **S4 (aperturas RoundRect).** Las aperturas `RoundRect` —definidas como macro KiCad (`AMRoundRect` + `ADDnnRoundRect`)— funcionan como aperturas por macro (flash y, cuando aplique, trazo), cubriendo los casos del corpus. (W2a, waves.md)

- **S5 (D02 dentro de G36/G37).** Un `D02` (move) dentro de una región cierra el contorno actual y empieza otro de la misma región. Entregado en 036 T7; este slice no lo rompe y lo ejerce sobre casos del corpus cuando sea alcanzable. (W2a, waves.md)

- **S6 (sin aproximación silenciosa).** Un contorno que el lector no puede representar con fidelidad se rechaza por nombre; nunca se aplana, se simplifica ni se sustituye en silencio. (036 S4, contrato heredado)

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Flash de apertura `O` (obround): círculo cuando w==h, estadio si w!=h, centrado en el punto | S2, S6 | inline | `fdab0f1` |
| T2 | Trazo `D01` con apertura `O` fuera de región | S1, S2, S6 | inline | `fdab0f1` |
| T3 | Trazo `D01` con apertura por macro fuera de región (casos barribles) | S1, S3, S4, S6 | inline | `fdab0f1` |
| T4 | Regresión: tests de cam-gerber y verificación de que D02-en-región sigue funcionando | S5, S6 | inline | `fdab0f1` |

## Log

- **L1 (pedido, verbatim).** «A\nlee @odd/roadmap/waves.md, eres el encargado de completar w2a»

- **L2 (reconocimiento).** Rama `waves/w2a` en worktree dedicado. 036 (`fc90dda`) entregó geometría de Gerber: polaridad, regiones G36/G37, arcos nativos, flash C/R, trazo C/R, D02 dentro de región (T7), macros de apertura en flash (T10). Siguen rechazados por nombre: flash/trace de apertura `O`, trace de apertura macro, y —según W2a— D01 fuera de región para esas aperturas. Corpus (`fixtures/corpus/**/*.gbr`): 1336 flashes `O`, 2572 flashes macro, 1316 `D02` dentro de región, 8 trazos `O`, 4 trazos macro (FreePoly1/polígono). D02-en-región ya implementado (036 T7); se verifica que no se rompa.

- **L3 (T1).** Flash `O` implementado: `obround_region` construye círculo (w==h) o estadio (cápsula en el eje largo) centrado en el punto. El test de rechazo `an_o_aperture_flash_is_refused_naming_the_command_index_and_the_template` se reemplaza por tres pruebas de forma (horizontal, vertical, igual). `cargo test -p cam-gerber`: 41 passed.

- **L4 (T2).** Trazo `O` implementado: `obround_stroke` descompone el estadio en rectángulo central barrido más dos cápsulas de los extremos (suma de Minkowski sin aplanar arcos). Igualdad w==h delega a cápsula circular. El test de rechazo `an_obround_aperture_draw_is_refused_naming_the_template` se reemplaza por pruebas de barrido (horizontal, vertical, igualdad). `cargo test -p cam-gerber`: 43 passed.

- **L5 (T3).** Trazo con apertura macro: `stroke_of` resuelve la macro y `macro_stroke` barre la forma local si todos sus contornos son segmentos rectos (`sweep_linear_contour`, unión de extremos trasladados y paralelogramos por arista; aristas paralelas al barrido se omiten). Formas con aristas curvas se rechazan por nombre (sin aplanar). `a_macro_aperture_draw_is_refused_rather_than_swept` pasa a rechazar por "curved edges"; se agregan pruebas de barrido poligonal (center-line/primitive 21 y outline/primitive 4). `cargo test -p cam-gerber`: 45 passed.

- **L6 (T4, verificación y corrección de primitiva 4).** `cargo fmt -p cam-gerber` y `cargo clippy -p cam-gerber --tests -- -D warnings` limpios; `cargo test -p cam-gerber`: 45 passed. D02 dentro de región sigue cubierto por la prueba existente (036 T7) y por un caso con dos contornos. Se corrige la lectura de rotación/cierre en primitiva 4 de macros: KiCad emite `n` vértices declarados con un par de cierre repetido y la rotación después de todos los pares; antes se leía la rotación en `1+n*2` (tomando la coordenada del par de cierre) y se rechazaban flashes `RoundRect`/`FreePoly1` del corpus como rotados; ahora se detecta el par de cierre, se lee la rotación en la posición correcta y se descarta el vértice de cierre duplicado al construir el contorno. Verificación dirigida con patrones del corpus: flash y trazo `O` (`O,2.8x1.6`, `O,2.05x0.59`), flash `RoundRect` macro y trazo de macro poligonal `FreePoly1` (4 D01 del corpus) generan imagen sin error. `git status` limpio salvo la tarea 050 y su claim en el árbol principal.

- **L7 (commit).** Slice W2a commiteado en `fdab0f1` (`feat(cam-gerber): add obround and macro-aperture strokes (W2a)`, 3 archivos). Corte cohesivo T1-T4 sobre `crates/cam-gerber/` (el hook advirtió >400 líneas). Corrección menor: `clippy::int_plus_one` en primitiva 4; verificación final `cargo fmt -p cam-gerber`, `cargo clippy -p cam-gerber --tests -- -D warnings`, `cargo test -p cam-gerber` (45 passed) en verde.

- **L8 (endurecimiento posterior a la entrega, review nativo).** El review del diff acumulado marcó `R4-002`: `macro_stroke` barre cada contorno de la región macro como material y los une, así que una apertura macro con un contorno interior de vacío rellenaba el hueco. Se corrigió primero rechazando toda región multi-contorno (`a896872`), lo que el validador señaló como **REG-001** (rechazaba también exteriores disjuntos válidos); la corrección definitiva es `has_nested_boundary`: se toma un punto representativo de cada contorno y se clasifica contra los demás con `cam_geom::winding::classify`, de modo que solo se rechaza el anidamiento y los exteriores disjuntos se siguen barriendo y uniendo (`c5d71c5`). `cargo test -p cam-gerber`: 45 passed. Riesgo: `none` fuera de `crates/cam-gerber`.
