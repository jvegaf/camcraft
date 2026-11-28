# NCC performance regression — measurement and localisation (task 112 / T1)

Medición del **2026-11-28**. Este documento es evidencia de T1 (S2, S3): reproduce la medición de
[odd/reports/f4-ncc-benchmark.md](f4-ncc-benchmark.md), comprueba las cuatro falsificaciones de S3 y
localiza el tramo dominante. **No** contiene arreglo: la regresión queda confirmada y atribuida.

## Veredicto

**Regresión confirmada.** No es ruido, no es otra máquina y no es otra placa: es un cambio de código
en `unite_copper`.

- El registro de 2088,64 ms **se reproduce en esta misma máquina** sobre el commit que introdujo el
  test y el reporte (7ddc23d, 2026-11-21): **2284,53 ms**, y sobre su descendiente inmediato anterior
  al cambio culpable (5f53de3): **2031,49 ms**.
- Hoy (HEAD `86b37d5`) la misma placa y el mismo comando dan **7314,47–7502,20 ms**: entre **3,2× y
  3,7×** el registro. La dispersión de las tres corridas de hoy es del **2,6 %**, no del 3×.
- El tiempo se fue en `unite_ms`, no en `clip_ms`:

  | Commit                        | Fecha      |    unite_ms | fill_ms | clip_ms |    total_ms |
  | ----------------------------- | ---------- | ----------: | ------: | ------: | ----------: |
  | 7ddc23d (reporte/test)        | 2026-11-21 |     1103,75 |    1,18 | 1179,60 |     2284,53 |
  | 5f53de3 (padre de 7714fc1)    | 2026-11-25 |      985,72 |    1,38 | 1044,38 |     2031,49 |
  | **7714fc1 (cambio culpable)** | 2026-11-25 | **6923,85** |    1,31 | 1129,67 | **8054,83** |
  | HEAD 86b37d5, corrida 1       | 2026-11-28 |     6431,43 |    1,51 | 1069,26 |     7502,20 |
  | HEAD 86b37d5, corrida 2       | 2026-11-28 |     6345,13 |    1,41 | 1081,78 |     7428,33 |
  | HEAD 86b37d5, corrida 3       | 2026-11-28 |     6273,51 |    1,49 | 1039,47 |     7314,47 |

- El culpable es **7714fc1** (`fix(ops): derive copper polarity from nesting`), que reescribió
  `unite_copper` de unión por winding a unión por profundidad de anidamiento. Su costo es
  `nesting_depths`, llamada **dos veces** (una en `unite_copper` y otra en `canonical_orientation`):
  **~5,34 s de los 6,33 s de `unite` (84 %)** con la instrumentación de abajo. La unión booleana en sí
  sigue costando **~987 ms**, igual que el `unite` completo del commit anterior a 7714fc1.

## Entorno y comando

- Máquina: Intel(R) Core(TM) i7-10875H @ 2,30 GHz, **16** CPUs (`nproc` → 16), Linux.
- `rustc 1.100.0 (0ef1bc386 2026-11-09)`, `cargo 1.100.0 (3acc79c0c 2026-11-05)`.
- `RAYON_NUM_THREADS` **sin fijar** (el test no depende de él: `unite`/`clip` no usan `rayon`).
- Perfil: `--release`. En HEAD el perfil release tiene `lto = true`, `codegen-units = 1`,
  `opt-level = 3`; los commits viejos medidos también salvo 7ddc23d, que no tenía `[profile.release]`
  y por eso corre con el release por defecto (si acaso, en desventaja).
- Comando exacto, idéntico al del reporte:

  ```bash
  cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored --nocapture
  ```

- Placa: `fixtures/corpus/kicad-large-pours/top.gbr` (+ `outline.gbr`), parámetros del reporte
  (tool 1,0 mm, overlap 50 %, margin 0, copper_offset 0, method standard/rings, all).

### Estado de la máquina (límite declarado)

Durante las corridas de HEAD había un proceso de usuario `/usr/bin/solidcraft` activo (arrancado a las
06:20, bajando de ~290 % a ~74 % de CPU). `uptime` marcó load average **1,2–2,4** sobre 16 CPUs. No
se mató (no es una operación permitida a este worker). Aun así el spread de HEAD es **2,6 %**, y los
commits viejos reproducen el registro bajo el mismo proceso; la conclusión no depende de ese ruido.
La máquina del registro **no está documentada** en `f4-ncc-benchmark.md`; la falsificación por máquina
se resolvió re-midiendo el commit viejo **en este host**, no comparando hardware declarado.

