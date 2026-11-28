# F4 NCC benchmark (E4.7 / Hito 3)

## Estado

- **Hito 3:** camcraft NCC es más rápida que FlatCAM en `kicad-large-pours/top.gbr` (placa grande del corpus) con la misma operación (NCC, tool 1.0mm, overlap 50%, margin 0, standard/rings, all).
- **Camcraft (release):** 2.09s total. **FlatCAM:** 49.87s. **Speedup:** ~23.9x.
- **Reproducibilidad:** camcraft reproducible con comando cargo abajo; FlatCAM medido con comando Tcl documentado (requiere FlatCAM-Plus venv).
- **Actualización 2026-11-28 (tarea 112 T4):** las dos cifras de esta sección son las del 2026-11-16 y se
  conservan como historia fechada. Una regresión posterior en `unite_copper` (commit `7714fc1`) llevó
  camcraft a 7314–7502 ms; el arreglo de la tarea 112 la devuelve a 1988–2060 ms (ver «Camcraft
  (medido 2026-11-28)»). El lado **FlatCAM no se re-midió** en esa tarea, así que el ~23.9x es la
  comparación del 2026-11-16 aplicada a una cifra de camcraft re-medida, no una comparación re-medida
  en las dos puntas.

## Caso: placa grande corpus (kicad-large-pours/top.gbr)

Parámetros: tool 1.0mm, overlap 50%, margin 0, copper_offset 0, method standard/rings, all.

### FlatCAM (medido 2026-11-16)

Entorno: `~/Code/FlatCAM-Plus/.venv/bin/python`, FlatCAM-Plus 1.3.6, QT_QPA_PLATFORM=offscreen.

```tcl
open_gerber fixtures/corpus/kicad-large-pours/top.gbr -outname board
ncc board -tooldia 1.0 -overlap 50 -margin 0 -method standard -all -outname board_ncc
```

| Métrica            | Valor |
| ------------------ | ----: |
| ncc_s              | 49.87 |
| paths (objeto NCC) |   222 |

### Camcraft (medido 2026-11-16)

> **Nota de fecha (añadida el 2026-11-28, tarea 112 T4).** La cifra se registró el 2026-11-16, pero
> el código que la produce **no existía en esa fecha**: en `934d48f` (HEAD de ese día) no había
> `crates/cam-ops/src/ncc.rs` ni el test `measure_ncc_large_plate_kicad_large_pours`. Ambos entraron en
> `7ddc23d` (2026-11-21), el mismo commit que publicó este reporte. La fila se conserva tal como se
> registró; la comprobación de que mide lo que dice es la re-medición del 2026-11-28 de abajo, que
> reproduce el mismo orden de magnitud sobre `7ddc23d` (**2284,53 ms**) y sobre su descendiente
> `5f53de3` (**2031,49 ms**), en esta misma máquina. Ver
> [ncc-regression.md](ncc-regression.md) para la medición completa y las cuatro falsificaciones.

Comando reproducible:

```bash
cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored --nocapture
```

Nota: el outline de kicad-large-pours se interpreta como stroke de perfil Gerber; el board interior se toma como outer-filled minus stroke.

| Build    | unite_ms | fill_ms | clip_ms | total_ms | raw contours | clipped boundaries | board_area_mm2 |
| -------- | -------: | ------: | ------: | -------: | -----------: | -----------------: | -------------: |
| release  |  1015.92 |    1.42 | 1071.29 |  2088.64 |           98 |                826 |       15825.68 |
| dev/test |  5449.77 |    3.42 | 5246.78 | 10699.96 |           98 |                826 |       15825.68 |

**Resultado (2026-11-16):** camcraft release 2.09s vs FlatCAM 49.87s en la misma placa grande/params = **~23.9x más rápido**.

### Camcraft (medido 2026-11-28, tarea 112 T4 — después del arreglo)

Misma máquina (Intel Core i7-10875H, `nproc` 16) y mismo comando reproducible de arriba, sobre
`86b37d5` con el arreglo de `unite_copper` de la tarea 112. Tres corridas:

| Corrida | unite_ms | fill_ms | clip_ms | total_ms | raw contours | clipped boundaries | board_area_mm2 |
| ------: | -------: | ------: | ------: | -------: | -----------: | -----------------: | -------------: |
|       1 |   974.86 |    1.21 | 1012.28 |  1988.35 |           98 |                826 |       15825.68 |
|       2 |  1004.97 |    1.26 | 1048.20 |  2054.43 |           98 |                826 |       15825.68 |
|       3 |   988.45 |    1.86 | 1068.42 |  2058.73 |           98 |                826 |       15825.68 |

