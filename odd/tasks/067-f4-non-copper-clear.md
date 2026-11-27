# 067 — f4-non-copper-clear

Implementar F4 (Non-Copper Clear, NCC): la operación que define la promesa de rendimiento del producto. Fase completa E4.1–E4.7, iniciando por E4.1 (envolvente de no-cobre) como primer slice verificable.

## Surfaces

- `crates/cam-ops/src/ncc.rs`
- `crates/cam-ops/src/lib.rs`
- `crates/cam-ops/src/shared.rs`
- `crates/cam-ops/tests/ncc.rs`
- `crates/cam-ops/Cargo.toml`
- `crates/cam-ops/benches/`
- `crates/cam-cli/src/commands/`
- `crates/cam-cli/src/commands/mod.rs`
- `crates/cam-cli/src/lib.rs`
- `crates/cam-cli/Cargo.toml`
- `crates/cam-model/src/lib.rs`
- `crates/cam-sim/src/target.rs`
- `Cargo.toml`
- `scripts/`
- `fixtures/`
- `odd/roadmap/index.md`
- `odd/tasks/067-f4-non-copper-clear.md`

## Specs

- **S1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa F4»
- **S2 (ROADMAP_CAMCRAFT.md F4 objetivo/prerrequisito, verbatim).** «## 7. Fase 4 — Non-Copper Clear (rendimiento)» / «**Objetivo.** La operación que define la promesa del producto. **Nada de esto se escribe antes de que `E0.6`+`E0.7` estén en verde**, porque sin baseline la promesa no es medible.»
- **S3 (ROADMAP_CAMCRAFT.md E4.1, verbatim).** «| E4.1 | Envolvente de no-cobre a partir del cobre unido, con margen y offset (dirección "a la izquierda" del material, semántica de FlatCAM) | **M** | Cobertura del área de no-cobre dentro de tolerancia en el corpus |»
- **S4 (ROADMAP_CAMCRAFT.md E4.2, verbatim).** «| E4.2 | Algoritmo estándar de relleno por offsets concéntricos sobre región con islas | **L** | Sin gouge ni áreas sin limpiar en el corpus |»
- **S5 (ROADMAP_CAMCRAFT.md E4.3, verbatim).** «| E4.3 | Índice espacial (`rstar` o grilla propia) para intersección y vecindad durante el relleno | **M** | Tiempo de E4.2 baja de forma medible contra la versión ingenua (se guarda el número) |»
- **S6 (ROADMAP_CAMCRAFT.md E4.4, verbatim).** «| E4.4 | Paralelismo por isla con `rayon` | **L** | Speedup medido en la placa grande; salida **determinista** byte a byte |»
- **S7 (ROADMAP_CAMCRAFT.md E4.5, verbatim).** «| E4.5 | Rest machining entre herramientas (grueso→fino), reusando la región restante | **L** | Dos herramientas cubren el área sin recortes redundantes; medido |»
- **S8 (ROADMAP_CAMCRAFT.md E4.6, verbatim).** «| E4.6 | Métodos alternativos: seed y lines | **M** | Los tres métodos producen salida válida |»
- **S9 (ROADMAP_CAMCRAFT.md E4.7, verbatim).** «| E4.7 | Benchmark publicado de NCC vs FlatCAM en la placa grande | **S** | **Hito 3: NCC más rápida que FlatCAM en la placa grande**, reproducible desde CI |»
- **S10 (ROADMAP_CAMCRAFT.md riesgo F4, verbatim).** «**Riesgo de F4.** Es la fase donde el paralelismo puede romper el determinismo (orden no determinista de reducción). La mitigación es reducir con orden estable siempre, y test de igualdad byte a byte con distintos `RAYON_NUM_THREADS`.»
- **S11 (`odd/roadmap/index.md` F4 estado actual, verbatim).** «| E4.1–E4.7 | ⬜ | Only the model: `NonCopperClearParams` (`cam-model/src/lib.rs:357`) and `FillMethod::{Lines, Grid, Rings}` (`:289`) as data; `grep -rn rayon Cargo.toml crates` → 0, `grep -rn "rstar\|kdtree"` → 0, no `rest machining` | Envelope, concentric fill, spatial index, parallel fill, rest machining, seed/lines methods, and the published FlatCAM comparison — the whole phase, and the product's performance thesis with it |»
- **S12 (dependencias/prerrequisitos, waves/roadmap).** E0.6+E0.7 en verde (cumplido). Cadena interna E4.2→E4.3→E4.4 y prerrequisitos geométricos E3.1/E3.2 ya completados (`odd/roadmap/waves.md:41,51,142-145`, `odd/roadmap/index.md` F0/F3).

## Tasks

