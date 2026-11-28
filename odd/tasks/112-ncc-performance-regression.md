# 112 — NCC performance regression: 3,4× slower than its own record, and the gate that missed it

Investigar y arreglar la regresión de rendimiento de NCC: hoy la placa grande del corpus mide
~3,4× por encima de su propia cifra del 2026-11-16, y el gate que debía haberla visto no corre.
Estado: `open` — no se reprodujo ni se localizó la causa, el `check_regression.sh` sigue roto y el
job `bench` de CI sigue siendo un placeholder; ningún `S#` está satisfecho.


- `src-tauri/crates/cam-geom/benches/` y `src-tauri/crates/cam-ops/benches/` (`check_regression.sh`, los `README.md` y los baselines commiteados)
- `src-tauri/crates/cam-ops/tests/ncc.rs` (`measure_ncc_large_plate_kicad_large_pours`)
- `.github/workflows/ci.yml` (job `bench`), `justfile` y `scripts/check.sh` si el gate se cablea ahí
- `odd/reports/f4-ncc-benchmark.md`
- `src-tauri/crates/cam-ops/src/ncc.rs` y `src-tauri/crates/cam-geom/src/`, **sólo** para el arreglo que S6 autoriza
- `odd/tasks/112-ncc-performance-regression.md`

## Surfaces


- `src-tauri/crates/cam-geom/benches/` y `src-tauri/crates/cam-ops/benches/` (`check_regression.sh`, los `README.md` y los baselines commiteados)
- `src-tauri/crates/cam-ops/tests/ncc.rs` (`measure_ncc_large_plate_kicad_large_pours`)
- `.github/workflows/ci.yml` (job `bench`), `justfile` y `scripts/check.sh` si el gate se cablea ahí
- `odd/reports/f4-ncc-benchmark.md`
- `src-tauri/crates/cam-ops/src/ncc.rs` y `src-tauri/crates/cam-geom/src/`, **sólo** para el arreglo que S6 autoriza
- `odd/tasks/112-ncc-performance-regression.md`

## Specs

- **S1 (mandato del owner, verbatim).** En una decisión estructurada el owner eligió la opción
  **«Abrir tarea para la regresión NCC»**, descrita así:

  ```text
  Documento ODD con el 3,4× medido, el gate de bench que no corre y el check_regression roto; investigarlo y arreglarlo con su propia unidad de trabajo.
  ```

  El entregable de esta tarea es **medir, localizar, arreglar el gate y arreglar o explicar la
  regresión**, en ese orden. No es un documento de diagnóstico como la [tarea 110](110-gaps-resurvey.md)
  —que sólo registra— ni un gate de documentación como la [tarea 111](111-gaps-states-gate.md): toca
  benches, scripts, CI y, si la medición lo justifica, código de `cam-ops`/`cam-geom`.

- **S2 (la medición se reproduce antes de creerla).** El 3,4× se sostiene sólo si la comparación es
  manzana con manzana contra los **2088,64 ms** que registró
  [odd/reports/f4-ncc-benchmark.md](../reports/f4-ncc-benchmark.md) el 2026-11-16. Antes de proponer
  nada, la medición se repite **al menos tres veces** con la misma placa y el mismo comando:

  ```bash
  cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored --nocapture
  ```

  sobre `fixtures/corpus/kicad-large-pours/top.gbr`, con los parámetros del reporte (tool 1.0 mm,
  overlap 50 %, margin 0, copper_offset 0, method standard/rings, all) y perfil **release**. La
  evidencia registra, junto a cada corrida:

  1. el comando exacto y el `commit` del árbol medido;
  2. `nproc` y el número de hilos con que corre Rust (`RAYON_NUM_THREADS`, si se fija);
  3. el perfil de build (`--release` frente a dev/test), que en el reporte separa 2088,64 ms de
     10699,96 ms para la **misma** placa;
  4. el split que el test ya imprime: `unite_ms`, `fill_ms`, `clip_ms` y `total_ms`.

  Una cifra sin esos cuatro datos no se compara contra 2088,64 ms, se registra como medición aparte.

