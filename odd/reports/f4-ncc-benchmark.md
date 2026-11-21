# F4 NCC benchmark (E4.7)

## Estado

- **Camcraft NCC (medido):** sí, en caso representativo sintético; en `kicad-large-pours/top.gbr` el resultado observado es **problemático** (ver abajo).
- **FlatCAM NCC (medido):** sí, en `kicad-large-pours/top.gbr`.
- **Hito 3 ("NCC más rápida que FlatCAM en la placa grande, reproducible desde CI"):** **NO cerrado**. En la placa grande del corpus, camcraft fue más lento que FlatCAM y produjo salida casi vacía en la medición observada; hay que depurar corrección/performance antes de reclamar el hito.

## FlatCAM NCC (placa grande corpus, medido 2026-11-16)

Comando (FlatCAM-Plus Tcl, offscreen, venv `~/Code/FlatCAM-Plus/.venv/bin/python`):

```tcl
open_gerber fixtures/corpus/kicad-large-pours/top.gbr -outname board
ncc board -tooldia 1.0 -overlap 50 -margin 0 -method standard -all -outname board_ncc
```

Resultado:

| Métrica | Valor |
|---|---:|
| ncc_s | 49.87 |
| paths (objeto NCC) | 222 |

## Camcraft NCC (placa grande corpus, observado 2026-11-16)

Caso: `fixtures/corpus/kicad-large-pours/top.gbr` + `outline.gbr`, margin 0, copper_offset 0, tool 1.0, overlap 0.5, rings, tolerancia 1e-6, build test/dev.

Resultado observado:

| Métrica | Valor |
|---|---:|
| envelope_ms | 155757.46 |
| fill_ms | 1.13 |
| total_ms | 155758.59 |
| contours | 0 |
| env_area_mm2 | 51.81 |

**Diagnóstico:** resultado incorrecto/sospechoso y mucho más lento que FlatCAM en este caso real. No usar como evidencia de Hito 3; investigar envelope/semántica en placas con pours grandes.

## Camcraft NCC (caso sintético representativo, medido 2026-11-16)

Comando reproducible:

```bash
cargo test -p cam-ops --test ncc measure_ncc_large_plate_benchmark -- --ignored --nocapture
```

Caso: board 200x150mm, 48 features de cobre, margin 0.0, copper_offset 0.2mm, tool 1.0mm, overlap 0.5.

| Métrica | Valor |
|---|---:|
| envelope_ms | 416.53 |
| fill_ms | 449.47 |
| total_ms | 866.00 |
| contours | 477 |
| env_area_mm2 | 22619.52 |

> Nota: números en build de test no optimizado; no comparables directamente con FlatCAM release. La comparación Hito 3 requiere el mismo caso, medición justa y salida correcta.
