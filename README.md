# camcraft

A CAM application for **PCB fabrication**, written in Rust. One document model and one geometry kernel
serve both entry points: `cam-cli` for headless work, and the editor window on Windows and Linux.

The project pivoted here from general 2.5D machining. What the pivot removed is gone on purpose — the
strategy catalogue and the `toolpath`/`emit`/`simulate` pipeline — and what it kept is the layer a PCB CAM
rests on: arc-native geometry, a document that validates itself, a deterministic store, and an editor you
can drive in CI.

**Status.** The kernel, the document model, the panel importers, the nesters, the tool library and the
editor are in place and tested. The two *board* readers — Gerber RS-274X and Excellon — exist as libraries
and produce copper and holes, but **no command turns a board into a cutting program yet**: that is the work
in progress, and its plan is [`docs/ROADMAP_CAMCRAFT.md`](docs/ROADMAP_CAMCRAFT.md) under the architecture
in [`docs/SYSTEM_DESIGN_CAMCRAFT.md`](docs/SYSTEM_DESIGN_CAMCRAFT.md).

## What works today

**Geometry keeps arcs.** An arc is an arc from the document through the boolean and the offset; nothing
tessellates ahead of an emitter. Point-in-contour is an exact winding number, not a tessellated guess. A
region is a boundary list under even-odd parity with normalization and subtraction. The offset carries a
distance proof measured against every original boundary, which is what stops a ring from gouging a wall no
vertex of it sees. When an offset crosses itself it is resolved on the arrangement and the curve survives;
the largest boundary by area is the one that comes back, and a refusal from the arrangement falls back to
the sampled cleanup instead of failing the cut.

**Gerber RS-274X**, the dialect KiCad writes (`cam-gerber`, a library, no command yet). Polarity is state,
not a rewrite. `G36`/`G37` regions carry native arcs and several contours each. `C` and `R` flashes become
real regions; a circular draw becomes a capsule and a rectangular one its Minkowski sum, because that is
what the aperture does to the path. Apertures that name a macro are built from primitives 1/20/21/4 with
parameter expressions evaluated. `image(tolerance)` returns copper. `LPI` is refused by name rather than
approximated.

**Excellon drill files** (`cam-excellon`, a library): the tool table, the coordinates, the slots, `G90` and
`G91`, and the file's own digit format — the header's `2:6` decides how `X11528623` is read, because
assuming a decimal point multiplies a coordinate by a hundred and nothing complains.

**Panel import and interchange.** DXF, SVG, PDF, EPS and PNG raster trace, each one either converted or
listed with a reason and a source location, so a dropped curve is never silent. A document goes back out as
DXF or SVG (`cam-cli export --format`).

**Nesting, with a certificate per placement.** `cam-cli nest` packs the document's sheets and parts and
stores the placements and one cost record, and each nester states its promise as one predicate instead of
restating arithmetic. Rectangles go through maxrects with a guillotine pass that earns its place: on a
234 × 173 mm board at gap 8 it fits three parts on one sheet where maxrects needs two (`0.5067` against
`0.2534` utilisation). Anything that is not a rectangle goes through a free-profile heuristic that is sound
and measurably incomplete — it refuses a legal placement rather than inventing an illegal one. The one
arithmetic is `Document::nesting_utilisation`, and the report, the stored field and load-time validation
all read it. A nesting that cannot place every instance exits `7` and writes nothing.

**A command language, a cut list and a print sheet.** `cam-cli script file.cam` runs one command per line
against one document (`open`, `sheet`, `part`, `nest`, `validate`, `save`, `parts`, `cutlist`), handing its
tokens to the same flag parser and the same function the matching subcommand calls, so a scripted `nest`
writes the document `cam-cli nest` writes. A refusal is located and named — `line 4:6: --gap must be a
non-negative distance in mm, got -1` — and every mutation passes validation on a clone, so a refused script
leaves the file exactly as it was. `cutlist` writes the placements as CSV; `report` writes a self-contained
HTML print sheet, with an `Omitted` section naming what a sheet cannot honestly show.

