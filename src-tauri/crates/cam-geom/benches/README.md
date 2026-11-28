# cam-geom benchmarks (W1c / E0.7)

Benchmarks Criterion para operaciones representativas del kernel geométrico.

## Ejecutar

Desde la raíz del repositorio:

```bash
cargo bench -p cam-geom --bench kernel
```

## Baseline guardado

El baseline comprometido vive en `src-tauri/crates/cam-geom/benches/baselines/criterion/` (copia de los datos de Criterion guardados con `--save-baseline committed`). Cubre los dos benches declarados en el `criterion_group!` de `kernel.rs`.

Para regenerarlo (sólo cuando el costo esperado cambie de verdad, no para hacer pasar una corrida):

```bash
cargo bench -p cam-geom --bench kernel -- --save-baseline committed --noplot
rm -rf src-tauri/crates/cam-geom/benches/baselines/criterion
for baseline in target/criterion/geom_*/committed; do
  bench="$(basename "$(dirname "$baseline")")"
  mkdir -p "src-tauri/crates/cam-geom/benches/baselines/criterion/$bench"
  cp -a "$baseline" "src-tauri/crates/cam-geom/benches/baselines/criterion/$bench/"
done
```

## Regresión

`check_regression.sh` restaura el baseline comprometido en `target/criterion/` —la raíz de workspace, que es donde cargo lo lee— y compara:

```bash
./src-tauri/crates/cam-geom/benches/check_regression.sh
```

El script resuelve la raíz del repositorio a partir de su propia ruta, así que funciona desde cualquier directorio de trabajo. Antes de correr comprueba que cada `bench_function` de `kernel.rs` tenga su baseline `committed` y falla si falta alguno.

Criterion sale con código `0` aunque imprima «Performance has regressed», así que el script lee esa salida y termina distinto de `0` cuando un bench supera el umbral. El umbral es **10 % de desaceleración relativa** (`--noise-threshold 0.10`): por encima del ruido normal entre corridas y muy por debajo de la regresión de 3,4× que este gate tiene que atrapar. La salida dice contra qué baseline comparó, qué benches cubre y con qué umbral.

Cualquier argumento extra se pasa a Criterion, por ejemplo `--noplot`.
