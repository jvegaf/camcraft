# Development Plan — the reference CAM application-class 2.5D CAM application (Windows + Linux)

Status: **plan, not implementation**. Derived from `evidence/analysis-report.md` (REA evidence on
`etc/the reference CAM applicationProTrialEdition_Setup.exe`, the reference CAM application).

Owner decisions taken at plan time (verbatim from the decision record, see `## Log`):

| # | Decision |
| --- | --- |
| D1 | Stack: **Rust**, viewport **wgpu**, UI shell `egui`/`cxx-qt` |
| D2 | Scope: **all toolpath strategies except 3D machining and Laser** |
| D3 | Document format: **own open format (ZIP + JSON)**, interop via DXF/SVG/PDF/EPS import |
| D4 | Nesting: **own heuristic engine**, in v1 |

---

## 1. Product definition

A cross-platform desktop application for 2.5D CNC routing: draw or import 2D vectors, assign
machining strategies that produce toolpaths with controlled depth passes, simulate material removal,
and emit machine-specific G-code through user-editable post-processors.

Non-goals for this program (explicit):

- Binary or project compatibility with the vendor `.crv` files (OLE2 internals are unresolved — see
  analysis report §2.5, Q3).
- 3D model machining (STL relief roughing/finishing), PhotoVCarve, sculpting.
- Laser strategies (`Laser Cut - Fill`, `Laser Sketch Engraving`).
- Windows-only APIs: no MFC, no GDI, no Codejock, no `WINSPOOL` printing path.
- the vendor Lua gadget compatibility (a scripting surface is planned, not their dialect).

## 2. Stack decision and its justification

`Rust` + `wgpu` was chosen. The evidence supports it: every domain subsystem in the original
(tool database, post-processor DSL and store, toolpath math, nesting API) is plain portable logic
with no Windows dependency, while the *only* genuinely platform-bound parts are the UI shell
(`mfc140u` + Codejock Xtreme Toolkit Pro + 1990s Win32 GDI) and the 3D viewport (OpenSceneGraph /
OpenGL). Replacing those two is exactly what a cross-platform requirement demands.

**UI shell sub-decision (resolved inside D1):** `egui` via `eframe` for M1, with the shell kept
strictly behind an adapter boundary (`cam-ui` depends on `cam-app`; `cam-app` depends on nothing
UI-related). Revisit trigger — switch to `cxx-qt` (Qt 6) if any of these happen: native file-dialog
or OS integration requirements become blocking, docking/table-heavy panels dominate the UI work, or
text input/IME becomes a recurring defect source. Because the domain never imports the shell, that
switch must not touch a single toolpath or post-processor line.

Why the alternatives lose on this evidence:

- **C++20 + Qt 6** reproduces the original's profile but carries its cost: manual memory discipline
  across a geometry kernel and a parallel toolpath engine, and the slowest path to a first G-code.
- **Python + PySide6** is the fastest to a UI, but the expensive operations here (repeated polygon
  offsetting, medial axis for V-Carve, per-pixel simulation) are precisely where interpreted code
  hurts, and a native extension calls Rust/C++ anyway.
- **TypeScript + Tauri** puts CAD hit-testing, snapping and multi-million-segment rendering behind a
  canvas/IPC boundary, which is the wrong place to add latency in an interactive editor.

## 3. Architecture

Cargo workspace, one binary, no dynamic plugin loading. The layout follows the boundary that makes
the program testable **without a UI**:

```
crates/
  cam-units/     Length, Angle, UnitSystem; millimetre-exact conversions; formatting policy
  cam-geom/      2D kernel: Point2, LineSeg, Arc (center/radius/direction), Polyline with bulges,
                 Bezier; chaining, winding, self-intersection, validation, bounding volumes,
                 spatial index (rstar), offsetting (arc-preserving), boolean ops, tessellation
  cam-model/     Document model: Job (size, thickness, origin, units, safe Z), Layer, Entity,
                 Toolpath, ToolRef; IDs; schema version; migrations
  cam-store/     Persistence: ZIP+JSON document read/write, SQLite tool library, SQLite post-processor
                 store; both versioned with forward-only migrations
  cam-toolpath/  Strategy trait + implementations; pass planning; lead-in/out; tabs; ordering;
                 rapid/feed move stream (the single output type every strategy produces)
  cam-post/      `.pp`-compatible DSL: lexer, directive parser, VAR format spec, 15 output blocks,
                 emitter; user post-processor directory; validation with actionable diagnostics
  cam-sim/       Material-removal simulation: heightfield (dexfuture: dexel) + per-tool removal;
                 reports gouges and uncut material
  cam-import/    DXF, SVG, PDF, EPS, raster image tracing, CSV part lists
  cam-nest/      Sheets/parts model + heuristics (maxrects + guillotine for rectangles,
                 no-fit-polygon for free profiles)
  cam-app/       Application state machine: commands, undo/redo, selection, job lifecycle. No UI deps
  cam-ui/        egui shell: docks, property grid, tool database editor, toolpath forms
  cam-view/      wgpu renderer: 2D scene (arcs, curves, toolpaths, grid, snap markers) + 3D view
  cam-cli/       Headless pipeline: `document -> toolpaths -> gcode|svg|sim-report`. The verification
                 surface; also the CI regression harness
```

Two rules make the rest of the plan enforceable:

1. **`cam-cli` is the contract.** Any behaviour that cannot be produced headless through the CLI is
   not considered testable, and therefore not done. Tests assert on exit code, exact stdout/stderr
   and produced files — never on internal state.
2. **Determinism is a feature.** Identical input must produce byte-identical G-code. No `HashMap`
   iteration may reach an output path; ordering is explicit (`BTreeMap`, sorted IDs, stable sorts).

### 3.1 Cross-cutting technical decisions

| Topic | Decision | Rationale from evidence |
| --- | --- | --- |
| Units | Internal geometry is `f64` **millimetres**. Document stores the user's unit preference; rounding happens **only** at post-processor emission using the `VAR ... |decimals` spec | The original carries unit pans per variable (`[X|C|X|1.4]` vs `1.3`) and mixed metric/imperial sheet libraries; rounding early would compound error across passes |
| Arcs | Arcs are **first-class entities**, not tessellated polylines. Polylines carry nonzero bulges | Native G2/G3 output exists in 8 of the 15 post-processor blocks; flattening at import would make arc output impossible |
| Tessellation | Only at render and simulation time, never in the toolpath pipeline | Keeps emitted geometry equal to authored geometry |
| Tolerance | A single `Tolerance` value threaded through geometry ops, exposed as a job setting | The original exposes a 2D toolpath tolerance string; a global tolerance prevents contradictory results |
| Tool/toolpath model | Tool geometry, cutting data and machine are **separate** records joined per (material, machine) | Observed SQLite schema `tool_geometry` / `tool_cutting_data` / `tool_entity` — the correct normalization, and the reason a tool library is valuable at all |
| Post-processor | Implement a reader compatible with the observed `.pp` dialect (13 directives, 17 variables, 15 blocks, `[var|A/C|prefix|decimals]`) **plus** our own authoring format | A data-driven emitter is the difference between one machine and seventy-five; grammar recovered in analysis report §2.2 |
| Post-processor licensing | **Do not vendor the vendor's `.pp` files.** Ship our own fixtures; support an optional user-supplied directory | Those files are third-party content; the *grammar* is what we reimplement |
| Undo/redo | Whole-document snapshots (documents are kilobytes by nature) with a bounded history and coalescing | Command inversion across geometry edits is a defect factory; snapshot + structural sharing is cheap here |
| Parallelism | `rayon` for independent toolpaths, pass carving and simulation tiles | The original uses OpenMP (`VCOMP140.DLL`, 18 imports) with the same coarse-grained structure |
| Text | `cosmic-text`/`ttf-parser` → outlines at authoring time, stored as curves | Complex-script shaping is a solved dependency; never reimplement it |

