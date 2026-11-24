# cam-geom benchmarks (W1c / E0.7)

Benchmarks Criterion para operaciones representativas del kernel geométrico.

## Ejecutar

```bash
cargo bench -p cam-geom --bench kernel
```

## Baseline guardado

El baseline comprometido vive en `benches/baselines/criterion/` (copia de los datos de Criterion guardados con `--save-baseline committed`).

Para regenerarlo:

```bash
cargo bench -p cam-geom --bench kernel -- --save-baseline committed --noplot
rm -rf crates/cam-geom/benches/baselines/criterion
mkdir -p crates/cam-geom/benches/baselines/criterion
cp -a target/criterion/geom_* crates/cam-geom/benches/baselines/criterion/
```

## Regresión

`check_regression.sh` restaura el baseline guardado en `target/criterion/` y compara:

```bash
./crates/cam-geom/benches/check_regression.sh
```

Pasa argumentos extra a Criterion después de `--` si hace falta, por ejemplo `-- --noplot`.
