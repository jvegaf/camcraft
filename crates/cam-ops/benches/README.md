# cam-ops benchmarks (W1c / E0.7)

Benchmarks Criterion para operaciones de toolpath representativas.

## Ejecutar

```bash
cargo bench -p cam-ops --bench toolpaths
```

## Baseline guardado

El baseline comprometido vive en `benches/baselines/criterion/` (copia de los datos de Criterion guardados con `--save-baseline committed`).

Para regenerarlo:

```bash
cargo bench -p cam-ops --bench toolpaths -- --save-baseline committed --noplot
rm -rf crates/cam-ops/benches/baselines/criterion
mkdir -p crates/cam-ops/benches/baselines/criterion
cp -a target/criterion/ops_* crates/cam-ops/benches/baselines/criterion/
```

## Regresión

`check_regression.sh` restaura el baseline guardado en `target/criterion/` y compara:

```bash
./crates/cam-ops/benches/check_regression.sh
```

Pasa argumentos extra a Criterion después de `--` si hace falta, por ejemplo `-- --noplot`.