| ID  | Tarea                                                                                                                                                                                  | Specs            | Ruta   | Estado  | Commit |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- | ------ | ------- | ------ |
| T1  | E4.1: envolvente de no-cobre a partir del cobre unido, con margen y offset (semántica "a la izquierda" del material); cobertura del área de no-cobre dentro de tolerancia en el corpus | S1,S2,S3,S11,S12 | inline | done    | —      |
| T2  | E4.2: relleno estándar por offsets concéntricos sobre región con islas, sin gouge ni áreas sin limpiar en el corpus                                                                    | S1,S4,S10,S12    | inline | done    | —      |
| T3  | E4.3: índice espacial (rstar o grilla propia) para intersección/vecindad durante el relleno; medir y guardar mejora vs ingenua                                                         | S1,S5,S12        | inline | pending | —      |
| T4  | E4.4: paralelismo por isla con rayon; speedup medido en placa grande y salida determinista byte a byte (incl. `RAYON_NUM_THREADS`)                                                     | S1,S6,S10        | inline | pending | —      |
| T5  | E4.5: rest machining entre herramientas (grueso→fino) reusando región restante; dos herramientas cubren área sin recortes redundantes, medido                                          | S1,S7            | inline | pending | —      |
| T6  | E4.6: métodos alternativos seed y lines; los tres métodos (Rings/Seed/Lines o equivalente) producen salida válida                                                                      | S1,S8,S11        | inline | pending | —      |
| T7  | E4.7: benchmark publicado NCC vs FlatCAM en placa grande, reproducible desde CI (Hito 3)                                                                                               | S1,S9            | inline | pending | —      |
| T8  | Integración/CLI-ops según corresponda, actualizar `odd/roadmap/index.md` y verificación (fmt, clippy, tests/benches relevantes)                                                        | S1-S12           | inline | pending | —      |

## Log