- **S3 (la causa no se asume).** Esta tarea **no** declara que el código se volvió lento: localiza
  dónde se va el tiempo antes de proponer cualquier arreglo. La localización es, como mínimo:

  - el reparto entre `unite_ms` y `clip_ms` en cada corrida de S2 (la evidencia de hoy dice
    `unite ≈ 6100 ms`, `clip ≈ 1000 ms`, contra 1015,92 ms y 1071,29 ms del registro), y
  - una medición de granularidad más fina sobre el tramo que domine —un profile (`perf`, `flamegraph`,
    o instrumentación temporal que no se commitea)— que nombre las funciones que consumen el tiempo.

  Y declara explícitamente **qué evidencia falsificaría la lectura de «regresión»**, antes de buscar
  culpables:

  1. **otra máquina** — la cifra de 2088,64 ms se midió en otro host; un `nproc` o una CPU distinta
     invalida la comparación directa y la convierte en «medición de referencia a re-medir»;
  2. **otra placa o otros parámetros** — si el fixture, el Gerber o los parámetros del test cambiaron
     desde el 2026-11-16 (`git log` sobre `fixtures/corpus/kicad-large-pours/` y sobre
     `src-tauri/crates/cam-ops/tests/ncc.rs`), los dos números no miden lo mismo;
  3. **otro perfil de build** — dev/test ya era 5,1× más lento en el propio reporte;
  4. **deriva de dependencias** — `Cargo.lock` y los `Cargo.toml` de `cam-ops`/`cam-geom` entre el
     commit del 2026-11-16 y hoy (entry `8d0ad9d`, `rayon`, `criterion`, el kernel de booleanas).

  Si alguna de las cuatro se cumple, el resultado de esta tarea es la explicación con su evidencia,
  no un «arreglo» de código que nunca estuvo lento.

- **S4 (el gate se arregla en esta tarea, no después).** La regresión se midió tres veces y ningún
  gate la vio; eso se corrige acá, con estas cuatro condiciones verificables sobre
  `src-tauri/crates/*/benches/check_regression.sh` y sus `README.md`:

  1. **compara contra un baseline que alguien commiteó de verdad** — hoy el baseline de `cam-geom` y
     `cam-ops` cubre 5 de los 7 benches declarados en sus `criterion_group!`; los dos de NCC no lo
     tienen, así que el bench que mide el relleno no tiene contra qué compararse. El baseline
     commiteado cubre **todos** los benches declarados, y `--baseline committed` no puede paniquear
     con «Baseline 'committed' must exist before comparison is allowed»;
  2. **escribe donde cargo lee** — el script restaura en `src-tauri/target/criterion` mientras cargo,
     cuya raíz de workspace es la raíz del repositorio, lee `target/criterion`; los dos tienen que ser
     el mismo directorio, y la prueba es que la comparación ocurra, no que el `cp` no falle;
  3. **es alcanzable en la ruta que documentan sus propios READMEs** — los dos `README.md` de
     `benches/` documentan `./crates/cam-*/benches/check_regression.sh`, una ruta anterior a la
     migración a `src-tauri/` que hoy sale 127; o el README apunta a la ruta real, o la ruta real
     existe en la que el README nombra;
  4. **falla fuerte** — una regresión termina en exit code distinto de `0`. El texto «change: …» de
     criterion no alcanza: la [tarea 044](044-e0-7-benchmarks.md) L7 ya observó que el proceso sale
     `0` con regresiones impresas. El umbral (porcentaje, banda de ruido) se declara en el script o en
     su README, y no se elige para que la corrida de hoy pase.

  El arreglo del arnés llega con la prueba de que **falla**: un baseline alterado a propósito (en una
  copia en `mktemp -d`, nunca en el árbol) hace salir al script distinto de `0`.