### 3.2 File formats we own

Document `.c2d` (ZIP container, `document.json` at root, assets under `assets/`):

```json
{ "schema": "c2d/document@1",
  "job": { "width": 1219.0, "height": 2438.0, "thickness": 18.0, "units": "mm",
           "origin": "bottom_left", "safe_z": 5.0, "xy_origin_corner": true },
  "layers": [ { "id": "L1", "name": "Part 1", "visible": true, "entities": [ ... ] } ],
  "toolpaths": [ { "id": "TP1", "strategy": "profile", "entity_refs": [...],
                   "tool_geometry_id": "...", "params": { "side": "outside", "pass_depth": 6.0,
                   "allowance": 0.0, "tabs": [ ... ] } } ],
  "tools_ref": "tools.sqlite", "postp": "Mach3-Arcs.pp" }
```

Tool library `.sqlite` mirrors the decoded schema (`tool_tree_entry`, `tool_geometry`,
`tool_cutting_data`, `tool_entity`, `material`, `machine`, `tool_name_format`) plus a
`schema_version` table using the same forward-only migration style the original already proves.

## 4. Roadmap

Milestones are sequential; each is its own ODD feature document created when the milestone starts.
Every milestone ends with a demonstrable artefact through `cam-cli`, not a slide.

| M | Milestone | Deliverable (verifiable) | Why here |
| --- | --- | --- | --- |
| **M0** ✅ | Foundation + vertical slice | **Delivered.** `cam-cli emit part.c2d --post fixture.pp --library tools.sqlite` produces a byte-for-byte tested G-code program for an outside profile, with arcs preserved, tabs, pass chunking and validation on every input | Proves units, geometry, doc format, tool library, post-processor emitter end-to-end before any UI exists |
| **M1** ✅ | 2D editor shell | **Delivered.** Viewport with pan/zoom-about-cursor/fit, grid, picking by pixel distance, marquee select, snapping, undo/redo, layer dock, properties and status bar, plus a native window | Geometry only becomes usable with a viewport; all of it rides on `cam-app`, proven headless |
| **M2** ✅ | Vector authoring & editing | **Delivered.** Line/polyline/arc/circle/ellipse/rectangle/polygon/star, single-stroke text, node editing, join/weld/trim, offset, mirror/rotate/scale, array copy and align — as commands through `cam-app`, in the shell and in the native window | The authoring half of "similar to the reference CAM application"; all of it rides on `cam-geom`, proven headless |
| **M3** ✅ | Import & interchange | **Delivered.** DXF, SVG, PDF, EPS and raster trace import with a full report; `cam-cli import` builds the document, `cam-cli check` reports open, self-intersecting and degenerate vectors, and the shell imports undoably (see §4.5) | Users bring geometry; the original's own filter list is the target set |
| **M4** ✅ | Core toolpaths | **Delivered.** Profile (inside/outside/on, tabs, passes, lead-in/out) from M0, plus offset and raster pocket with allowance, stepover and clear stepover, and drilling with peck, both retract modes and drill-bank selection; every strategy is authored through `cam-cli toolpath` and reaches G-code through `cam-cli emit` (see §4.6) | The three strategies that define 2.5D routing; they exercise every post-processor block |
| **M5** ✅ | Simulation & 3D preview | **Delivered.** `cam-sim` sweeps the emitted `MoveStream` over a heightfield of the stock, derives the depth each cell was authorised to lose from the document alone, and reports removed volume, through-cut cells, gouge, uncut material and every crash; `cam-cli simulate` is the contract, with exit `6` for a program that gouges, leaves material standing or rams the tool; a depth-shaded 3D stock view with an animated playback is in the shell; and every strategy M4 ships has an oracle (see §4.8) | G-code without simulation is unverifiable by the user; also the only honest acceptance test for M4–M7 |
| **M6** | Depth-variant strategies | Quick Engrave, Chamfer, Fluting, Texture, Thread Milling, Inlay (male/female pair) | All are profile/pocket variants with a different Z law — cheap once M4 and M5 exist |
| **M7** | V-Carve | Depth from medial-axis distance to boundary, per V-bit included angle, corner relief | Research-grade geometry (medial axis of arcs); deliberately isolated so it cannot delay the rest |
| **M8** | Moulding / swept profile | Rail + profile sweep with stepover | Needs the 3D-adjacent math from M7's distance field |
| **M9** | Nesting | Sheets + parts model, rectangle maxrects+guillotine, free-profile NFP, cost/summary report | Own heuristic engine as decided (D4); depends on stable geometry and toolpaths |
| **M10** | Documents & scripting | HTML print sheets, CSV import for part lists, scripting surface (Rhai/Lua) with a UI panel | Reporting and extension points; the original's gadget platform is the model |

Deferred / conditional, not scheduled: PDF text-to-outline fidelity beyond vector paths, EPS edge
cases, rotary wrapping (`ROTARY_WRAP_X/Y` in the DSL is parsed but wrapping toolpaths are a later
milestone), drill-bank hardware integration, i18n.

### 4.1 Why V-Carve is its own milestone and not "just another strategy"

V-Carve computes cut depth from the distance between a point and the nearest boundary, scaled by the
V-bit's included angle, which means the toolpath is not a fixed-Z pass but a depth field. That
requires a medial-axis/straight-skeleton computation on geometry containing arcs, plus corner-relief
handling. The original's binary contains the strategy but the algorithm is not recoverable from static
analysis — this is a genuine research task, and putting it inside M4 would have hidden a multi-week
unknown inside an otherwise mechanical milestone.