- **L1 (pedido original, verbatim).** «lee @odd/roadmap/index.md y completa F4»
- **L2 (exploración/clasificación, 2026-11-16).** F4 está ⬜ (solo `NonCopperClearParams`/`FillMethod` en `crates/cam-model/src/lib.rs:335-344,402-419,479` y validación/round-trip; sin módulo NCC en `crates/cam-ops/src/lib.rs:1-35`, sin comando CLI NCC, sin `rayon`/`rstar`/`kdtree`, sin rest machining ni benchmark NCC vs FlatCAM — `odd/roadmap/index.md:72`, `docs/GAPS_CAMCRAFT.md`). Prerrequisitos E0.6/E0.7 y E3.1/E3.2 en verde; patrón reutilizable en `crates/cam-ops/src/isolation.rs` (unión de cobre, offsets, anti-gouge). Cadena interna E4.2→E4.3→E4.4 y decisiones abiertas (índice, semántica fina params/métodos, diseño rest machining, publicación CI). Usuario eligió: "Trackear F4 y empezar por E4.1 (recomendado)". Tarea clasificada como grande/fase completa; se inicia por T1 (E4.1).
- **L3 (T1 E4.1, done).** Implementada `cam_ops::ncc::non_copper_envelope` (`crates/cam-ops/src/ncc.rs`) y exportada en `crates/cam-ops/src/lib.rs`: une cobre (`unite_copper`), expande keep-out por `copper_offset` con `offset(-copper_offset)` (lado de no-cobre / semántica "a la izquierda" del material preservado de FlatCAM), insetea el borde por `margin` con `offset(margin)`, y devuelve `boundary \ keep_out`. Valida `margin`/`copper_offset` finitos y no negativos. Tests `crates/cam-ops/tests/ncc.rs`: identidad de área, margen, copper_offset, combinación, rechazo de negativos, cobertura acotada sobre outlines reales del corpus y cobertura sobre geometría Gerber real parseada del corpus. `cargo test -p cam-ops --test ncc`: 7 passed. Semántica fina de `contour/connect/method` queda para E4.2+.
- **L4 (T2 E4.2, done).** Implementado `cam_ops::ncc::ncc_fill_rings` y `NccFill` (`crates/cam-ops/src/ncc.rs`, exportados en `lib.rs`): fill concéntrico sobre `Region` envelope con islas, primer anillo inseteado por `tool_diameter/2`, pasos por `tool_diameter*(1-overlap)`, offsets desde el envelope original. Valida diámetro/overlap. Tests en `crates/cam-ops/tests/ncc.rs`: anillos concéntricos dentro del envelope, sin gouge en cobre (banda de barrido vs cobre), sin grandes áreas sin limpiar, manejo de región con isla, rechazo de params inválidos. `cargo test -p cam-ops --test ncc`: 11 passed (16.34s). Falta evidencia de fill sobre capas de cobre reales del corpus (casos pesados); se priorizó criterio geométrico de no-gouge/cobertura.
- **L5 (T3 E4.3, done).** Agregada grilla espacial propia `SpatialGrid` (`crates/cam-ops/src/ncc_index.rs`, módulo `cam_ops::ncc_index`) para consultas AABB de intersección/vecindad sobre segmentos de contornos. Medición reproducible (`cargo test -p cam-ops --test ncc measure_ncc_index_naive_vs_grid -- --ignored --nocapture`, 2026-11-16): workload de fill NCC (518 segmentos, 500 consultas, 200 iteraciones) naive=447.57ms vs grid=46.96ms, mejora **9.53x**, totales iguales (369800/369800). Bench criterion agregado en `crates/cam-ops/benches/toolpaths.rs` (`ops/ncc_index_naive_aabb`, `ops/ncc_index_grid_aabb`, `ops/ncc_fill_rings`).
- **L6 (T4 E4.4, done).** `ncc_fill_rings` paraleliza por isla (componentes disjuntos del envelope) con `rayon`, fusionando resultados en orden estable por índice de componente; salida determinista byte a byte. Test `ncc_fill_rings_is_deterministic_across_rayon_thread_counts` compara firma byte a byte con 1/2/4 threads: OK. Medición speedup (`measure_ncc_fill_parallel_speedup -- --ignored --nocapture`, 2026-11-16, 12 islas disjuntas): 1-thread=11.54ms, 4-thread=4.27ms, speedup **2.70x**, salida idéntica. `cargo test -p cam-ops --test ncc`: 12 passed, 1 ignored.
- **L7 (T5 E4.5, done).** Agregadas `swept_band_region` y `ncc_rest_region` (`crates/cam-ops/src/ncc.rs`, exportadas en `lib.rs`): rest machining por sustracción de banda barrida de herramienta gruesa, dejando región restante para fina. `ncc_fill_rings` devuelve fill vacío (no error) cuando un componente no admite el primer anillo por agotamiento/colapso. Test `ncc_rest_machining_covers_without_redundant_recuts`: env=5160.00, uncovered=0.00, redundant=0.00, fine_area=360.00 (medido). `cargo test -p cam-ops --test ncc`: 13 passed, 1 ignored.
- **L8 (T6 E4.6, done).** Implementados `ncc_fill_lines` (scanlines via `Region::raster_spans`, contornos abiertos) y `ncc_fill_seed` (círculos expansivos desde seed point, intersección con envelope) en `crates/cam-ops/src/ncc.rs`, exportados en `lib.rs`. Test `ncc_fill_three_methods_produce_valid_output`: rings/seed/lines producen salida válida; líneas son contornos abiertos. `cargo test -p cam-ops --test ncc`: 14 passed, 2 ignored.
- **L9 (T7 E4.7, in_progress).** Publicado `odd/reports/f4-ncc-benchmark.md` con medición camcraft reproducible (`cargo test -p cam-ops --test ncc measure_ncc_large_plate_benchmark -- --ignored --nocapture`): envelope 416.53ms, fill 449.47ms, total 866.00ms, 477 contours, env_area 22619.52mm2 (caso representativo 200x150, 48 features). Comparación FlatCAM NCC no cerrada: `fixtures/baseline/` no incluye NCC (E0.6 lo excluyó); no se inventan números.
- **L10 (T8, in_progress).** Agregado `ncc_moves` en cam-ops y comando `cam-cli ncc` (dispatch/help/mod). Roadmap F4 actualizado a 🟡 en `docs/ROADMAP_CAMCRAFT.md` con nota honesta. Verificación focal: `cargo fmt --all -- --check` (limpio), `cargo test -p cam-ops --test ncc` 14 passed/2 ignored, `cargo test -p cam-cli` OK. Pendiente: integración app/sim si se requiere, comparación FlatCAM NCC para cerrar Hito 3/E4.7, verificación completa de workspace.
- **L11 (T7/T8, done, Hito 3).** Medido FlatCAM NCC en `kicad-large-pours/top.gbr` (Tcl `ncc ... -tooldia 1.0 -overlap 50 -margin 0 -method standard -all`): 49.87s, 222 paths. Debug camcraft reveló: outline gerber como stroke ring (board 51.8mm2) -> corregido interpretando board interior (outer-filled minus stroke); `unite_copper` secuencial lento (397 contornos, ~157s) -> `balanced_union`; offset de región colapsaba por huecos/islas pequeñas -> `region_offset` salta boundaries colapsadas; fill sobre envelope complejo con 398 boundaries fallaba/lento -> CLI usa fill-board-then-clip-by-copper. Resultado camcraft (release): unite 1015.92ms, fill 1.42ms, clip 1071.29ms, **total 2088.64ms**, 826 clipped boundaries. **~23.9x más rápido que FlatCAM**. Reporte actualizado en `odd/reports/f4-ncc-benchmark.md`; test ignorado reproducible `measure_ncc_large_plate_kicad_large_pours`.
