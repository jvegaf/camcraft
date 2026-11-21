# F4 NCC benchmark (E4.7)

## Estado

- **Camcraft NCC (medido):** sí, reproducible con cargo.
- **FlatCAM NCC baseline:** no presente en `fixtures/baseline/` (E0.6 excluyó NCC; ver `scripts/baseline/README.md`: "It also does not measure non-copper clear.").
- **Hito 3 ("NCC más rápida que FlatCAM en la placa grande, reproducible desde CI"):** comparación FlatCAM pendiente de capturar baseline NCC de FlatCAM en placa grande.

## Camcraft NCC (placa grande representativa)

Comando reproducible:

```bash
cargo test -p cam-ops --test ncc measure_ncc_large_plate_benchmark -- --ignored --nocapture
```

Caso: board 200x150mm, 48 features de cobre (8x6 rectángulos), margin 0.0, copper_offset 0.2mm, tool 1.0mm, overlap 0.5.

Resultado (2026-11-16, debug test build, máquina local dev):

| Métrica | Valor |
|---|---:|
| envelope_ms | 416.53 |
| fill_ms | 449.47 |
| total_ms | 866.00 |
| contours | 477 |
| env_area_mm2 | 22619.52 |

> Nota: número en build de test no optimizado; para comparación de performance usar perfil release/bench dedicado. La medición es reproducible con el comando anterior.

## Comparación FlatCAM (pendiente)

Para cerrar Hito 3 falta capturar NCC de FlatCAM sobre la placa grande del corpus con el mismo entorno FlatCAM fijado por E0.6 (`~/Code/FlatCAM-Plus/.venv/bin/python`, FlatCAM-Plus 1.3.6, Shapely 2.2.0, PyQt6 6.11.0) y publicar el número junto a este reporte. No se inventan números.