### 4.2 M0 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Nine-crate workspace, one-way dependencies | `Cargo.toml`, `crates/` | `./scripts/check.sh` green: `fmt --check`, `clippy -D warnings`, 123 tests |
| Millimetre-exact units, locale-free formatting | `crates/cam-units` | 10 tests, 3 property tests |
| Arc-first-class 2D kernel with arc-preserving offset | `crates/cam-geom` | 25 tests, 7 property tests; a circle offset stays two arcs |
| Document model with `c2d/document@1` | `crates/cam-model` | 9 tests; invalid geometry cannot be deserialised |
| `.c2d` ZIP+JSON store, atomic writes | `crates/cam-store` | 8 tests; unknown archive entries survive a round trip |
| SQLite tool library with migrations | `crates/cam-store` | 7 tests, incl. a v1→v2 migration of a legacy file |
| `.pp` parser (13 directives, 16 blocks) | `crates/cam-post` | 18 tests, every rejection with its `line:column` |
| Data-driven emitter | `crates/cam-post` | 13 tests, incl. the golden program and the arc refusal |
| Profile strategy | `crates/cam-toolpath` | 20 tests: sides, allowance, passes, tabs, leads, origins |
| Headless CLI with the documented exit codes | `crates/cam-cli` | 12 tests, golden G-code through the real binary |

Known M0 limits, recorded so they are not discovered later: offsetting is not arrangement-based (a
shrinking offset that would pass through the shape is refused, not topologically resolved); tabs split
arcs into sub-arcs but the tab boundary itself is a straight Z move; `FEED_RATE_CHANGE` is parsed but
never emitted because no M0 strategy changes feed mid-segment.

### 4.3 M1 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Application state with exact snapshot undo/redo, bounded history | `crates/cam-app` | 12 tests; a command sequence undone five times equals the original document byte for byte |
| Camera: world/screen mapping, zoom about the cursor, clamping, fit | `crates/cam-app` | 11 tests incl. a property test over the round trip |
| Commands: job size, rectangle, circle, delete, layer visibility | `crates/cam-app` | 10 tests; refusals never reach the document |
| Backend-agnostic scene and grid | `crates/cam-view` | 10 tests; arcs stay arcs, hidden layers disappear from scene and picking |
| Picking, marquee and snapping in screen pixels | `crates/cam-view` | 13 tests incl. tie-breaks and zoom independence |
| egui shell: panels, canvas, properties, status | `crates/cam-ui` | 16 tests running real `egui` frames headless and reading back painted text |
| Pointer, wheel and keyboard wiring | `crates/cam-ui` | 10 tests with synthetic clicks, drags, wheel and keys |
| Native window for Windows and Linux | `crates/cam-desktop` | builds; launched under a compositor, screenshot in `docs/images/editor-m1.png` |

M1 defects found and fixed at the root: the entity id counter ignored ids already in the opened
document (duplicate `E1`); arc bounds were computed by tessellation, so a circle's bounds were off by
the chord tolerance and every frame re-tessellated every arc; the status bar showed the previous
frame's cursor because panels are laid out before the central area; and auto-framing fitted a
provisional canvas before the panel widths settled.

Recorded deviation from this roadmap's crate description: `cam-view` computes a backend-agnostic
`Scene` and the shell paints it with `egui`; a `wgpu` backend will consume the same `Scene` when the
3D simulation (M5) needs the GPU. See the M1 feature document, decision D-M1-2.

### 4.4 M2 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Seven exact primitive builders (line, polyline, rectangle, circle, regular polygon, star, arc through three points) | `crates/cam-geom` | 20 tests; analytic area, perimeter and winding, no pre-tessellation |
| Ellipse approximated by arcs with a declared error | `crates/cam-geom` | 10 tests; tolerance met up to 100:1 and re-checked by independent sampling; an unmet tolerance is reported, not hidden |
| Translate, rotate, uniform scale and mirror, exact on arcs; non-uniform scale reports what it cost | `crates/cam-geom` | 15 tests incl. two property tests; winding normalised; the sagitta reported is measured against the true curve |
| Tolerant line/arc intersections, with tangency and coincidence distinguished | `crates/cam-geom` | 18 tests; a tangent is one point and cannot divide a contour |
| Node editing that projects the click onto the segment and splits an arc exactly | `crates/cam-geom` | 14 tests; the halves keep the original centre, radius and total sweep |
| Join with named refusals, closing a ring when the ends meet | `crates/cam-geom` | 12 tests; forks, two-chain groups and closed inputs refused by name |
| Deterministic trim | `crates/cam-geom` | 7 tests; arcs split exactly, four refusal classes |
| Every authoring action as one atomic command through `AppState` | `crates/cam-app` | 73 tests in the command suite; refusals leave document and history depth untouched; one undo per command |
| `lost_arcs` aggregated from the kernel to the caller | `crates/cam-app` | reported in `CommandOutcome` and shown in the status bar |
| Single-stroke text, no new dependency, behind a `GlyphSource` trait | `crates/cam-geom` | 10 tests; 79 glyphs; an unsupported character is refused by name |
| Authoring tools in the shell, drag to draw, outcome in the status bar | `crates/cam-ui` | 18 tests, headless: one command per gesture, a too-short gesture cancelled, one test per action group |
| Native window wired through the same outcome path | `crates/cam-desktop` | builds; the host applies actions through `Shell::apply_action` |

M2 defects found and fixed at the root: `serde_json` parsed floats one ULP off, so the documented
lossless round trip of `c2d/document@1` was false (fixed with `float_roundtrip`); `offset_contour`
reported an open-contour precondition as "offset collapsed the contour", a message about the wrong
thing, and the guard had no test at all; and the desktop window bypassed the shell's outcome path, so
created counts, ellipse error and lost-arc counts never reached the user outside the tests.

Recorded deviations from this milestone's description: text is a single-stroke glyph table rather than
a TrueType renderer, decided with the owner as D-M2-1; a non-uniform scale tessellates and reports the
cost rather than refusing, decided as D-M2-2. Both are in the M2 feature document with their
acceptance criteria. Known limit carried forward: `Contour::bounds()` still samples arcs through
tessellation, so bounds, align and the lost-arc counts are accurate to about 1e-6 mm on arc-bearing
selections rather than exact.

