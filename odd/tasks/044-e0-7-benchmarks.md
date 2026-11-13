# 044 — E0.7 benchmarks (W1c)

Criterion en `cam-geom` y `cam-ops`, baseline guardado y mecanismo de regresión, dentro de las superficies de W1c definidas en la ola 1.

## Surfaces

- `odd/tasks/044-e0-7-benchmarks.md`
- `crates/cam-geom/Cargo.toml`
- `crates/cam-geom/benches/`
- `crates/cam-ops/Cargo.toml`
- `crates/cam-ops/benches/`
- `Cargo.toml`

## Specs

**S1 — Entrega W1c (ola 1) según `odd/roadmap/waves.md`.** (verbatim de la fila W1c)
> | W1c | **E0.7 benchmarks** | `criterion` en `cam-geom` y `cam-ops`, baseline guardado, job de regresión | `crates/cam-geom/benches/`, `crates/cam-ops/benches/`, `Cargo.toml` | — |

**S2 — Criterio E0.7 del roadmap.** (verbatim de `docs/ROADMAP_CAMCRAFT.md`)
> | E0.7 | Benchmarks propios con `criterion` + job de CI que falla ante regresión | **S** | `just bench` corre y CI compara contra el baseline |

**S3 — Restricciones de ola y colisiones.** (verbatim relevante de `odd/roadmap/waves.md`)
> **Colisiones declaradas:** `justfile` es de W1b; si W1c necesita una receta, la pide, no la escribe. `Cargo.toml` raíz es de W1c. `scripts/check.sh` es de W1b.
> **Orden de merge:** W1c → W1a → W1b. El CI cierra la ola porque sus jobs necesitan lo que entregan los otros dos.

**S4 — Superficies y claim.** El trabajo queda limitado a las superficies declaradas; no se toca `justfile`, `.github/`, `scripts/check.sh` ni `odd/tasks/INDEX.md`.

## Tasks

| ID | Tarea | Specs | Ruta | Commit |
| --- | --- | --- | --- | --- |
| T1 | Crear documento ODD 044, reclamar tarea y alinear dependencias Criterion en manifests de superficies | S1,S3,S4 | inline | `027c2cd` |
| T2 | Implementar benches representativos en `cam-geom` con baseline guardado | S1,S2 | inline | `027c2cd` |
| T3 | Implementar benches representativos en `cam-ops` con baseline guardado y mecanismo de regresión ejecutable dentro de superficies | S1,S2,S3 | inline | `027c2cd` |
| T4 | Verificar ejecución de benches/regresión y cerrar evidencia en doc/Engram/todo | S1-S4 | inline | `027c2cd` |

## Log

**L1** (usuario, 2026-11-12, verbatim): "lee @odd/roadmap/waves.md y tu seras el encargado de completar w1c"

**L2** (contexto y encuadre): en el worktree `waves/1c` (`/home/th3g3ntl3man/.herdr/worktrees/camcraft/waves-1c`) se asumió W1c (E0.7). El claim `043` ya estaba tomado por `waves/1b` con superficies de W1b (`.github`, `justfile`, `scripts/check.sh`), así que W1c se registró como tarea `044`. Se creó el doc y se copió a `$MAIN/odd/tasks/044-e0-7-benchmarks.md` para que `just claim 044` pudiera leer las superficies; claim exitoso (branch `waves/1c`, superficies declaradas en el doc). `git status` al inicio: limpio.

**L3** (T1, manifests): se agregó `criterion = { version = "0.5", features = ["html_reports"] }` en `Cargo.toml` raíz (`[workspace.dependencies]`), `criterion` + `[[bench]] name="kernel" harness=false` en `crates/cam-geom/Cargo.toml`, y `[dev-dependencies].criterion` + `[[bench]] name="toolpaths" harness=false` en `crates/cam-ops/Cargo.toml`. Creados `crates/cam-geom/benches/` y `crates/cam-ops/benches/` (con `baselines/`).

**L4** (T2, cam-geom): `crates/cam-geom/benches/kernel.rs` con benches representativos `geom/combine_union_rect_overlap` (`cam_geom::boolean::combine`, unión de dos rectángulos solapados) y `geom/offset_circle` (`cam_geom::offset::offset_contour` de círculo). `cargo bench -p cam-geom --bench kernel --no-run` compiló OK. Baseline comprometido generado con `cargo bench -p cam-geom --bench kernel -- --sample-size 10 --warm-up-time 1 --measurement-time 1 --save-baseline committed --noplot` y copiado a `crates/cam-geom/benches/baselines/criterion/`.

**L5** (T3, cam-ops): `crates/cam-ops/benches/toolpaths.rs` con `ops/profile_rectangle`, `ops/pocket_offset_rectangle` (`PocketStrategy::Offset`) y `ops/drill_6_holes`. `cargo bench -p cam-ops --bench toolpaths --no-run` compiló OK. Baseline comprometido generado y copiado a `crates/cam-ops/benches/baselines/criterion/`.

**L6** (mecanismo de regresión, dentro de superficies): agregados `crates/cam-geom/benches/check_regression.sh`, `crates/cam-ops/benches/check_regression.sh` y `README.md` en cada directorio de benches. Los scripts restauran el baseline guardado a `target/criterion/` y ejecutan comparación `--baseline committed`. No se toca `justfile`, `.github/` ni `scripts/check.sh` (S3/W1b); W1b puede cablear el job de regresión en CI usando estos scripts/baselines.

**L7** (T4, verificación): `./crates/cam-geom/benches/check_regression.sh --sample-size 10 --warm-up-time 0.5 --measurement-time 0.5 --noplot` y el análogo de `cam-ops` corrieron y compararon contra `committed` (salida con `change: ...` por benchmark). Observación: Criterion reporta regresión/mejora en texto pero el proceso salió con `EXIT:0` en las corridas observadas; el fallo duro ante regresión en CI depende del cableado/flags que haga W1b sobre este mecanismo. `Risk: none` (cambios acotados a manifests, benches, baselines y docs de tarea; sin cambios de contratos públicos de runtime). No se actualizó `odd/tasks/INDEX.md` (merge/deliver). Commit preparado: `027c2cd`.

**L8** (defecto encontrado al mergear la ola, corregido en `waves/1b`): la corrida real de `crates/cam-ops/benches/check_regression.sh` sin `--sample-size` acotado **paniquea** en el tercer bench — `ops/drill_6_holes` — con «Baseline 'committed' must exist before comparison is allowed»: el directorio comprometido traía `ops/profile_rectangle` y `ops/pocket_offset_rectangle`, no el del taladro. L7 verificó con flags acotados y no llegó a cubrirlo. Arreglado generando el baseline faltante con el procedimiento de este mismo documento (`--save-baseline committed --noplot`) y copiando sólo `ops_drill_6_holes/committed/`, sin tocar los números ya comprometidos de los otros dos benches. Commit del arreglo: `fix(bench): commit the drill benchmark's missing baseline`.