- **S5 (el job `bench` de CI corre algo real o se borra).** El job `bench` de
  [.github/workflows/ci.yml](../../.github/workflows/ci.yml) (~`:110-121`) es hoy un placeholder que
  imprime «Benchmarks deshabilitados durante migración Tauri.» y no corre ningún benchmark, bajo
  `if: github.event_name == 'workflow_dispatch'`: **pasa en verde sin medir nada**, y un placeholder
  que pasa en silencio es peor que no tener job. Al cerrar esta tarea el job es una de dos cosas,
  nunca el placeholder:

  - **real** — corre el gate de S4 y la medición reproducible de S2, con un umbral declarado que tenga
    sentido en un runner compartido (o una política explícita de «mide y publica, no bloquea», dicha
    en el YAML y en el README, no implícita); o
  - **borrado** — el job desaparece del workflow y ningún documento lo sigue anunciando como gate.

  `bash scripts/tests/ci-workflow.sh` sigue en `0` después del cambio, porque parsea la forma del
  workflow.

- **S6 (si la regresión es real, el arreglo llega con su guardia y el registro se actualiza).** Un
  arreglo de performance sin la medición que lo vigila es el mismo defecto con otra fecha:

  1. el cambio de rendimiento llega con **el bench o el test que habría atrapado la regresión** —el
     bench de NCC con baseline commiteado de S4, o un test de la placa grande con umbral—, y ese guard
     se demuestra **rojo sin el arreglo y verde con él**;
  2. [odd/reports/f4-ncc-benchmark.md](../reports/f4-ncc-benchmark.md) se **actualiza**, no se
     sobrescribe: la fila de 2026-11-16 y el 49,87 s de FlatCAM quedan como historia fechada con su
     comando, y al lado consta la medición de hoy con la suya. El «~23,9× más rápido que FlatCAM» no
     se reescribe en silencio: si la cifra de hoy ya no lo sostiene, el reporte lo dice y explica por
     qué (el lado FlatCAM **no** se re-mide en esta tarea);
  3. la actualización del reporte entra en el mismo cambio que el arreglo o la explicación, no en un
     commit posterior «de documentación».

- **S7 (frontera: sólo rendimiento).** El comportamiento no cambia: mismos moves, mismo orden, mismo
  G-code, misma salida golden. **Cualquier diferencia de G-code es una parada, no un arreglo** —si el
  arreglo de performance cambia la salida, el problema es otro y esta tarea se detiene a reportarlo—.
  Quedan fuera de alcance: rediseñar el kernel de booleanas, cambiar la estrategia
  fill-board-then-clip-by-copper del reporte, sumar dependencias nuevas de performance (`rstar`,
  `kdtree`) y tocar `cam-model`, la UI o la simulación. El paralelismo **no** es la palanca de esta
  tarea hasta que una medición lo diga: el bench `ops/ncc_fill_rings` no gana con hilos
  (`RAYON_NUM_THREADS=1` 3,72 ms vs `RAYON_NUM_THREADS=16` 3,83 ms, p = 0,06) y `rayon` sólo aparece
  en `cam-ops` (`src/ncc.rs:18`, `:141` y sus tests).

- **S8 (superficie y el gate de documentación).** La superficie de esta tarea es
  `src-tauri/crates/cam-geom/benches/`, `src-tauri/crates/cam-ops/benches/`,
  `src-tauri/crates/cam-ops/tests/ncc.rs`, `.github/workflows/ci.yml`, el `justfile` y
  `scripts/check.sh` si el gate se cablea ahí, [odd/reports/f4-ncc-benchmark.md](../reports/f4-ncc-benchmark.md),
  `src-tauri/crates/cam-ops/src/ncc.rs` y `src-tauri/crates/cam-geom/src/` **sólo** para el arreglo
  que S6 autoriza, y este documento. `./scripts/check-docs.sh` termina en `0` y todos los enlaces de
  este documento resuelven. Corregir lo que la investigación encuentre en
  [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) —que hoy registra el 3,4× y el gate roto sin
  corregirlos, por el S7 de la tarea 110— es de esa tarea o de otra, no de ésta.

## Tasks