### 4.5 M3 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Import contract: bytes + options in, millimetre contours + report out, no filesystem and no document mutation | `crates/cam-import` | 10 tests; detection by extension then by content signature; the report renders whole and identically every time |
| Document assembly: stock fitted to the imported bounds plus a 10 mm placement margin, `L1..Ln`, `E1..En` | `cam-import::into_document` | 3 tests; exact placement and job size, single-layer override, `validate()` runs on the result |
| DXF: `LINE`, `LWPOLYLINE`/`POLYLINE` with bulges, `ARC`, `CIRCLE`, `ELLIPSE`, layers, `$INSUNITS`/`$MEASUREMENT` | `cam-import::dxf` | 25 tests; a bulge is an exact arc, a full ellipse is rotated exactly, a partial one is flattened and counted, skipped entities carry their line number |
| SVG: hand-written XML reader, the shape set, inherited `transform`, `viewBox` scaling, exact circular arcs, y inversion (D-M3-10) | `cam-import::svg` | 30 tests; a circular `A` stays an `Arc` and an elliptical one is flattened and counted, the path grammar covers relative and implicit repeats, `viewBox` + `width` gives the exact millimetre size, the reflection keeps both of a circle's arcs, a mismatched tag is located |
| PDF: tolerant object scan, page tree with inherited `MediaBox`, `FlateDecode`, content-stream paths through `cm` | `cam-import::pdf` | 16 tests; compressed and uncompressed streams give identical contours, two pages import in order, encryption is refused by name |
| EPS: PostScript path subset with a transformation stack | `cam-import::eps` | 26 tests; `arc` of 360° is a closed circle of two arcs, `curveto` is flattened and counted, `%%BoundingBox` places the drawing |
| Raster trace: hand-written PNG decoder plus deterministic contour extraction | `cam-import::raster` | 16 tests; all five scanline filters, holes wound the opposite way, `--min-area`/`--simplify` reported, same bytes give the same contours |
| Vector validation in the kernel: open gap, self-intersections with segment indices, degenerate segments | `cam-geom::validate` | 10 tests incl. two property tests; adjacent segments are never intersected, so a valid polygon is clean |
| `cam-cli import` and `cam-cli check`, with exit `5` for vector issues | `crates/cam-cli` | 10 binary tests; exact stdout, exit codes, a conversion written and reloaded |
| Shell import: `UiAction::ImportFile`, undoable replacement, outcome in the status bar | `crates/cam-ui`, `crates/cam-app` | 5 tests; one undo restores the previous drawing, a failed import changes nothing, the status sentence is asserted whole |

M3 defects found and fixed at the root: `eps::import` had invented its own parse error for a file that
draws nothing, because one spine test only accepted `Refused` or `Parse` — the test was pinning the stub,
not the contract; every importer now returns an empty layer set and the spine reports `NothingImported`
(`d93fd01`). Recorded, not fixed: `replace_document` clears history, so the import path needed
`replace_document_undoable` rather than reusing it.

**SVG (T4) closed in a later session.** The parked work on branch `import/svg` was the starting point, and
its three real defects were found and fixed at the root: the path-data reader scanned each coordinate pair
into a throwaway cursor, so it never advanced and every `d` attribute hung; a unitless number was read as a
CSS pixel on shape attributes too, so a 100 mm page came out at 26.46 mm; and `point()` borrowed `self`
immutably, which is what made the first defect possible. The owner's decision that SVG user space is
**y-down** is now D-M3-10: the root mapping composes a reflection, which is an isometry, so arcs stay exact
with their bulge sign flipped and no curve is flattened for it. One parked test asserted both that a
mismatched tag is located at the *closing* tag and that an unclosed one is located at the *opening* tag; the
two now test their own case, and a genuinely unclosed element at end of input points at the line it opened
on. The parked suite's y expectations were written before the flip and now state it through an `up()`
helper, so no assertion hides the convention.

Recorded deviation: PDF imports one user unit as one millimetre with a warning, because a PDF path carries
no physical size and A9 names no scale. Known limit carried forward: a `W f` clip-and-fill in PDF loses the
fill, because a clip discards the path.

### 4.6 M4 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| Two new `Operation` variants inside `c2d/document@1`: `Pocket(PocketParams)` and `Drilling(DrillParams)`, with `PocketStrategy::{Raster, Offset}` and `RetractMode::{Amount, Full}` | `crates/cam-model` | 6 tests; both kinds round-trip losslessly, `RetractMode::Amount` serialises as `{"mode":"amount","amount":…}`, an absent `bank` is omitted, and an unknown `kind` still fails to load |
| Closed-contour validation generalised from profile to every operation that cuts one | `crates/cam-model` | the existing error variant now names pocket and drilling toolpaths by entity, and every M0–M3 document still loads |
| `circle_centre`, and one shared depth rule extracted from profile | `crates/cam-toolpath` | 8 tests on `circle_centre` (2- and 4-arc circles either winding; lines, mixed shapes, open contours and arcs with different centres refused); `pass_depths` shared by profile, pocket and drilling |
| Offset pocket: rings offset from the original boundary, outermost first, each proving its distance | `crates/cam-toolpath::pocket` | 12 tests; 12 rings at insets 3, 7, … 47 on a 100 mm square with a 6 mm tool at 4 mm stepover; `[5, 10, 12]` pass chunking with every ring cut once per pass; arcs kept as arcs in rings; a wall-gouging ring ends the ringing instead of being emitted |
| Raster pocket: rows on the `min.y + k·stepover` grid, clipped by true line/arc intersections | `crates/cam-toolpath::pocket` | 32 rows at y = 3, 6, … 96 on the same square at 3 mm stepover; analytic chord widths on a circle within 1e-6; a tangent row emits no unpaired span; no arc moves in raster mode |
| Drilling: circle-only holes, nearest-neighbour tour, pecks, both retract modes | `crates/cam-toolpath::drill` | 13 tests; `order = [h1, h2, h3, h4]` and `rapid_length = 110 mm` on a 20 mm square; a 3 mm peck on a 7 mm cut plunges at −3, −6, −7; `Full` reaches safe Z and `Amount(1)` does not |
| Drill bank as selection with head compensation, and no machine command | `crates/cam-toolpath::drill` | 7 tests; `drill_index = [Some(0), Some(0), Some(1), Some(2)]` for holes at x = 0, 10, 40, 100 with 3 drills at 32 mm; `bank_travel = 60 mm` against `rapid_length = 120 mm`; a one-drill bank emits the un-banked program move for move |
| `cam-cli toolpath`, plus `inspect` and `emit` for the new kinds | `crates/cam-cli` | 9 tests; appends and reloads, ids are deterministic, the library's `stepover`/`clear_stepover`/`stepdown` supply the defaults, every refusal exits `2` with the file byte-identical, `--out` leaves the input alone, `inspect` names each kind and strategy |
| Golden G-code for pocket and drilling, and a closed error set | `crates/cam-cli`, `crates/cam-toolpath` | 5 tests; a 42-line program asserted whole three ways (as built, after a c2d round trip, and with both toolpaths authored through `cam-cli toolpath`); an exhaustive `match` over every `ToolpathError` with its exact text, so a stub cannot return as a contract |
| 61 tests over four slices, **610 in the workspace** | `crates/` | `cargo test --workspace` green: 23 + 12 + 13 + 7 + 5 added, one T3 placeholder removed with them |