## Mediciones reproducibles

Todas las corridas registran conteos idénticos: `raw=98`, `clipped_boundaries=826`,
`board_area=15825.68` mm². HEAD es `86b37d5` (2026-11-28).

### HEAD (3 corridas)

```text
corrida 1: unite_ms=6431.43 fill_ms=1.51 clip_ms=1069.26 total_ms=7502.20 raw=98 clipped_boundaries=826 board_area=15825.68
corrida 2: unite_ms=6345.13 fill_ms=1.41 clip_ms=1081.78 total_ms=7428.33 raw=98 clipped_boundaries=826 board_area=15825.68
corrida 3: unite_ms=6273.51 fill_ms=1.49 clip_ms=1039.47 total_ms=7314.47 raw=98 clipped_boundaries=826 board_area=15825.68
```

Spread de `total_ms`: mínimo 7314,47, máximo 7502,20 → **2,6 %**. Corrida 1 incluye compilación
(wall 20,4 s); 2 y 3 corren el binario ya construido (wall ~9,0 s).

### Commits viejos (misma máquina, mismo rustc, mismo comando)

Extraídos con `git archive <commit> | tar -x -C "$(mktemp -d /tmp/...)"` y compilados ahí, fuera del
árbol. No se commiteó nada.

```text
7ddc23d: unite_ms=1103.75 fill_ms=1.18 clip_ms=1179.60 total_ms=2284.53 raw=98 clipped_boundaries=826 board_area=15825.68
5f53de3: unite_ms=985.72  fill_ms=1.38 clip_ms=1044.38 total_ms=2031.49 raw=98 clipped_boundaries=826 board_area=15825.68
7714fc1: unite_ms=6923.85 fill_ms=1.31 clip_ms=1129.67 total_ms=8054.83 raw=98 clipped_boundaries=826 board_area=15825.68
```

Atribución por bracketing: 5f53de3 es el padre directo de 7714fc1; `unite_ms` salta de **985,72** a
**6923,85** en ese único commit. Entre 7ddc23d y 7714fc1, `isolation.rs` (`unite_copper`) sólo cambia
en 7714fc1; entre 7714fc1 y HEAD no hay ningún diff en `isolation.rs`.

## Las cuatro falsificaciones de S3

1. **Otra máquina / una más ocupada — descartada.** El 2088,64 se reproduce a **2284,53 / 2031,49 ms**
   en este host, con el mismo `nproc=16` y el mismo rustc que producen los 7314–7502 ms de hoy. La
   comparación es manzana con manzana en hardware; además el spread de hoy es 2,6 %, no 3,4×.
   El reporte no declara su hardware, así que la igualdad de host no se asume: se demuestra
   re-midiendo el código viejo acá.
2. **Otra placa u otros parámetros — descartada.** `git diff --stat 7ddc23d..HEAD --
   fixtures/corpus/kicad-large-pours` está vacío: `top.gbr` y `outline.gbr` no cambiaron (lo único que
   cambió en el corpus desde 934d48f es `parity.json`). El test sólo cambió la ruta raíz
   (`"../.."` → `"../../.."`) por la migración a `src-tauri/`; los parámetros
   (`1.0`, `0.5`, `TolerancePolicy::uniform(1.0e-6)`) son idénticos. Conteos idénticos en todas las
   corridas (`raw=98`, `826` boundary, `15825.68` mm²) lo confirman.
3. **Otro perfil de build o features — descartada.** Las dos puntas se midieron con `--release`. El
   perfil de HEAD es **más** optimizado que el del commit 7ddc23d (agrega LTO y `codegen-units=1`), así
   que el perfil empuja en contra de la regresión, no a favor. No hay features de NCC que activar o
   desactivar; el test no las usa.
4. **Deriva de dependencias — descartada.** Entre 7ddc23d y HEAD los cambios de `Cargo.toml`/`Cargo.lock`
   son la migración a `src-tauri/`, el workspace Tauri y crates de UI/export; no entra ninguna
   dependencia de geometría ni de índice espacial (`geo`, `i_overlay`, `clipper2`, `rstar`, `kdtree`
   siguen ausentes). `rayon` ya estaba desde 8d0ad9d y no interviene en `unite`/`clip`. Los dos árboles
   viejos se compilaron **hoy con el mismo rustc**, así que tampoco hay deriva de compilador en la
   comparación. La causa es código in-tree (`unite_copper`), no versiones.

## Localización: dónde se va el tiempo