Conteos idénticos a los del 2026-11-16 (`98` / `826` / `15825.68` mm²), mismo perfil `--release`,
`RAYON_NUM_THREADS` sin fijar (el camino de `unite`/`clip` no usa `rayon`).

Verificación independiente del cierre de T4, con el binario release ya construido y el comando
exacto de arriba (una corrida):

```text
kicad-large-pours top: unite_ms=961.67 fill_ms=1.30 clip_ms=1096.90 total_ms=2059.87 raw=98 clipped_boundaries=826 board_area=15825.68
```

Es decir **2,06 s**, dentro de la dispersión de las tres corridas de arriba y del orden de
`5f53de3` (2031,49 ms); la regresión de `unite` (6273–6431 ms) no aparece.

La tabla intermedia, que este arreglo elimina, es la de la tarea 112 T1 sobre el mismo comando y
placa: **7314,47–7502,20 ms** en HEAD `86b37d5` antes del arreglo (entre 3,2× y 3,7× el registro), con
`unite_ms` en 6273–6431 ms. Ver [ncc-regression.md](ncc-regression.md).

**Resultado (2026-11-28):** camcraft release ~1,99–2,06 s, es decir el mismo orden que la cifra del
2026-11-16 (**2088,64 ms**) y de su reproducción sobre `5f53de3` (**2031,49 ms**). Frente a los
49,87 s de FlatCAM la relación sigue siendo ~24× **si se conserva el lado FlatCAM**, que en esta tarea
**no se re-midió**: el venv de FlatCAM-Plus 1.3.6 no está en este árbol. La cifra de FlatCAM y la de
camcraft del 2026-11-16 quedan como historia fechada; ninguna de las dos se reescribe.

## Causa y arreglo del registro (tarea 112 T4)

- **Causa.** `unite_copper` en `src-tauri/crates/cam-ops/src/isolation.rs` clasifica la polaridad del
  cobre por profundidad de anidamiento (`nesting_depths`), y `nesting_depths` preguntaba
  `Region::contains` —distancia exacta a todos los segmentos del candidato, más una segunda vuelta
  para el número de vueltas— por **cada vértice de cada boundary contra cada una de las otras
  boundaries**. Con 397 boundaries y ~30 vértices por boundary son millones de consultas geométricas,
  y se pagaban dos veces: una en `unite_copper` y otra en `canonical_orientation`. Por eso `unite`
  pasó de ~986 ms a ~6,3 s mientras la unión booleana real seguía costando ~987 ms.
- **Arreglo.** `nesting_depths` conserva la misma pregunta (mayoría de los vértices encerrados) y
  responde exactamente lo mismo, pero deja de preguntar por los vértices que no pueden estar dentro:
  `Bounds::contains` (que ya crece con la tolerancia) descarta el vértice antes de la consulta
  geométrica, un par cuyos vértices alcanzables no llegan a la mayoría se salta entero, y la cuenta
  se corta en cuanto la comparación de mayoría ya no puede cambiar en ningún sentido. Ninguno de los
  dos filtros lee algo que la cuenta sin filtrar no leyera, así que la clasificación es idéntica.
  `unite` baja a ~975–1005 ms, el costo previo a la regresión.
- **Frontera de comportamiento (S7).** Las dos únicas placas del corpus que `image()` acepta se
  compararon volcando la región devuelta por `unite_copper` —número de boundaries, cada vértice en
  orden con sus 17 dígitos de `x`, `y` y `bulge`, y el área firmada de cada boundary— sobre el código
  anterior y el arreglado: **byte a byte idénticas** (707.263 bytes; `kicad-large-pours` sola,
  333.205 bytes). `kicad-4l-pours`, `substitute-altium`, `substitute-eagle` y `substitute-easyeda`
  fallan ya en `image()` (polaridad/arrangement) en el código anterior y en el arreglado por igual, así
  que quedan fuera de la comparación y no son un efecto de este cambio.
- **Guardia.** `./src-tauri/crates/cam-ops/benches/check_regression.sh` compara
  `ops/ncc_unite_corpus_large_plate` contra el baseline comprometido: antes del arreglo salía `1` con
  `Performance has regressed` (+426,80 % sobre 1,14 s); después sale `0` con el bench dentro del
  umbral del 10 %.

## Notas

- Conteos de paths/contours no son directamente comparables (representaciones distintas). La comparación de Hito 3 es de tiempo en la placa grande.
- La medición camcraft usa la estrategia fill-board-then-clip-by-copper para evitar offsets de envelopes con cientos de huecos.