| ID  | Tarea                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Specs  | Ruta   | Commit |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | ------ | ------ |
| T1  | Reproducir y localizar. Repetir al menos tres veces la medición de S2 con el comando exacto del reporte, registrando `commit` medido, `nproc`, hilos, perfil release y el split `unite_ms`/`fill_ms`/`clip_ms`/`total_ms` de cada corrida; después localizar el tramo dominante con un profile o instrumentación temporal (no commiteada), y dejar escritas las cuatro comprobaciones de falsificación de S3 —placa y parámetros sin cambios desde 2026-11-16, perfil idéntico, `Cargo.lock` y `Cargo.toml` de `cam-ops`/`cam-geom` sin deriva relevante—. Sin arreglo en esta unidad: sólo evidencia y la conclusión de si la lectura de «regresión» se sostiene. Área: `## Log` de este documento. | S2, S3 | worker |        |
| T2  | Arreglar el arnés de regresión: que el baseline commiteado cubra **todos** los benches declarados en cada `criterion_group!` —incluidos `ops/ncc_fill_rings` y los dos del índice, que hoy no lo tienen—, que `check_regression.sh` restaure donde cargo lee (raíz de workspace, `target/criterion`) en vez de `src-tauri/target/criterion`, que la ruta documentada por los dos `README.md` de `benches/` sea la que existe, y que una regresión salga distinta de `0` con un umbral declarado. Prueba: baseline alterado a propósito en una copia en `mktemp -d` → rojo observado; corrida normal → verde. Área: `src-tauri/crates/cam-geom/benches/`, `src-tauri/crates/cam-ops/benches/`. | S4     | worker |        |
| T3  | Hacer honesto el job `bench` de CI: o corre el gate de T2 más la medición reproducible de T1 con una política de umbral explícita para runner compartido, o se borra del workflow y ningún documento lo sigue anunciando como gate. Verificar `bash scripts/tests/ci-workflow.sh` en `0` y que el YAML no deja un paso que pasa sin medir. Área: `.github/workflows/ci.yml`, `justfile`/`scripts/check.sh` si el gate se cablea ahí. | S5     | worker |        |
| T4  | Cerrar la regresión: si T1 la sostiene, encontrar la causa concreta y arreglarla dentro de la frontera de S7, con la guardia de S6 —el bench o test que falla sin el arreglo y pasa con él— y sin ninguna diferencia de G-code (comprobación de salida igual antes/después). Si T1 la falsifica, escribir la explicación con la evidencia de las cuatro comprobaciones. En ambos casos, actualizar `odd/reports/f4-ncc-benchmark.md` conservando la fila de 2026-11-16 y el 49,87 s de FlatCAM como historia fechada y dejando constancia de que el lado FlatCAM no se re-midió. Área: `src-tauri/crates/cam-ops/src/ncc.rs`, `src-tauri/crates/cam-geom/src/`, benches/tests, `odd/reports/f4-ncc-benchmark.md`. | S3, S6, S7 | worker |        |
| T5  | Cierre: correr `./scripts/check-docs.sh` (debe salir en `0`), comprobar que todos los enlaces de este documento resuelven, y registrar en el `## Log` las mediciones finales con sus comandos, el veredicto sobre la regresión y el defecto de gate que quedó arreglado. No se toca nada más allá de lo que T1–T4 dejaron. Área: `## Log` de este documento. | S8     | worker |        |

## Log

- **L1 (decisión del owner, verbatim, 2026-11-28).** En una decisión estructurada el owner eligió la
  opción **«Abrir tarea para la regresión NCC»**, y la describió así:

  ```text
  Documento ODD con el 3,4× medido, el gate de bench que no corre y el check_regression roto; investigarlo y arreglarlo con su propia unidad de trabajo.
  ```

  El contexto en que apareció: al re-medir la tabla de rendimiento del §4 de
  [docs/GAPS_CAMCRAFT.md](../../docs/GAPS_CAMCRAFT.md) como T3 de la [tarea 110](110-gaps-resurvey.md),
  la placa grande del corpus midió ~3,4× por encima de su propia cifra del 2026-11-16, y los dos
  mecanismos que debían haberlo visto —el job `bench` de CI y `check_regression.sh`— resultaron
  inertes. El §4 de 110 registra el hallazgo y no lo corrige porque su S7 se lo prohíbe; esta tarea
  existe para investigarlo y arreglarlo.

