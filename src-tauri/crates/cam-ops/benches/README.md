# cam-ops benchmarks (W1c / E0.7)

Benchmarks Criterion para operaciones de toolpath representativas.

## Ejecutar

Desde la raíz del repositorio:

```bash
cargo bench -p cam-ops --bench toolpaths
```

## Baseline guardado

El baseline comprometido vive en `src-tauri/crates/cam-ops/benches/baselines/criterion/` (copia de los datos de Criterion guardados con `--save-baseline committed`). Cubre todos los benches declarados en el `criterion_group!` de `toolpaths.rs`: los tres de toolpath, `ops/ncc_fill_rings`, los dos del índice y `ops/ncc_unite_corpus_large_plate` (la unión de cobre de la placa grande del corpus, que es donde se mide la clasificación por anidamiento de NCC).

Para regenerarlo (sólo cuando el costo esperado cambie de verdad, no para hacer pasar una corrida):

```bash
cargo bench -p cam-ops --bench toolpaths -- --save-baseline committed --noplot
rm -rf src-tauri/crates/cam-ops/benches/baselines/criterion
for baseline in target/criterion/ops_*/committed; do
  bench="$(basename "$(dirname "$baseline")")"
  mkdir -p "src-tauri/crates/cam-ops/benches/baselines/criterion/$bench"
  cp -a "$baseline" "src-tauri/crates/cam-ops/benches/baselines/criterion/$bench/"
done
```

## Regresión

`check_regression.sh` restaura el baseline comprometido en `target/criterion/` —la raíz de workspace, que es donde cargo lo lee— y compara:

```bash
./src-tauri/crates/cam-ops/benches/check_regression.sh
```

El script resuelve la raíz del repositorio a partir de su propia ruta, así que funciona desde cualquier directorio de trabajo. Antes de correr comprueba que cada `bench_function` de `toolpaths.rs` tenga su baseline `committed` y falla si falta alguno.

Criterion sale con código `0` aunque imprima «Performance has regressed», así que el script lee la línea `change: [low mid high]` que Criterion imprime para cada bench y termina distinto de `0` cuando el cambio medido (el valor `mid`) supera el umbral. El umbral es **10 % de desaceleración relativa**: un umbral relativo al baseline, sin ningún piso de ruido absoluto, por encima del ruido normal entre corridas y muy por debajo de la regresión de 3,4× que este gate tiene que atrapar. La salida dice contra qué baseline comparó, qué benches cubre y con qué umbral; el mensaje de fallo nombra el bench, el baseline, el umbral y el cambio medido.

Cualquier argumento extra se pasa a Criterion, por ejemplo `--noplot`.