M4 defects found and fixed at the root: `RetractMode` could not be a tagged newtype over a length, because
a bare number cannot carry the tag, so it is a struct-like variant with a named `amount` field; the first
`circle_centre` tests built a two-half-turn shape from `tan(π/4)` where a quarter turn needs `tan(π/8)`, so
they were measuring themselves; the `toolpath` parser advanced two tokens for a positional argument, so the
document path swallowed its first option; and the offset kernel's own refusals do not end the ringing — a
shrink wider than a feature comes back as a well-formed smaller ring with unchanged winding after which the
rings grow, so a ring now proves every vertex sits the requested distance inside the original boundary
(D-M4-9), without which a 3 mm square pocketed by a 4 mm tool emitted a wall-gouging ring and looped
forever. Three more were fixed after both writers reported them: `OpenContour` said "profile toolpaths
need closed geometry" while pocket and drilling return it too, so the message now names no single
operation; `cam-cli toolpath --kind drilling` accepted a rectangle and left the refusal to `emit`, so the
append now resolves every entity through `circle_centre` and exits `2` before writing; and the golden
program pinned a bare `G00` the fixture post emits between toolpaths as `NEW_SEGMENT`, which is
pre-existing post behaviour now covered by a test rather than left to chance. This narrows §4.2's M0 wording: the kernel refuses an offset that crosses the shape's interior, but
not every over-shrink, and the guard lives in the strategy while the kernel limit stays recorded for
arrangement-based offsetting.

Recorded deviations, each a decision in the M4 feature document: `stepover` is a toolpath parameter and the
library's `stepover`/`clear_stepover` are the CLI's defaults (D-M4-3), because the same tool pockets MDF and
hardwood differently and the product stores the rate per tool; `--clear` reads `clear_stepover` for one
clearing pocket rather than introducing a two-tool toolpath (D-M4-4), with the observable semantics of the
product's own "Clear stepover" field still unresolved in the analysis report; a pocket was one region per
selected contour, with no islands until the kernel gained boolean subtraction (D-M4-2, retired since — see
§4.7); drilling accepts
circles only (D-M4-5); linking stays conservative, retracting to safe Z between rings, rows, spans and holes
(D-M4-6, D-M4-10); raster rows always run along X (D-M4-7); and M4 ships no toolpath UI, `cam-cli toolpath`
being the authoring surface (D-M4-8). The drill bank is geometry and selection only: no `G` code, `M` code or
toolchange is emitted, because the plan defers drill-bank hardware integration and the move vocabulary has
no bank command, and `rapid_length` (hole spacing) and `bank_travel` (head travel) are reported as the two
different numbers they are.

Six spec wordings were corrected by implementing rather than by reading, and all six are in the feature
document's log. Four came from the pocket slice: the ring stop rules were insufficient, because a shrink
wider than a feature returns a valid smaller ring and the rings then grow (the distance proof of D-M4-9 is
the fix); the raster example said "32 rows at y = 3, 6, … 97", arithmetic that cannot hold on the declared
grid, where 32 rows end at 96; the retry nudge of `1e-6` equalled the kernel tolerance, so a retried row
could land back on the ambiguity band, and it is now `1e-5`; and "a 4 mm tool in a 5 mm pocket is refused"
is false — a 2 mm inset leaves a 1 mm region the tool fits, so the refusal test uses a 3 mm pocket. Two came
from the drill-bank slice: the bank bound could not be `(count-1)*pitch/2` from a hole when the emitted
position is `x − k*pitch` with `k >= 0`, so what is bounded is the head's incremental step, and the T3 test
that asserted a bank is ignored was deleted rather than adapted, because it pinned exactly the behaviour the
bank task exists to reverse. One scope correction was recorded instead of papered over: `drill()` receives
resolved holes rather than contours, so the non-circle refusal is pinned where the resolution happens, in
`tests/drill_targets.rs` and the CLI layer, not in `tests/drill.rs`.

### 4.7 Region booleans and pocket islands delivered (evidence)

M4's decision D-M4-2 left pockets island-free "until the kernel has boolean subtraction". That kernel
work exists now: the `boolean ops` item of §3's `cam-geom` description is delivered as region
**subtraction**, expressed as even-odd parity plus the arrangement and the distance proof that keep
parity honest. A union is not part of it and is not claimed. The feature document is
`odd/tasks/region-booleans.md`; the contract and the user-facing behaviour are in
`docs/region-booleans.md`. The workspace stands at **707 tests**, green under `./scripts/check.sh`.

| Deliverable | Where | Proof |
| --- | --- | --- |
| Exact winding number, three-way membership (`Outside`, `Inside`, `OnBoundary`) and the contour relation (`Disjoint`, `FirstInside`, `SecondInside`, `Crossing`, `Touching`), arc-native and never decided from bounds alone | `crates/cam-geom::winding` | 9 + 11 tests; a single-point intersection counts as a crossing only away from both segments' ends, so a shared corner is `Touching`; a property test compares `classify` against a fine tessellation over three shapes, another asserts the relation is antisymmetric |
| `Region` as a flat boundary list under even-odd parity, with normalization (split at crossings, classify by parity, orient material-on-the-left, walk the arrangement) and subtraction as append-and-normalize | `crates/cam-geom::region`, `crates/cam-geom::boolean` | 15 tests; two overlapping squares normalize to two 75 mm² L-shapes — their symmetrical difference, not their 175 mm² union (D-RB-8); touching squares become one 200 mm² contour of 60 mm perimeter, so the shared edge is not doubled; a square plus an edge-crossing r4 circle gives 74.867 + 25.133 mm² with every surviving arc still radius 4 about its original centre; the walk is vertex-for-vertex deterministic; a property test checks that a normalized region's `contains` agrees with the original's |
| `Region::offset` — signs from each boundary's nesting depth, so a wall shrinks and an island grows — with the distance proof moved out of the pocket and into the kernel, and `Region::raster_spans` for even-odd row clipping | `crates/cam-geom::region_offset` | 17 tests; a 100 mm square inset by 10 gives `(10,10)–(90,90)` with area 6400 and every vertex 10.0 mm from its wall; an r10 island at an 8 mm distance gives an outer ring 84 mm wide and an r18 island ring (1017.876 mm²); an odd-depth hole grows (314.159 → 706.858 mm²); square/hole/boss gives 90, 70 and 10 mm widths; an over-shrink is refused where a naive `offset_contour` returns a well-formed gouging ring; a row crossing the island gives spans at `50 ± √91`, a tangent row gives `None` |
| Five whole-program goldens through `cam-cli emit` and the real post: offset island, its save/load round trip, raster island, the `@1` round trip, and the nested boss | `crates/cam-cli` | 45, 45, 45, identical-bytes and 43 lines; the programs were derived from the geometry and read line by line before being pinned — wall squares 3..97 / 11..89 / 19..81, island rings `r13`/`r21`/`r29` as `G03` half turns, raster gaps at `30 ± √(13² − (y−30)²)`, and the boss's rings shrunk to `r9`/`r2` because a nested island is material again and is cut |
| The exact minimum distance between two segments — line/line, line/arc and arc/arc in closed form — and a distance proof that is one comparison per pair with **no resolution knob** | `crates/cam-geom::distance` | 13 tests; hand-computed closed forms for every pair type, including a nearest point that is an endpoint and one that is interior; K5's chord (ends 23 mm, middle 17 mm) is dropped; the 4 mm fixture's analytic 14 stages are visible without a tool; property tests cross-check the answer against a sampled lower bound and against `intersect` |
| `Toolpath.island_ids`, its validation, and `c2d/document@2` | `crates/cam-model`, `crates/cam-store` | 11 model tests plus 1 store test; nesting is legal — a boss strictly inside a hole validates, and a refactor that refuses it again fails `an_island_nested_inside_another_is_a_boss_and_is_accepted` — while crossing and touching are refused; the field round-trips and is omitted when empty; an island outside the boundary, one in a C-shaped pocket's notch, two overlapping islands, a repeated island, an open island and islands on a profile toolpath are each refused by name; a `@1` document loads, keeps its toolpaths byte-identical and saves as `@2`; `@3` is refused naming both tags |
| `--islands` on `cam-cli toolpath --kind pocket`, and the island count `inspect` prints | `crates/cam-cli` | 3 binary tests: a pocket records its islands and reloads with them; an island outside the boundary exits `2` with the file byte-identical; `--islands` on a profile exits `2` naming the flag and the operation it belongs to. `inspect` appends `, N island(s)` and the island-free line is M4's |
| Offset pocket over a region with islands: rings from `Region::offset(inset + k·stepover)`, the strategy's own distance proof deleted | `crates/cam-toolpath::pocket` | 8 tests (7 in `tests/pocket_islands.rs`, one through the CLI asserting what the island adds as arc blocks — 0 without it, 14 with it — rather than as a line count); the 4 mm fixture cuts the analytic **14** rings and the closest approach over the emitted moves is 3.0000 mm; an 8 mm stepover gives 6 rings over three stages; a region that leaves nothing and a 3 mm pocket under a 4 mm tool are both `OffsetCollapsed` carrying the kernel's reason; a narrow neck reports `flattened`; two runs are identical. **All 12 M4 pocket tests passed with their expectations untouched** |
| Raster pocket over a region with islands: rows clipped by `Region::raster_spans`, so a row crossing an island is two spans | `crates/cam-toolpath::pocket` | 5 tests; 100 mm square plus centred r10 island at a 3 mm stepover gives **32 rows and 40 spans** — 8 rows cut two spans ending at `50 ± √(13² − (y−50)²)`, 24 cut one span `3…97`, and the row at `y = 63` is exactly tangent, so the kernel answers `None` and the retry resolves it just outside; a full-width island band gives 24 rows and 24 spans; a narrow strip at 7 mm gives 14 rows and 24 spans with no unpaired span; every cutting move stays ≥ `3 − 1e-6` mm from the island. **`tests/pocket.rs` needed no edit** (12/12) and a differential over eight island-free raster fixtures came back as 875 identical output lines |

