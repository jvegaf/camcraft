# 104 — Reparar la ruta del corpus en tres suites y devolver los crates al gate

Tres tests del workspace resuelven `fixtures/corpus` un nivel de más, así que fallan en cualquier máquina, no sólo en CI. Nadie lo notó porque el gate nunca corrió esos crates: sólo `cargo test --manifest-path src-tauri/Cargo.toml`. Esta tarea arregla la ruta, corre la suite completa y —si queda verde— devuelve los crates del workspace al `gate` de CI.

## Surfaces

- `src-tauri/crates/cam-import/tests/folder.rs`
- `src-tauri/crates/cam-geom/tests/parity.rs`
- `src-tauri/crates/cam-excellon/tests/excellon.rs`
- `.github/workflows/ci.yml`
- `odd/roadmap/workflow.md`
- `odd/roadmap/index.md`
- `odd/tasks/104-corpus-path-repair.md`
- `odd/tasks/INDEX.md`

## Specs

- **S1 (la ruta es la correcta, no la que andaba).** Los tres tests resuelven el corpus como `env!("CARGO_MANIFEST_DIR")` + `../../../fixtures/corpus`, la profundidad que ya usa `src-tauri/crates/cam-ops/tests/ncc.rs:132`: desde `src-tauri/crates/<crate>` eso es la raíz del repo, y `../../fixtures` caía en `src-tauri/fixtures/`, que no existe. Ningún test se saltea con un `if exists` ni con un `unwrap_or`: si el corpus falta, el test falla y lo dice.
- **S2 (la suite del workspace corre entera).** `cargo test --workspace --exclude cam-ui --exclude cam-desktop` termina verde. `cam-ui` y `cam-desktop` quedan fuera porque necesitan display, y eso se dice en el comentario del paso, no se deja implícito.
- **S3 (el gate recupera la cobertura).** Con la suite verde, el `gate` vuelve a correr los tests del workspace en CI, y los documentos que hoy dicen "los crates del workspace no están gateados" se corrigen en `workflow.md` y en la fila `E0.8` del índice.
- **S4 (reparación, no cambio de comportamiento).** El cambio toca sólo la resolución de la ruta de fixtures en tests. Ninguna aserción se debilita, ningún conteo esperado se ajusta para que pase, y si el corpus real no coincide con lo que un test declara, el hallazgo se reporta como hallazgo.

## Tasks

| ID  | Tarea                                                                                                                                         | Specs      | Ruta   | Commit |
| --- | --------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | ------ |
| T1  | Los tres `tests` con la profundidad corregida; `cargo test --workspace --exclude cam-ui --exclude cam-desktop` corrido y reportado con totales | S1, S2, S4 | worker | —      |
| T2  | Con la suite verde, `.github/workflows/ci.yml` vuelve a los tests del workspace, y `workflow.md` más `E0.8` dejan de declarar el hueco          | S3         | worker | —      |
| T3  | Este documento y su fila en el índice                                                                                                          | S1, S3     | worker | —      |

## Log

- **L1 (pedido original, verbatim).**

  ```
  haz los 4 pasos por ese orden
  ```

- **L2 (el defecto, medido).** `gate` de la PR #2 (run `45935064103`, job `140296258398`) falló en 3m34s con `thread 'the_six_corpus_drill_files_match_their_tools_and_hits' panicked at src-tauri/crates/cam-excellon/tests/excellon.rs:291:6: fixtures/corpus is part of the repository: Os { code: 2, kind: NotFound }`. Reproducido en local en 13s con el mismo panic, así que no es el entorno de CI: `env!("CARGO_MANIFEST_DIR")` + `../../fixtures/corpus` desde `src-tauri/crates/<crate>` cae en `src-tauri/fixtures/`, que no existe en el repo ni en disco.

- **L3 (superficie verificada, no estimada).** Sólo tres archivos tienen la profundidad mala: `cam-import/tests/folder.rs:14`, `cam-geom/tests/parity.rs:44` y `cam-excellon/tests/excellon.rs:289`. `cam-ops/tests/excellon_drill.rs:40` ya usa `../../../fixtures/corpus` y `cam-gerber/tests/gerber.rs` no toca el corpus (su `../../../../fixtures/gerber/...` está bien). Una cuenta anterior de cinco archivos venía de un grep que también matcheaba esa ruta larga correcta.

- **L4 (por qué nunca se vio).** El `gate` de CI corría `scripts/check.sh`, que hace `cargo test --manifest-path src-tauri/Cargo.toml` — el paquete host, no los crates del workspace. La tarea 103 amplió el paso a `--workspace`, destapó el defecto y lo revirtió a la cobertura probada para no mezclar la reparación con el cambio de workflow (`odd/tasks/103-single-trunk-workflow.md`, `L12`).
