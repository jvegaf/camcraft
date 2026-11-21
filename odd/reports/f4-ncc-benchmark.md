# F4 NCC benchmark (E4.7 / Hito 3)

## Estado

- **Hito 3:** camcraft NCC es más rápida que FlatCAM en `kicad-large-pours/top.gbr` (placa grande del corpus) con la misma operación (NCC, tool 1.0mm, overlap 50%, margin 0, standard/rings, all).
- **Camcraft (release):** 2.09s total. **FlatCAM:** 49.87s. **Speedup:** ~23.9x.
- **Reproducibilidad:** camcraft reproducible con comando cargo abajo; FlatCAM medido con comando Tcl documentado (requiere FlatCAM-Plus venv).

## Caso: placa grande corpus (kicad-large-pours/top.gbr)

Parámetros: tool 1.0mm, overlap 50%, margin 0, copper_offset 0, method standard/rings, all.

### FlatCAM (medido 2026-11-16)

Entorno: `~/Code/FlatCAM-Plus/.venv/bin/python`, FlatCAM-Plus 1.3.6, QT_QPA_PLATFORM=offscreen.

```tcl
open_gerber fixtures/corpus/kicad-large-pours/top.gbr -outname board
ncc board -tooldia 1.0 -overlap 50 -margin 0 -method standard -all -outname board_ncc
```

| Métrica | Valor |
|---|---:|
| ncc_s | 49.87 |
| paths (objeto NCC) | 222 |

### Camcraft (medido 2026-11-16)

Comando reproducible:

```bash
cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored --nocapture
```

Nota: el outline de kicad-large-pours se interpreta como stroke de perfil Gerber; el board interior se toma como outer-filled minus stroke.

| Build | unite_ms | fill_ms | clip_ms | total_ms | raw contours | clipped boundaries | board_area_mm2 |
|---|---:|---:|---:|---:|---:|---:|---:|
| release | 1015.92 | 1.42 | 1071.29 | 2088.64 | 98 | 826 | 15825.68 |
| dev/test | 5449.77 | 3.42 | 5246.78 | 10699.96 | 98 | 826 | 15825.68 |

**Resultado:** camcraft release 2.09s vs FlatCAM 49.87s en la misma placa grande/params = **~23.9x más rápido**.

## Notas

- Conteos de paths/contours no son directamente comparables (representaciones distintas). La comparación de Hito 3 es de tiempo en la placa grande.
- La medición camcraft usa la estrategia fill-board-then-clip-by-copper para evitar offsets de envelopes con cientos de huecos.