Defects found and fixed at the root, in the order the tasks surfaced them: the arc's contribution to the
winding number was computed at its tangency points and its branch chosen from the *sign* of an `atan2`
result, so a bulge dipping below its own chord read as outside — it is now cut where it crosses the
horizontal line through the query point and the branch is decided by distance (`crates/cam-geom::winding`);
A2's rule that any single-point intersection is a crossing made two squares sharing a corner overlap, so a
meeting point now only counts when it is interior to both segments (L5); A4 named `cam_geom::chain` for
chaining the arrangement, and `chain`'s greedy endpoint match rejoins a piece to its own continuation and
hands the input back with its crossings, so the walk turns at every crossing and `chain` is untouched (L7);
the island containment check asked `relation(boundary, island)` and compared it to `FirstInside`, so every
valid island was refused — `relation` is directional and the island is the first argument (L6); the offset
kernel's vertex-only distance proof was unsound for arcs, and the 4 mm fixture carried two lens-shaped
pieces whose only vertices cleared the distance while their arcs bulged 6 mm into the wall (L9); and the
straight-piece form of the same defect — a chord 17 mm from a boundary whose ends are 23 mm away — is what
made a closed-form segment distance the answer rather than a finer subdivision (L10, D-RB-11).

Recorded deviations and limits, each attributed: normalization is **parity-preserving, not a union**
(D-RB-8), so two overlapping boundaries describe their symmetrical difference and a union remains a
separate operation; a boundary that crosses **itself** is passed through unresolved, because A4 only asks
about crossings between boundaries (L7); an exact tangency in the arrangement is **refused by name** rather
than resolved, as at exactly 20 mm in the 100 mm square plus r10 island fixture, where the grown island is
tangent to all four wall insets; `intersect` at tolerance `0.0` can miss a real crossing, which is why the
distance proof asks for the unsnapped minimum and the cross-check runs at the kernel's nanometre; and a
piece that fails the proof is still dropped **whole** (D-RB-9), which costs the material it covered but can
never gouge. The two earlier forms of the proof — vertices only (D-RB-9) and arc stretches against a
sagitta budget (D-RB-10) — are gone rather than softened, and D-RB-11 records why the exact pair distance
is the form that has nothing left to tune.

Four spec wordings were corrected by implementing, all in the feature document's log. A2's crossing rule
was wrong about shared corners and edges (L5); A4's chaining primitive was `chain`, which cannot resolve a
four-way crossing (L7); A4's area expectations described a union where parity gives the symmetrical
difference, so the tests pin 75 mm² L-shapes and 100 mm² for a square plus an edge-crossing circle (L7);
and D-RB-10 as first written budgeted the subdivision by the piece's sagitta, which does not justify a
Lipschitz step hundreds of times larger, so the budget became the stretch's arc length and then, with
D-RB-11, nothing at all (L10). Two corrections came from the strategies rather than the kernel: a raster
row is emitted at the height the kernel actually clipped at, not the nominal grid height, because emitting
the nominal one moves a tangent row's ends off the curve by about 1e-5 mm (L11); and a CLI assertion that
counted program lines was replaced, since an island ring is two arcs where a wall is four straight moves
— what the island adds is arc blocks (L10).

One task of this feature closed its own defect. **P5**, the golden G-code for a pocket with an island in
both modes, the same program after a save/load round trip, and an island nested in an island (a boss)
cut as material again (A14), landed in `b7910f3`: five whole programs, and the boss golden found P1's
rule refusing every nested pair, so the fix and its proof arrived together. The island-free half of A15
is covered by the two differentials above — M4's pocket tests pass unchanged and the island-free
fixtures emit identical programs.

### 4.8 M5 delivered (evidence)

M5 is the acceptance test the earlier milestones were written against: a green suite says a toolpath
satisfies its rules, not that the part comes out right. The new `cam-sim` crate takes the `MoveStream`
`cam-cli emit` would hand the post-processor (D-M5-1), sweeps it over a heightfield of the stock, derives
the depth each cell was **authorised** to lose from the document alone, and reports gouge and uncut
material as one subtraction with two signs (D-M5-2). `cam-cli simulate` is that report, and the shell
shows the same field as a depth-shaded 3D surface it can play. The feature document is
`odd/tasks/m5-simulation.md`; the model, the report's exact shape, the exit codes and every
recorded limit are in [docs/m5-simulation.md](../docs/m5-simulation.md). The workspace stands at **795
tests**, green under `./scripts/check.sh` (`fmt --check`, `clippy -D warnings`, the whole suite).