- **L2 (evidencia recibida, 2026-11-28 — mediciones del escritor del T3 de 110).** Cada cifra con el
  comando que la produjo, tal como se recibió:

  - **Registro de 2026-11-16** ([odd/reports/f4-ncc-benchmark.md](../reports/f4-ncc-benchmark.md),
    tabla «Camcraft (medido 2026-11-16)», fila `release`): `unite_ms` 1015,92; `fill_ms` 1,42;
    `clip_ms` 1071,29; `total_ms` **2088,64**; 98 raw contours; 826 clipped boundaries; área
    15825,68 mm². Comando reproducible que el propio reporte documenta:

    ```bash
    cargo test -p cam-ops --release --test ncc measure_ncc_large_plate_kicad_large_pours -- --ignored --nocapture
    ```

    La misma tabla trae la fila `dev/test` de la misma placa: 5449,77 / 3,42 / 5246,78 / **10699,96** ms.

  - **Medición de hoy, tres corridas** del comando anterior sobre la misma placa: `total_ms`
    **7260,08**, **7120,70** y **7101,21** ms, con `unite_ms` ≈ **6100** y `clip_ms` ≈ **1000**.
    Frente a los 2088,64 ms del registro, la operación está **~3,4×** por encima de su propia cifra.

  - **El job `bench` de CI no corre nada.** `.github/workflows/ci.yml` (~`:110-121`) declara el job
    `bench` con `if: github.event_name == 'workflow_dispatch'` y un único paso
    `Bench gate (migration placeholder)` que imprime «Benchmarks deshabilitados durante migración
    Tauri.» — ningún benchmark. Un job que pasa en verde sin medir nada.

  - **`check_regression.sh` no compara.** `src-tauri/crates/cam-geom/benches/check_regression.sh`
    paniquea con «Baseline 'committed' must exist before comparison is allowed».
    `crates/cam-ops/benches/check_regression.sh` —la ruta que documentan los dos `README.md` de
    `benches/`— no existe: `No such file or directory`, código **127**. Y el script restaura su
    baseline en `src-tauri/target/criterion` mientras cargo lee `target/criterion`, así que los dos
    directorios nunca se encuentran.

  - **El paralelismo no es la palanca.** `RAYON_NUM_THREADS=1` → **3,72 ms** frente a
    `RAYON_NUM_THREADS=16` → **3,83 ms** sobre `ops/ncc_fill_rings` (p = 0,06). `rayon` aparece sólo
    en `cam-ops`: `src-tauri/crates/cam-ops/src/ncc.rs:18`, `:141` y sus tests.

  - **El lado FlatCAM de la comparación no se re-midió.** Los 49,87 s de FlatCAM-Plus 1.3.6 del
    reporte son de 2026-11-16, medidos con un venv que no está en este árbol; ninguna cifra de esta
    tarea los vuelve a medir. Por eso el «~23,9× más rápido» del reporte no se sostiene como está
    escrito y no se corrige acá: se corrige en T4 (S6), conservando la cifra vieja como historia.

  Al escribir este documento se re-verificaron en el árbol, sin agregar hallazgos: el placeholder del
  job `bench` en `.github/workflows/ci.yml:110-121`, la existencia de
  `src-tauri/crates/cam-geom/benches/check_regression.sh` y
  `src-tauri/crates/cam-ops/benches/check_regression.sh` (con su `cp` a
  `$ROOT/target/criterion`, donde `ROOT` es `src-tauri`), la inexistencia de
  `crates/cam-*/benches/check_regression.sh` desde la raíz, que la raíz de workspace es la raíz del
  repositorio (`Cargo.toml:1`, con `src-tauri/crates/*` como miembros, así que cargo lee
  `target/criterion`), que el baseline commiteado de `cam-ops` no incluye
  `ops/ncc_fill_rings` ni los dos del índice, los dos usos de `rayon` en
  `src-tauri/crates/cam-ops/src/ncc.rs`, y que `nproc` en esta máquina da **16**.
