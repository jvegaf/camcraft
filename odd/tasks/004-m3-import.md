# Feature: 2.5D CAM application — Milestone M3 (import & interchange)

Program plan: `odd/roadmap/index.md` (M3 = "Import & interchange"). This document is the executable
specification for M3 only. Read this file until `## Log` before implementing any task.
Stack: Rust workspace; new crate `cam-import` (geometry in, report out), then `cam-cli`, then `cam-ui`.
Branch: `feat/m3-import`, cut from `master` at `0e12edc`.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — "continua con M3" (this session's instruction).
- **S2** — Roadmap M3 row (verbatim, `odd/roadmap/index.md:140`): "DXF, SVG, PDF, EPS, raster trace;
  validation report of open/self-intersecting vectors".
- **S3** — Decision D2 (user, verbatim option label): "Todo menos 3D y Laser".
- **S4** — Decision D3 (user, verbatim option label): "Formato propio abierto (JSON/ZIP) + import
  DXF/SVG/PDF/EPS".
- **S5** — The import filter list the file picker offers, verbatim: "`Drawing Files
  (*.dxf;*.dwg;*.eps;*.ai;*.pdf,*.skp;*.svg)`", "`Bitmap Image Files
  (*.bmp;*.jpg;*.gif;*.tif;*.tiff;*.png;*.jpeg;*.webp)`". These are the target formats we measure
  ourselves against; the milestone delivers the subset in S2.
- **S6** — Rule 1 of the architecture, in force since M0 (verbatim, `odd/roadmap/index.md:95`): "**`cam-cli`
  is the contract.** Any behaviour that cannot be produced headless through the CLI is not considered
  testable, and therefore not done."

Owner requirements still in force from earlier milestones:

- **S7** — Decision D1 (user, verbatim option label): "Rust + wgpu/egui (o Rust + Qt bindings)".
- **S8** — Decision D6 (verbatim, `odd/roadmap/index.md:287`): "Arcs first-class, no pre-tessellation".

Everything else in this document is a **derived acceptance criterion** for M3 — an engineering
consequence of S1–S8 with no new scope. Each one is falsifiable and is the thing a task is checked
against.

### Derived acceptance criteria (M3)

- **A1** — Import lives in a new crate `cam-import` that depends on `cam-geom` and `cam-model`
  only: it takes bytes and options, and returns geometry plus a report. It never writes files and
  never depends on `cam-store`, `cam-ui` or `cam-post`.
- **A2** — One entry point decides the format from the extension and, when that fails, from the
  content (DXF group codes, `%PDF-`, `%!PS-Adobe`, SVG `<svg`, PNG signature). An unknown format is
  refused naming the extension and listing the supported set.
- **A3** — Every importer returns the same `ImportReport`: format, tolerance, unit scale applied,
  contour count, per-source-construct counts, **curves flattened**, and every skipped construct with
  its reason and its source location. Nothing is dropped silently: a construct that cannot be
  imported is either converted or listed with a reason.
- **A4** — DXF imports `LINE`, `LWPOLYLINE` (with bulges), `POLYLINE`/`VERTEX`/`SEQEND` (with bulges),
  `ARC`, `CIRCLE` and `ELLIPSE`; a bulge becomes an exact `Arc`, never a polyline. DXF layers become
  document layers carrying their DXF names; `$INSUNITS`/`$MEASUREMENT` decide the scale.
- **A5** — A malformed DXF is refused with the offending group-code line number; an unknown entity
  type is skipped and reported, never fatal.
- **A6** — SVG imports `<path>` with the full command set (`M m L l H h V v C c S s Q q T t A a Z z`,
  including implicit repeated coordinate pairs), `<line>`, `<polyline>`, `<polygon>`, `<rect>`
  (including `rx`/`ry`), `<circle>`, `<ellipse>`; `transform` is honoured, inherited through `<g>`
  (`matrix`, `translate`, `scale`, `rotate`, `skewX`, `skewY`, composed left to right); `viewBox`
  plus `width`/`height` scale the drawing to millimetres.
- **A7** — SVG units: unitless lengths are px at 96 dpi; `mm`, `cm`, `in`, `pt`, `pc` and `px` are
  accepted and converted; an SVG with no `width`/`height` uses the `viewBox` as millimetres and says
  so in the report.
- **A8** — SVG circular arcs (`A` with `rx == ry`) become exact `Arc`s. Curves that are not circular
  arcs (cubic/quadratic Béziers, elliptical or rotated arcs) are flattened at the caller's tolerance
  and counted in the report, with the error bound stated (S8, D-M3-2).
- **A9** — PDF imports the vector paths of every page: the object scan is tolerant of a missing or
  damaged xref, `FlateDecode` streams are inflated, and the content-stream operators `m l c v y re h
  S s f F f* B B* b b* n W` are interpreted, with the `cm` transform and the page `MediaBox` origin
  honoured.
- **A10** — PDF constructs that cannot become cut geometry (images, inline images, shadings, Type3
  glyph procedures, embedded files) are skipped and counted. An encrypted PDF is refused by name.
- **A11** — EPS imports PostScript paths: `newpath`, `moveto`, `lineto`, `curveto`, `rlineto`,
  `rcurveto`, `closepath`, `arc`, `arcn`, with `translate`, `scale`, `rotate`, `concat` and
  `setmatrix` tracked through a transformation stack, and `stroke`, `fill`, `eofill`, `clip` closing
  the current path. `%%BoundingBox` places the drawing.
- **A12** — Raster trace reads PNG (8-bit greyscale, RGB, RGBA and palette, non-interlaced) and
  produces closed contours in millimetres: luminance → global threshold → contour extraction. The
  report carries the threshold, the pixel bounding box and the contour count, so the same input
  reproduces the same output.
- **A13** — Trace options: `--threshold <0-255>`, `--invert`, `--dpi <n>` (pixel → millimetre scale),
  `--min-area <mm²>` and `--simplify <mm>`; the same input and options produce byte-identical output.
- **A14** — `cam-cli import <input> [--out <doc.c2d>] [--layer <name>] [--tolerance <mm>]` plus the
  trace options writes a `c2d/document@1` document and prints the report on stdout; exit `0` on
  success and `2` on unreadable, malformed or unsupported input. With no `--out` it prints the report
  only.
- **A15** — `cam-cli check <doc.c2d>` prints a vector validation report: for each entity with problems,
  either an open contour with the gap distance, or each self-intersection with its point and the two
  segment indices. Exit `0` when clean, `5` when the document has vector issues, `2`/`3` for the
  existing input and schema errors.
- **A16** — `cam_geom::validate` decides openness and self-intersection for any `Contour`, arcs
  included, driven by an explicit tolerance and deterministically ordered, so `check` and the shell
  report the same thing.
- **A17** — The shell imports a file by path through one `UiAction` and reports the outcome in the
  status bar (created entities, flattened curves, skipped constructs). A native file dialog is out of
  scope for this milestone (D-M3-6); the path comes from the caller.
- **A18** — Every earlier milestone is unchanged: the whole existing suite stays green, `cam-cli
  emit` output is byte-identical, and `validate`/`inspect`/`postp`/`tools` behave exactly as before.

### Decisions recorded for M3

- **D-M3-1** — No third-party parsers: DXF, SVG, PDF, EPS and PNG readers are hand-written, in the
  spirit of D7 (reimplement the grammar, do not vendor someone else's format handling). The only new
  direct dependency is `flate2`, already in the tree as `zip`'s DEFLATE backend, used for PDF
  `FlateDecode` and PNG `IDAT`. `cam-import` therefore adds no parser, no XML library and no image
  decoder.
- **D-M3-2** — Curves that cannot be arcs are flattened at the caller's tolerance and the count is
  reported (S8 stays true for anything that *is* an arc, and the roadmap's "import report instead of
  silent loss" mitigation applies to everything else).
- **D-M3-3** — Unitless DXF falls back to `$INSUNITS`; when the header has neither `$INSUNITS` nor
  `$MEASUREMENT`, millimetres are assumed and the report says so.
- **D-M3-4** — SVG unitless lengths are px at 96 dpi, per the SVG/CSS convention, not millimetres.
- **D-M3-5** — PDF text is not traced: only path operators produce geometry (the roadmap explicitly
  defers PDF text-to-outline fidelity). A page with text reports the text-showing operators it
  skipped.
- **D-M3-6** — No native file dialog in M3: file choice is the caller's, so the shell stays testable
  headless (S6). `rfd` or equivalent is a later decision.
- **D-M3-7** — A filled path is imported as its boundary contours: the outline is the cut path we can
  define. Stroke and fill are both read as geometry; the report states which operator produced each
  contour group so a reviewer can tell a stroked centreline from a filled outline.
- **D-M3-8** — Raster tracing is deterministic and explainable: global threshold, luminance with
  `0.2126 R + 0.7152 G + 0.0722 B`, contours extracted on the binary mask, and every parameter
  echoed in the report. No adaptive thresholding, no smoothing that depends on iteration order.
- **D-M3-9** — Import never mutates an existing document in this milestone: `import` always builds a
  new document, and merging an import into an open document is a later command (recorded so the
  `cam-app` command surface stays untouched by M3).
- **D-M3-10** — SVG user space is **y-down**, so it is mapped to millimetre space with **Y inverted**,
  exactly like the raster tracer does, and unlike DXF/PDF/EPS which are y-up already. Confirmed by the
  owner (L10). The flip goes through `cam_geom::transform::mirror`, which is exact on arcs and
  normalises winding, never by negating points in a way that would reverse arc bulges.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **T1** — A1, A2, A3, A14 — inline — `feat(import): cam-import crate, format detection and cli import`
- **T2** — A16 — inline — `feat(geom): vector validation for open and self-intersecting contours`
- **T3** — A4, A5 — writer — `feat(import): dxf entities, layers and units`
- **T4** — A6, A7, A8, D-M3-10 — writer (parked work finished in a later session, L11) — `feat(import): svg shapes, paths, transforms and units`
- **T5** — A9, A10 — writer — `feat(import): pdf content stream path import`
- **T6** — A11 — writer — `feat(import): eps postscript path import`
- **T7** — A12, A13 — writer — `feat(import): png raster trace`
- **T8** — A15 — inline — `feat(cli): check command with the vector validation report`
- **T9** — A17 — inline — `feat(ui): import action and report in the status bar`
- **T10** — A18 — inline — `docs: m3 import quickstart and roadmap evidence`

Route notes: T1 and T2 are the shared foundation and are inline. T3–T7 are five importers over five
disjoint module + test + fixture surfaces, each several hundred lines: that is the writer reason
(independent units, launched together), not file count. T8–T10 depend on the importers and are inline.

### Task detail

**T1 — Crate, detection, report, CLI.** New `crates/cam-import` with `Cargo.toml` (workspace deps
`cam-geom`, `cam-model`, `thiserror`, `flate2`, dev `serde_json`) and, in `src/lib.rs`, the whole
public spine:

```rust
pub enum Format { Dxf, Svg, Pdf, Eps, Png }
pub struct ImportOptions { pub tolerance: f64, pub trace: TraceOptions }
pub struct TraceOptions { pub threshold: Option<u8>, pub invert: bool, pub dpi: f64,
                          pub min_area: f64, pub simplify: f64 }
pub struct ImportedLayer { pub name: String, pub contours: Vec<Contour> }
pub struct Imported { pub format: Format, pub layers: Vec<ImportedLayer>, pub report: ImportReport }
pub enum ImportError { Io(String), Unsupported { extension: Option<String>, supported: &'static str },
                       Parse { format: &'static str, message: String, location: Option<String> },
                       Refused { format: &'static str, reason: String }, NothingImported { format: &'static str } }
pub fn detect(path: &Path, bytes: &[u8]) -> Result<Format, ImportError>;
pub fn import_bytes(source: &str, bytes: &[u8], options: &ImportOptions) -> Result<Imported, ImportError>;
pub fn import_path(path: &Path, options: &ImportOptions) -> Result<Imported, ImportError>;
```

`report.rs` holds `ImportReport { format, tolerance, unit_scale, contours, flattened_curves, sources:
BTreeMap<&'static str, usize>, skipped: Vec<Skipped>, warnings: Vec<String> }` with
`Skipped { construct: String, reason: String, location: Option<String> }` and a deterministic
`render() -> String` used by both the CLI and the shell. `dxf.rs`, `svg.rs`, `pdf.rs`, `eps.rs`,
`raster.rs` each start as a module exposing exactly one function — `pub fn import(bytes: &[u8],
options: &ImportOptions, report: &mut ImportReport) -> Result<Vec<ImportedLayer>, ImportError>` —
returning `ImportError::Refused { reason: "not implemented yet" }`; the dispatch in `lib.rs` is
already wired, so every later task fills one module and touches nothing shared. `cam-cli` gains
`import` (A14): reads the input, imports, builds a `Document` with one layer per imported layer and
`E1..En` ids inside `L1..Ln`, sizes the job to the imported bounds plus a 10 mm margin when
`--job-width`/`--job-height` are absent, saves through `cam_store::document::save_document` when
`--out` is given, and prints `report.render()`. Tests: extension detection and content-sniffing
fallback for all five formats; an unknown extension refused naming the supported set; the CLI on a
not-yet-implemented format exits `2` with the refusal on stderr; a round trip `import` → `inspect`
prints the expected entity count; the saved document reloads and validates.

**T2 — Vector validation.** New `cam-geom/src/validate.rs`, exported as `cam_geom::validate`:
`pub fn inspect(contour: &Contour, tolerance: f64) -> ContourIssues` with
`ContourIssues { open: Option<f64>, intersections: Vec<SelfIntersection>, degenerate: Vec<usize> }`,
`SelfIntersection { point: Point2, first: usize, second: usize }`. Rules: a closed contour is never
"open"; an open contour reports its endpoint gap (A16's "open with the gap distance"); every pair of
non-adjacent segments is intersected with `intersect()` and a `Point`/`Points` result is reported
(tangency is not a self-intersection; `Coincident` between adjacent segments is not either);
degenerate means a zero-length segment within tolerance. Deterministic ordering: by `first`, then
`second`, then coordinates. Tests: a figure-eight reports one intersection at the crossing; a circle
and a rectangle report none; an open polyline reports its gap and no intersection; two segments that
merely touch at a shared endpoint are not reported; an arc crossing a line inside the same contour is
reported with its arc index; a property test asserts a convex polygon never reports anything.

**T3 — DXF.** `dxf::import` parses the ASCII group-code stream (code/value pairs, sections
`HEADER`/`TABLES`/`BLOCKS`/`ENTITIES`, `0`-records), reads `$INSUNITS`/`$MEASUREMENT`, maps layer
table entries to `ImportedLayer` names and imports `LINE`, `LWPOLYLINE` (Bulge 42), `POLYLINE` with
`VERTEX`/`SEQEND` (bulge 42), `ARC` (10/20/40/50/51, CCW from start to end), `CIRCLE` (10/20/40) and
`ELLIPSE` (10/20 major axis endpoint 11/21, ratio 40, params 41/42 — a full ellipse becomes
`primitive::ellipse` rotated by the major-axis angle, a partial one is flattened and counted). A
bulge maps to `Vertex::new(point, bulge)` so the kernel's arc is exact. Unknown entity types are
skipped with `Skipped { construct, reason, location: Some("line N") }`. Tests: one test per entity
type asserting the stored geometry (a bulge of 1 on a 10 mm segment is a half-circle of radius 5); an
`ARC` of 90° keeps its centre and sweep; a full `ELLIPSE` is arc-bearing and rotated correctly; a
`TEXT` entity is skipped and reported; a truncated group code is refused with its line number; unit
scaling from `$INSUNITS = 1` (inches) and from `$MEASUREMENT = 0`; a DXF with two layers produces two
`ImportedLayer`s with their names.

**T4 — SVG.** `svg::import` parses the XML subset by hand (elements, attributes, text, comments,
`<![CDATA[`, processing instructions, entity references `&amp; &lt; &gt; &quot; &apos;` and numeric
character references; a mismatched tag is a located parse error), builds a transform stack for
`<svg>`/`<g>`/element `transform`, resolves `width`/`height`/`viewBox` into a millimetre mapping, and
converts shapes and path data into contours. Path parsing follows the SVG grammar including implicit
repeats, `M` followed by implicit `L`, and `Z` closing the subpath; circular arcs become exact
`Arc`s (`Arc::new` with the correct sweep sign from the sweep flag and `large-arc`), everything else
is flattened adaptively at `options.tolerance` and counted. Tests: each shape element produces the
expected bounds; `rect` with `rx` rounds exactly; a path with `M/L/Z` twice produces two contours; an
`A` of `rx == ry` is arc-bearing while an elliptical `A` is a flat polyline with one counted curve; a
`<g transform="translate(10,20) scale(2)">` moves its child exactly; `viewBox` plus `width="100mm"`
gives the expected millimetre size; unitless px uses 96 dpi; a mismatched tag is refused with its
line number.

**T5 — PDF.** `pdf::import` scans `N G obj ... endobj` records tolerantly, resolves indirect references
for page trees when present and falls back to "every object with a `stream` that parses as a content
stream" when the xref is damaged, inflates `FlateDecode` with `flate2`, and interprets content-stream
operators through a graphics-state stack (`q`/`Q`, `cm`, `w`) and a current-path builder, flushing on
the paint operators in A9. `re` adds four corners and closes; `h` closes; `v`/`y`/`c` flatten into
cubics; a path with `n`/`W` and no paint operator is not geometry. `MediaBox` origin is subtracted so
the drawing lands in positive millimetres. Encrypted PDFs (`/Encrypt` in the trailer) are refused by
name. Unsupported constructs are counted: `Do` with an image XObject, `BI…ID…EI` inline images, `sh`
shadings, `Tj/TJ/'/"` text, Type3 font procedures. Tests: a single-page PDF built by the test itself
with an uncompressed content stream imports a rectangle; the same with `FlateDecode` imports the same
contours; a two-page PDF imports both pages' geometry; `cm` scaling and translation are applied; a
`MediaBox` with a non-zero origin puts the path at positive coordinates; `/Encrypt` is refused naming
it; an image `Do` is skipped and counted; a truncated file is refused as a parse error.

**T6 — EPS.** `eps::import` lexes PostScript tokens (names, numbers, strings, `%` comments,
`[ ] { } << >>`), skips the DSC header but reads `%%BoundingBox`/`%%HiResBoundingBox`, and interprets
the path subset in A11 with a transformation stack; `arc`/`arcn` are converted to Bézier or arc
segments from the current point (a full circle is two `Arc`s, a partial arc becomes a sequence of
exact `Arc`s when it is circular, which it always is here). Unknown operators are counted, not fatal.
Tests: a minimal `%!PS-Adobe-3.0` file with `moveto lineto closepath stroke` yields one closed
contour; `rlineto` produces the same contour as the absolute form; `translate/scale` applied to a unit
square gives the expected millimetre size; `arc` of 360° on a 10 mm radius is a circle with two arcs
and the analytic area; `curveto` is flattened and counted; `%%BoundingBox` offsets the drawing; a file
that is not PostScript is refused by name.

**T7 — Raster trace.** `raster::import` decodes PNG by hand: signature, `IHDR` (bit depth 8, colour
types 0/2/3/6, interlace 0, otherwise refused by name), `PLTE`, `tRNS`, `IDAT` concatenated and
inflated with `flate2`, all four scanline filters (None/Sub/Up/Average/Paeth) undone. Luminance →
binary mask by the global threshold (default 128, `--invert` flips the polarity) → marching-squares
contour extraction on the mask (closed contours around foreground regions, holes kept as their own
contours) → `--min-area` discards tiny regions and is reported → `--simplify` runs Douglas–Peucker on
each traced contour and is reported. Millimetres come from `--dpi` (default 96). Determinism: contour
start point, direction and emission order follow the scan order (row-major, first foreground pixel),
so the same input yields byte-identical output. Tests: a hand-built 4×4 PNG with a 2×2 block yields
one contour of the expected perimeter in mm at a known dpi; a donut yields an outer and an inner
contour; `--invert` yields the complement; `--threshold` changes the mask as expected; `--min-area`
drops a single-pixel speck and reports it; `--simplify` reduces vertex count on a staircase and never
moves a vertex by more than the tolerance; a 16-bit or interlaced PNG is refused by name; a truncated
`IDAT` is refused as a parse error.

**T8 — `check`.** `cam-cli check <doc.c2d>` loads the document, runs `cam_geom::validate::inspect` on
every contour with `DEFAULT_TOLERANCE`, and prints one line per issue, sorted by entity id then by
issue order: `E3 open: gap 0.0210 mm`, `E7 self-intersection at 12.500, 4.000 (segments 3, 7)`,
`E9 degenerate segment 2`. A clean document prints `all vectors valid` and exits `0`; a document with
issues exits `5`. Tests through the binary: a document with a figure eight exits `5` with both entity
and point in stdout; an open polyline exits `5` with the gap; a clean document prints the exact
success line and exits `0`; a missing file still exits `2`, an unsupported schema still exits `3`.

**T9 — Shell.** `cam-ui` gains `UiAction::ImportFile { path: String }`; the shell calls
`cam_import::import_path`, replaces the document through `AppState` (one undoable step), and puts the
report summary in the status bar (`imported 12 contours from part.dxf, 3 curves flattened, 1 skipped`).
Tests: a synthetic `UiAction::ImportFile` over a fixture DXF updates the document and the status text;
an unreadable path leaves the document untouched and reports the error; one undo restores the previous
document byte for byte.

**T10 — Docs.** `docs/m3-import.md`: the supported formats and what each one loses, the unit rules
(px = 96 dpi, DXF `$INSUNITS`), the flattening tolerance and its default, the trace options with their
defaults, the `check` exit code `5` and the report line format, and the two CLI synopses. `docs/
format.md` gains the `import`/`check` rows in the CLI contract table. Roadmap M3 row marked delivered
with the evidence table and the deviations.

## Delivery slices

| Slice | Tasks | Subject |
| --- | --- | --- |
| 1 | T1–T2 | Crate spine, report, CLI `import`; kernel validation |
| 2 | T3–T7 | The five importers, in parallel on disjoint surfaces — **T3, T5, T6, T7 delivered in this session; T4 delivered in a later one (L12)** |
| 3 | T8–T10 | `check`, shell import, docs — **delivered** |

Slice boundaries, recorded as they close: slice 1 (T1+T2) closed at `c1a9ac2`, 1705 authored lines across
three commits (plan `86b0e79`, `163a3b7`, `c1a9ac2`). Like M2 the milestone overtakes the ~400-line review
unit deliberately: the plan and the two foundation tasks are separate units, and each later task is its own
commit with its own tests, to be reviewed per slice.

Slice 2 keeps that boundary for the same reason, which is scope and independence, not line count: T3
`cd5b3ba` (1660 lines), T5 `6f8ae3c` (2137), T6 `3c24394` (1809), T7 `ecd2b50` (1539) and T4 `cf8cd07`
(2517) are five different formats over five disjoint module + test surfaces, so the review unit is one
format at a time, and no reader has to hold two grammars in their head to review either.

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `163a3b7` | 10 cam-import tests + 6 CLI tests; detection by extension then signature; report render asserted whole; assembly places geometry at the 10 mm margin with ordered ids |
| T2 | `c1a9ac2` | 10 tests incl. two property tests; adjacent segments never intersected; figure eight reports its crossing; arc index reported |
| T3 | `cd5b3ba` | 25 tests; bulges exact arcs, full ellipse rotated exactly, partial ellipse flattened and counted; `$INSUNITS`/`$MEASUREMENT`/mm-inference; skipped entities carry their line number |
| T4 | `cf8cd07` | 30 tests; XML reader with located errors, the full path grammar, every shape element, inherited transforms, `viewBox` with `xMidYMid meet`, exact circular arcs. The parked work's three root defects fixed (non-advancing path cursor, unitless attribute read as px, contradictory tag-location tests) and the owner's Y convention applied as a reflection in the root mapping (D-M3-10). See L11, L12 |
| T5 | `6f8ae3c` | 16 tests; tolerant object scan, page tree with inherited `MediaBox`, `FlateDecode`, CTM stack, curves counted, encrypted refused by name |
| T6 | `3c24394` | 26 tests; transformation stack, `arc`/`arcn` exact, cubic flattening counted, empty drawing unified to `NothingImported` (`ebb9e1d`) |
| T7 | `ecd2b50` | 16 tests; hand-written PNG decoder (all five filters), deterministic marching squares, holes as opposite-wound contours, `--min-area`/`--simplify` reported |
| T8 | `86fc659`-series | 5 CLI tests; deterministic ordering, exit `5`, `all vectors valid` on a clean document |
| T9 | `041ee97` | 5 tests (2 cam-app, 3 cam-ui); undoable replacement, reseeded id counter, exact status sentence, failed import leaves document and history untouched |
| T10 | this commit | `docs/m3-import.md`, `docs/format.md` contract rows, roadmap M3 evidence |

## Log

- **L1** — Owner instruction (verbatim, this session): "continua con M3". Read as: author the M3
  feature document under ODD and start implementing it, on a feature branch off `master`.
- **L2** — Plan: 10 tasks, branch `feat/m3-import` from `master` (`0e12edc`), baseline suite
  recorded from `scripts/check.sh` before the first write.
- **L3** — Slice 1 evidence: T1 `163a3b7` (crate, detection, report, `import` CLI), T2 `c1a9ac2`
  (`cam_geom::validate`). Gates green after both (`./scripts/check.sh`). One correction found by the
  second gate run: `assert!(len >= 1)` fails clippy's `len_zero`, so the degenerate-contour test asserts
  `!is_empty()`.
- **L4** — Slice 2 evidence: five importers were launched as writers over disjoint surfaces (`src/<fmt>.rs`,
  `tests/<fmt>.rs`, `tests/fixtures/<fmt>/`) in five isolated worktrees, so no two could compile each other's
  half-written module. T3, T5, T6 and T7 landed and merged; T4 (SVG) did not (L7). Merges are `e459251`
  (DXF), `8fb4fd4` (PDF), `7e02978` (EPS), `fa99180` (raster).
- **L5** — Slice 3 evidence: `check` (T8), the shell import action (T9) and the docs (T10). T9 also added
  `AppState::replace_document_undoable`, because `replace_document` is the open-a-file path and clears the
  history: an import the user did not ask for must be one undo away, not a lost drawing.
- **L6** — Contract defect found while reviewing the EPS delivery: `eps::import` had grown its own parse
  error for "a file that names no path", because the T1 spine test only accepted `Refused` or `Parse`. That
  was the test pinning a stub, not the contract. Fixed in `ebb9e1d`: every importer returns an empty layer
  set and the spine turns it into `NothingImported` with one message.
- **L7** — T4 (SVG) blocked, recorded honestly rather than merged. Its worker vanished mid-flight; the work
  on branch `import/svg` (1940 lines of `svg.rs`, 552 of tests) leaves 14 of 29 tests failing on assertions
  and 11 hanging past 60 s, which points at a non-advancing loop in the path-data scanner. Merging a parser
  that can hang and that mirrors its own tests' expectations would poison the branch, so the branch is left
  unmerged as evidence for a later session with a working worker. M3 therefore ships four of the five
  formats S2 names.
- **L8** — Open decision for the owner, found while reading the parked SVG work: it maps SVG user space to
  millimetres **without flipping Y**, so a part would arrive mirrored about the horizontal axis relative to
  how the file renders. DXF, PDF, EPS are y-up already and the raster tracer flips, so SVG would be the one
  format that does not match the artwork. The next session should decide the convention before finishing
  that module, and flip through `cam_geom::transform::mirror` (exact on arcs) rather than by negating points.
- **L10** — Owner decision (verbatim, this session): "orientacion del svg confirmada como tu recomiendas. y el
  svg lo pasamos a una sesion nueva". Two outcomes: S8's counterpart for SVG is now decided as Y-inverted
  (D-M3-10), and T4 leaves this milestone's scope by the owner's choice — the branch `import/svg` and the
  worktree stay as the starting point for that session. M3 therefore closes delivered on four of the five
  formats S2 names.
- **L11** — Owner instruction (verbatim, this session): "vamos terminar de completar m3". Read as: finish the
  one task M3 still owes — T4, the SVG importer — on `feat/m3-import`, starting from the work parked on
  branch `import/svg`.
- **L12** — T4 evidence: `cf8cd07`. The parked branch was rebased onto the current tip by copying its two
  files (`src/svg.rs`, `tests/svg.rs`) into a fresh worktree; of its 29 tests, 5 passed, 14 failed and 10
  hung. Reproduced each hang under a debugger: `PathReader::point` scanned both coordinates into a local copy
  of the cursor, so the run loop never advanced — the scanner's own, not the parser's, defect. Fixed, then
  the 14 assertion failures collapsed to three causes: shape attributes were converted px→mm on a page whose
  user unit is not a pixel (a bare number on a shape is a user unit; only in the root viewport is it px,
  D-M3-4), so a 100 mm page came out at 26.46 mm; and the two tag-location tests contradicted each other, so
  the unclosed-at-end-of-input case keeps the opening line and the mismatched case names both tags at the
  closing line. D-M3-10 was applied as a reflection composed into the root mapping: exact on arcs, winding
  normalised when each contour is built, and the test file now writes the flip down through an `up()` helper
  instead of inheriting y-down expectations. Gates green: `fmt`, `clippy -D warnings`, 550 tests. End-to-end
  through the contract (S6): `cam-cli import` on a 120x80 mm drawing with a rounded rect, a circle, a
  triangle and a `<text>` reports `contours: 3`, one skipped `<text>`, writes the document, `inspect` shows a
  120x95 mm job (bounds plus the 10 mm margin) and `check` prints `all vectors valid`.
- **L9** — Dependency decision taken while planning: `flate2` promoted from a transitive dependency of
  `zip` to a direct one for `cam-import` (PDF `FlateDecode`, PNG `IDAT`); nothing else added, so no
  parser, XML or image crate enters the tree (D-M3-1).