| Deliverable | Where | Proof |
| --- | --- | --- |
| A grid that covers the job in whole cells with a hard cap, and the stock as one surface height per cell | `crates/cam-sim::grid`, `crates/cam-sim::heightfield` | 11 tests; the default 1219 x 2438 mm sheet is 2,971,922 cells, under the 4,000,000 cap, and a 0.1 mm request names `12190 x 24380`, the product and the cap in one message; a 10.2 mm wide job gets 11 columns whose last centre lies outside the nominal width, which is the coverage bound stated rather than hidden; a zero-sized job and a zero cell are `Degenerate`, not a panic; the top is `0` and the bottom `-thickness`, a cell never rises, a cell clamped at the bottom counts as through, a whole field taken there reports 500 mm³ removed, 0 remaining and 100 through cells, and a property test asserts `removed + remaining` equals the grid volume over random cut sequences |
| Swept removal over a program: feeds and arcs cut, rapids never do, and a rapid below the surface is a crash | `crates/cam-sim::sweep` | 6 tests; a straight feed's cut cells agree cell for cell with a brute-forced exact point-to-segment capsule; a full-circle arc leaves its own centre cell standing, so the arc is swept as an arc; a rapid at clearance cuts nothing and is no crash; a rapid to `-3` is exactly one crash carrying `pocket 1` and move index 1, with the field untouched; two runs are field-identical. `cam_move::Segment` gained `tool_diameter`, which the emitter ignores |
| The authorised target surface and the two deviations | `crates/cam-sim::target` | 15 tests, then 20 after D-M5-5's amendment; a pocket region with a centred r10 island authorises the square minus the disc; `Outside`, `Inside` and `On` profiles authorise the band each tool actually sweeps; a tab keeps its height back; a drilling toolpath authorises each hole and nothing else; two pockets authorise the deeper, never the sum; a cell nobody claims is zero; a field cut exactly to the target deviates zero, and one difference read with two signs is a gouge and then an uncut of the same volume |
| `cam-cli simulate`: the report, the exit code and the tolerance | `crates/cam-cli` | 9 binary tests, `cam-cli` at 53 (55 after S5 and S6); whole six-line reports pinned byte for byte for an offset pocket, a raster pocket, a drilling toolpath and a too-shallow pocket; at a finer resolution the coverage error falls from 505.0 to 126.5 mm³ on a 120.4 x 90.4 mm pocket; a too-shallow pocket exits `6` with uncut and a drill wider than its hole exits `6` with gouge, each stating its own cubic millimetres; a crash exits `6` naming the toolpath and move index on the first crash line; two runs print identical bytes; `--resolution 0.01` exits `2` naming the cell, the grid and the cap; `--help` lists the command, its flags and its exit code; the document is byte-identical afterwards |
| The oracle for every strategy M4 ships, in both signs | `crates/cam-sim/tests/oracle.rs`, `crates/cam-cli` | 6 kernel tests (`cam-sim` at 43) plus 2 added and 1 strengthened through the binary; an offset pocket removes 59976 of 60000 mm³ analytic, raster agrees with offset within 600 mm³, three r3 holes remove 576 mm³ against 508.938, and a circular `Outside` profile is exact; refinement at 2, 1 and 0.5 mm never grows the error (115.062, 67.062, 4.938 mm³); one region cut 1 mm shallow reports 96 mm³ uncut and 1 mm deep 96 mm³ gouge, so the test cannot pass by reporting the wrong sign |
| A 3D camera and the stock surface as a mesh, both pure data | `crates/cam-view::camera3d`, `crates/cam-view::mesh` | 13 tests (`cam-view` at 36); the target projects to the viewport's centre at four pitches and three yaws; yawing equals rotating the point about the vertical; behind the eye is `None`; the pitch clamp holds at both ends; zoom clamps at 1 mm and 100 m and ignores a zero, a negative, a NaN and an infinity; a 10 x 8 field gives 80 triangles and 126 vertices at stride 1, 12 and 12 at stride 4, 4 and 2 at stride 100; the depth ramp is exactly 0.0 at the top and 1.0 at the bottom; the stock box is 12 axis-aligned edges; two builds are identical vertex for vertex, with the triangles ordered back to front |
| The 3D panel, its gestures and the painter's mesh | `crates/cam-ui::view3d` | 9 tests (`cam-ui` at 61); an open panel paints triangles and exactly 16 3D lines (12 box edges + the program's 4); a drag orbits and the wheel zooms about the target; a camera the projection rejects paints nothing instead of panicking; a closed panel paints no 3D at all; the surface reaches the painter as one `egui::Mesh` of three vertices per painted triangle (A11); the 2D canvas still paints its 4 items and its grid, and a closed panel leaves the counts equal to a shell that holds no preview at all — the 52 pre-existing shell tests untouched |
| Playback that is a pure function of an injected delta | `crates/cam-ui::view3d` | 10 tests (`cam-ui` at 71, `cam-desktop` at 2); six deltas at speed 1 walk the cursor 0→1→2→2→2→4 with fractions 0.5/0.25/0.2/0.6/0.9/1.0; the field's volumes equal the closed form of the same prefix; a partly consumed move cuts nothing; a zero delta and a paused frame change nothing; `step` advances exactly one move; `reset` restores the stock cell for cell; the final field equals `sweep::apply` over the whole stream both from one 10 s call and from 40 x 0.03 s frames; playback stops at the end and never loops; and `DrawnState::preview_progress` is 0.0 paused, strictly between 0 and 1 after one playing frame, and exactly 1.0 at the end |
| No new dependency, and the 2D editor untouched | workspace | the one new `[[package]]` in `Cargo.lock` is the workspace's own `cam-sim`: the 3D view is `egui::Mesh` on the existing painter (D-M5-7), and the editor's suites keep passing unchanged (A12) |

The milestone found one real defect outside its own crates, and it is the kind S6 exists to find. **S6:**
`Region::raster_spans` answers `None` for a row lying along a boundary, and `row_spans` retried such a row
at `y + nudge` only — so at a region's near edge the nudge landed inside the material and the row was
cut, while at the far edge it landed outside and the row was dropped. The row the far edge sits on is the
row that clears the strip that edge ends. Measured before the fix on a 100 mm square with a 2 mm tool at a
2 mm stepover: **49** rows instead of 50, 9800 of 10000 cell centres covered, 58800 mm³ removed against
the analytic 60000, and 200 cells / 1200 mm³ reported uncut — the same numbers the M5 oracle had reported
as a simulation failure. The fix is in `row_spans`: the nominal height, then `y + nudge`, then `y - nudge`,
and the first height that clips any material wins. Two nudge directions rather than a tolerance knob, no
"last row" special case, and a fixed candidate order, so the emitted program stays a function of the
region and the row height alone (S9). Two expectations moved and were re-derived rather than fitted: the
raster `simulate` fixture now runs its own 2 mm stepover instead of the 1.58 mm one that had been chosen
so the row lattice missed the far edge, and the island raster golden gained the `y = 57` row (45 to 49
lines, 9 spans to 10). Every non-raster golden is byte-identical, which is what says the defect was where
the oracle measured it and nowhere else.