**The store.** `.cmc` archives with forward-only migrations and load-time validation, and the tool library
in SQLite (`tools.sqlite` in the application data directory — a directory that still carries the old
product's name). There is one library for every document; on first launch it is seeded with a factory
catalogue of 22 tools across six kinds, speeds and feeds derived by one rule rather than hand-tuned, and a
library you empty stays empty until you press `Restore defaults`.

**Simulation** sweeps a move stream over a heightfield of the stock and reports removed material, gouge,
uncut material and crashes.

**Data-driven post-processors.** The `.pp` dialect is implemented — 13 directives, `VAR` formatting with
the dialect's `1.4`-means-four-decimals spelling and its optional scale factor, 20 output blocks. No
third-party post-processor file is vendored.

**An editor you can drive and test without a GPU.** Pan, zoom about the cursor, zoom-to-fit, grid, picking
by screen distance, marquee selection, snapping, bounded undo/redo, layer visibility; the authoring tools —
line, polyline, arc, circle, ellipse, rectangle, polygon, star, single-stroke text — plus node editing,
join, trim, offset, mirror/rotate/scale, array copy and align. Open, Save and Save as go out as an intent
to the host, so `cam-ui` never links a desktop toolkit, and a save validates first and renames a temporary
sibling into place, so an interrupted save leaves the previous archive on disk.

**Refusal instead of guessing.** A tool that does not fit, an open contour assigned to a profile cut, a post
that cannot express arcs, a document with an unknown schema, an opening that would discard unsaved edits:
each fails loudly with its reason and a stable exit code.

## Quickstart

```bash
just check                 # the three gates: fmt --check, clippy -D warnings, all tests
just run                   # the editor window
just status                # branch, sync with origin, uncommitted files
```

Without `just`:

```bash
./scripts/check.sh
cargo run -q -p cam-cli -- --help          # every command, with its flags
cargo run -q -p cam-cli -- inspect board.cmc
cargo run -q -p cam-cli -- export board.cmc --format svg --out board.svg
cargo run -q -p cam-cli -- tools add tools.sqlite --id endmill6 --diameter 6 --number 1
```

The document schemas, the post-processor grammar and the CLI contract are specified in
[`docs/old/format.md`](docs/old/format.md) — a frozen document that predates the pivot and is still the
reference for what the formats mean.

## Layout

| Crate | Responsibility |
| --- | --- |
| `cam-units` | Millimetres, angles, locale-independent fixed-decimal formatting |
| `cam-geom` | Points, arcs, bulged contours, chaining, tessellation, arc-preserving offset, regions and their exact boundary distance |
| `cam-gerber` | Gerber RS-274X reader for the dialect KiCad writes: polarity, regions, flashes, macro apertures, copper |
| `cam-excellon` | Excellon drill files: tool table, coordinates, slots |
| `cam-ops` | The machining capabilities the PCB product keeps: profile follow with tabs, drilling with pecks and a drill bank, pocket rings with islands |
| `cam-move` | The move vocabulary: the contract between toolpaths and the emitter |
| `cam-model` | Job, layers, entities, toolpaths, schema version, validation |
| `cam-store` | `.cmc` documents and the SQLite tool library, both with migrations |
| `cam-export` | A document back out as DXF or SVG |
| `cam-post` | `.pp` parser and the data-driven G-code emitter |
| `cam-sim` | Material-removal simulation: the stock's heightfield, swept removal, gouge and uncut reporting |
| `cam-import` | DXF, SVG, PDF, EPS and PNG raster import, with a report of what was converted or skipped |
| `cam-cli` | Headless pipeline and the verification surface — a library with a thin binary, so the shell's scripting panel calls the same functions |
| `cam-app` | Application state machine: commands, selection, undo/redo. No UI dependencies |
| `cam-view` | Backend-agnostic scene and grid, picking, marquee and snapping in screen pixels |
| `cam-ui` | `egui` shell: docks, canvas, properties, status bar, authoring tools |
| `cam-desktop` | The native window for Windows and Linux |

Dependency direction is one-way: units → geom → move → model → store, with `post`, `ops`, `gerber` and
`excellon` on the geometry and move vocabulary; `sim` on geom, move and model; `app` on model + geom; `view`
on app, geom and sim; `ui` on view and sim; `desktop` and `cli` on everything. Nothing below `cam-ui` knows
that `egui` exists.

## Testing

1319 tests, all deterministic — including the editor, which runs real `egui` frames headless and asserts on
both the actions they produce and the text they paint:

```bash
cargo test --workspace
```

They cover, in the order of what they protect: the geometry kernel (offset simplicity, arc fidelity, area
monotonicity, winding against a fine tessellation, a distance proof that never keeps a piece it has not
measured), the two board readers against Gerber and Excellon fixtures measured from real KiCad exports,
golden G-code for the post-processor dialects, the simulation's sweep and heightfield, document and
tool-library round trips and migrations, every parser diagnostic by line and column, and the editor's
behaviour — undo restoring a document exactly, picking measured in pixels at any zoom, snapping, and
synthetic clicks, drags, wheel and keys driving the real shell.

The native window is the one thing automation does not cover: it needs a display, so it is verified by
building and by launching it under a compositor.

## Where the plan lives

- [`docs/SYSTEM_DESIGN_CAMCRAFT.md`](docs/SYSTEM_DESIGN_CAMCRAFT.md) — the architecture contract.
- [`docs/ROADMAP_CAMCRAFT.md`](docs/ROADMAP_CAMCRAFT.md) — the PCB phases and their exit criteria.
- [`docs/GAPS_CAMCRAFT.md`](docs/GAPS_CAMCRAFT.md) — the gap ledger: what is measured, what is missing, with numbers.
- [`odd/tasks/INDEX.md`](odd/tasks/INDEX.md) — every ODD feature in the order it entered the project, with status.
- [`odd/roadmap/index.md`](odd/roadmap/index.md) — the history of the pre-pivot product, milestone by milestone.

## Next

The PCB operations themselves: isolation routing, non-copper clearing, drilling and cutout — the geometry
they need now exists, and what is missing is the operation that turns copper into a program and a command
that reads a KiCad export end to end. The readers are tested; nothing calls them yet.


## Desarrollo paralelo con agentes (worktrees)

Cada agente trabaja en su propio worktree aislado, una tarea por agente:

```bash
just worktree N        # crea .worktrees/N y rama feat/N-slug
cd .worktrees/N
just claim N           # reclama la tarea (evita colisiones)
# ... trabajar ...
just check-worktree    # validar estado
just release N         # liberar al terminar/entregar
```

Reglas: un agente = un worktree = una tarea. Las tareas se gestionan en [`odd/tasks/INDEX.md`](odd/tasks/INDEX.md). Ver [`AGENTS.md`](AGENTS.md) y [`odd/README.md`](odd/README.md) para más detalles.