Código relevante: `src-tauri/crates/cam-ops/src/ncc.rs` (test en
`src-tauri/crates/cam-ops/tests/ncc.rs:692`) y `src-tauri/crates/cam-ops/src/isolation.rs`
(`unite_copper`).

El test separa `unite` (`unite_copper`), `fill` (`ncc_fill_rings`) y `clip` (normalizar el relleno y
restarle el cobre unido). `fill` siempre costó ~1,4 ms y `clip` ~1,0–1,2 s en **todos** los commits:
no son la regresión. Todo el delta está en `unite`.

Split más fino: en una copia scratch de 7714fc1 (`/tmp/ncc-7714fc1-jo16rO`, fuera del repo) se
instrumentó `unite_copper` con `Instant` (sin commitear) y se corrigió sólo la ruta del test a
`"../../.."` para que encontrara el fixture tras la migración. Resultado de una corrida:

```text
unite_detail: n=397 norm_ms=7.44
unite_detail: nesting_ms=2650.90
unite_detail: union_ms=986.77
unite_detail: canonical_nesting_ms=2684.47
unite_detail: canonical_total_ms=2684.71
kicad-large-pours top: unite_ms=6329.88 fill_ms=1.12 clip_ms=1040.13 total_ms=7371.12 ...
```

- Entrada: 98 contornos de cobre que la normalización por contorno convierte en **397 boundaries**.
- `nesting_depths` (`isolation.rs`): para cada boundary, prueba **todos los vértices** contra **todas**
  las demás boundaries con `Region::contains`, O(n²) en boundaries y O(vértices) por prueba. Se llama
  **dos veces**: una en `unite_copper` y otra en `canonical_orientation`.
  Costo medido: **2650,90 + 2684,47 = 5335,37 ms**, el **84 %** de `unite`.
- `union_ms = 986,77` es la unión booleana real (balanced union por profundidad); coincide con el
  `unite_ms = 985,72` del commit anterior a 7714fc1, que no tenía nesting. Es decir: el trabajo
  geométrico no empeoró; lo que se agregó es la clasificación de anidamiento.
- `norm_ms = 7,44` es despreciable.

La placa fuerza el caso porque tiene muchos contornos de cobre solapados/anidados (pours, islas y
huecos): 397 boundaries hacen ~157.000 pares y cada `contains` recorre los vértices del boundary
consultado. Los índices espaciales existentes (`ncc_index.rs`, la grilla local de `boolean.rs`) no se
usan en `nesting_depths`.

**Función caliente:** `nesting_depths` en `src-tauri/crates/cam-ops/src/isolation.rs`, llamada desde
`unite_copper` y `canonical_orientation`. Evidencia: bracketing 5f53de3 → 7714fc1 y split por fases de
arriba. No se usó `perf` (no fue necesario); la instrumentación temporal en scratch da la atribución
directa.

## Qué evidencia cambiaría el veredicto

- Que el commit 5f53de3 (o 7ddc23d) medido en este host diera ~7000 ms en vez de ~2000 ms: entonces el
  2088 no sería reproducible y la comparación sería «no comparable».
- Que el fixture `top.gbr`/`outline.gbr` o los parámetros del test difirieran del reporte: medirían
  otra cosa.
- Que la regresión desapareciera al compilar HEAD con el perfil release sin LTO o con otro rustc:
  apuntaría a perfil/compilador y no a `unite_copper`.
- Que el split de fases mostrara el costo en `union_ms` y no en `nesting_*`: entonces el culpable sería
  el kernel de booleanas, no la clasificación por anidamiento.

## Caveats

- El reporte de F4 fecha la medición el **2026-11-16**, pero en `934d48f` (HEAD de ese día) no existían
  `crates/cam-ops/src/ncc.rs` ni el test `measure_ncc_large_plate_kicad_large_pours`; ambos entran en
  `7ddc23d` (2026-11-21). La cifra **sí se reproduce** en el código de 7ddc23d/5f53de3, así que la
  comparación es válida en sustancia, pero la fecha del reporte no coincide con el commit que contiene
  el test. Queda anotado para T4 (S6), no corregido acá.
- Corridas hechas con `/usr/bin/solidcraft` de fondo (~1–3 CPUs); ver «Estado de la máquina».
- Directorios scratch dejados fuera del repo (este worker no borra): `/tmp/ncc-report-commit-CnbNHn`,
  `/tmp/ncc-parent-7fhp6F`, `/tmp/ncc-7714fc1-jo16rO`.