Recorded deviations, each a decision in the feature document: **D-M5-5 was amended by evidence** — the
profile target is the material the tool *sweeps* (`allowance … 2·radius + allowance`), not the offset of
its centre, because the first form reported the outer half of every clean kerf as gouge (1632.0 mm³ over
272 cells on a perfect 100 mm `Outside` cut), and the `On` side ignores an allowance because the emitter
does; **D-M5-6** gives a found defect its own exit code, `6`, on the precedent `check` set with `5`;
**D-M5-7** paints the surface with `egui::Mesh` while D-M1-2's `wgpu` trigger stays intact, since a wgpu
backend would consume the same mesh; **D-M5-8** ships no `rayon` and no tiling, with the one-second
measurement as its trigger; **D-M5-9** keeps the preview off the document — it moves a cursor over a
program a caller hands in, adds no `cam-app` command and pulls no `UiAction`; **D-M5-10** does not
authorise the wall an allowance keeps, so a correct allowance pocket is not reported as uncut;
**D-M5-11** records the mitered-corner over-authorisation — at most `4(r² − πr²/4)` per right-angled
corner, about `7.73 mm² × depth` at a 6 mm tool, measured as 24 mm³ over 4 cells at a 1 mm cell — with
the circular contour as the case where the statement is exact. Two smaller corrections are recorded rather
than hidden: `set_preview` takes the untouched stock, because `reset` needs a pristine field and nothing
can un-cut one (L8), and a rapid and a retract are timed at the segment's feed rate because `Segment`
carries no rapid rate. Linking stays conservative (D-M4-6, unchanged): M5 makes a gouge-free linking
provable, it does not re-plan a program.

## 5. Verification strategy

Per ODD, change behaviour with applicable runnable checks, test-first:

1. **Golden-file G-code tests** — the primary regression harness. `cam-cli` on a fixture document
   must emit byte-identical output; every strategy and every output block gets a golden file.
2. **Property tests** (`proptest`) on the geometry kernel — invariants that catch the classic 2.5D
   bugs: an outward offset of a closed loop never self-intersects below the tool radius; chaining
   never produces duplicate endpoints within tolerance; reversing a polyline preserves winding sign.
3. **Round-trip tests** — document → JSON → document is lossless; SQLite tool library migrates
   forward and never loses a tool.
4. **Post-processor conformance** — a fixture set covering all 13 directives and 15 blocks, plus a
   parser that rejects malformed files with a located diagnostic (`line:column`, expected token).
5. **Simulation as oracle** — for each strategy, a test asserts the simulated removed volume matches
   the analytic expectation within tolerance; a too-shallow pocket fails the test, not a screenshot.
6. **No-manual-QA rule** — nothing is "done" because it looks right in the window; the CLI proof
   comes first.

Explicitly not used: comparisons against the vendor's own G-code output as an oracle. Byte-matching a
closed-source implementation's ordering and rounding choices would turn every improvement into a
regression, and we did not decompile their algorithms.

## 6. Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Scope D2 is broad (11 strategies incl. V-Carve, Inlay, Moulding) | Plan inflation, half-finished strategies | Milestones ordered so each is shippable; V-Carve and Moulding isolated at M7/M8; UI never blocks a strategy |
| Free-profile nesting (NFP) is NP-hard and quality-sensitive | M9 slips | Ship rectangle nesting first (covers cabinetry parts), free profiles second |
| Offsetting with arcs is where CAM kernels break | Wrong geometry silently | Arc-preserving offset library + property tests with randomized polygons; tolerance surfaced as a job setting |
| egui limits dense CAD UI | Rework of UI shell | Shell behind an adapter; `cxx-qt` swap documented, domain untouched |
| PDF/EPS vector import fidelity | Users blame the app for their file | Import report that lists dropped/approximated entities instead of failing silently |
| Simulation performance on large jobs | UI stalls | **M5:** the documented cell cap (4,000,000 cells, 32 MB of surface heights), a decimation stride that keeps the mesh's lattice within 200 sampled lines, and a single-threaded, untiled removal loop (D-M5-8). Parallel or tiled removal lands when a measured simulation of a default sheet at the default resolution exceeds one second |
| No machine-gate for `.pp` compatibility | Users bring malformed post-processors | Strict parser with located diagnostics and a dry-run "validate post-processor" command |

## 7. Decision log

| ID | Decision | Date | Basis |
| --- | --- | --- | --- |
| D1 | Rust + wgpu, `egui` shell behind an adapter | plan time | User choice; analysis report §1/§3 (only shell + viewport are platform-bound) |
| D2 | All strategies except 3D and Laser | plan time | User choice; superset recovered in analysis report §2.3 |
| D3 | Own ZIP+JSON document format, interop via import | plan time | User choice; `.crv` OLE2 internals are an unknown (analysis report §2.5) |
| D4 | Own heuristic nesting in v1 | plan time | User choice; original uses a licensed third-party engine (analysis report §2.4) |
| D5 | Millimetres internal, rounding only at emission | architecture | Post-processor decimal specs differ per variable in 75 shipped files |
| D6 | Arcs first-class, no pre-tessellation | architecture | 8 arc output blocks in the recovered DSL grammar |
| D7 | Do not vendor the vendor `.pp` files | legal/architecture | Third-party content; grammar reimplemented instead |
| D8 | `cam-cli` as the test surface | verification | Makes every milestone verifiable without the UI |

## 8. Log

- **L1 (user, verbatim):** "quiero crear una aplicacion similar a the reference CAM application, para ello necesito
  que la analices con el mcp de REA @etc/the reference CAM applicationProTrialEdition_Setup.exe, extraigas las evidencias y
  generemos un plan de desarrollo mediante ODD. Como extra, decirte que quiero que sea cross platform
  Windows/Linux."
- **L2 (user decisions, verbatim option labels):** "Rust + wgpu/egui (o Rust + Qt bindings)";
  "Todo menos 3D y Laser"; "Formato propio abierto (JSON/ZIP) + import DXF/SVG/PDF/EPS";
  "Implementar nesting heurístico propio en v1".
- **L3 (analysis):** evidence dossier written to `evidence/analysis-report.md`; installer extracted to
  `evidence/installer-extract/`; REA evidence IDs recorded in dossier §5.
- **L4 (plan):** this document. Next executable artifact is
  `odd/tasks/m0-foundation.md`.
