# Development Plan — the reference CAM application-class 2.5D CAM application (Windows + Linux)

Status: **executed through M13.** The plan was derived from `evidence/analysis-report.md` (REA evidence on
`etc/the reference CAM applicationProTrialEdition_Setup.exe`, the reference CAM application) and M0–M12 are delivered; §4 carries
the evidence for each, section by section through §4.15, and §6 the risk register with the one row M7 closed. **After M10 the work did not
continue as milestones**: the findings those milestones left open became three recorded features with their
own documents — the advisory debt (`odd/tasks/debt-advisories.md`, which carries its own counts and the reason
the rest cannot be recovered),
the script panel's mount (`odd/tasks/script-panel-mount.md`) and the panel's document
(`odd/tasks/script-panel-document.md`) — and the roadmap's Log in §8 names all three.

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
  cam-export/    The reader's mirror: DXF and SVG writers. The document goes out as bytes in millimetres,
                 nothing is written to disk here and no bulge is flattened
  cam-move/      The machining move vocabulary (`Move`, `MoveStream`): the contract between the toolpath
                 engine and the emitter, so neither side depends on the other
  cam-program/   The join between a document's toolpaths and a tool library — `program_for`,
                 `program_for_snapshot`, `program_tool`, and the material selector M11 added
  cam-app/       Application state machine: commands, undo/redo, selection, job lifecycle. No UI deps
  cam-ui/        egui shell: docks, property grid, tool database editor, toolpath forms
  cam-view/      wgpu renderer: 2D scene (arcs, curves, toolpaths, grid, snap markers) + 3D view
  cam-desktop/   The native window binary: opens the editor with a sample document, imports a `.c2d`
                 the user picks, plays the 3D preview. Verified by building, not by automation
  cam-cli/       Headless pipeline: `document -> toolpaths -> gcode|svg|sim-report`. The verification
                 surface; also the CI regression harness
```

**There is no `cam-nest` crate.** This layout named one when M9 was still unplanned; the heuristic engine
M9's row decides (D4) landed in `cam-geom::nest` and `cam-geom::nest_profile`, because a placement is
geometry that `cam-model`'s `nesting` section carries, not a crate of its own. The four crates listed above
that the original layout did not name — `cam-move` (M6), `cam-program`, `cam-export` (M12) and
`cam-desktop` (M1's native window) — are the layout catching up with the delivered workspace.

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
| **M6** ✅ | Depth-variant strategies | **Delivered.** Quick Engrave, Chamfer, Fluting, Texture, Thread Milling and the Inlay pair (female cavity + male plug from one command), each authored through `cam-cli toolpath` and each with a simulation oracle in both signs; the V-bit's cone now reaches the move vocabulary and the simulation, and the post gained a helical move so a thread is emitted as a helical arc; `c2d/document@2` unchanged and every pre-existing command byte-identical (see §4.9) | The six strategies that make it a carving machine rather than a router; they cost two prerequisites the row did not name — tool geometry and a helical move — and are cheap only after them |
| **M7** ✅ | V-Carve | **Delivered.** `cam-cli toolpath --kind vcarve` walks the region's verified medial axis with the tip at `−d/tan(θ/2)` for each tip's own exact distance `d` to the boundary, so the toolpath is a depth **field**: the emitted program is varying-Z linear feeds and needs no post change, `simulate` authorises the distance field from the document alone, and the oracle asserts both signs with `gouge` **exactly zero**; V-carve is proved by two golden programs (a square and a disc, each three ways), the exact certificate every axis vertex carries, and a 35-invocation differential against `9ff0b8c` (see §4.10) | Research-grade geometry (medial axis of arcs); deliberately isolated so it cannot delay the rest |
| **M8** ✅ | Moulding / swept profile | **Delivered.** `cam-cli toolpath --kind moulding` sweeps an authored cross-section along a rail at a stepover: the rail is the selected entity, one repeatable `--profile <lateral>:<depth>` per cross-section point and `--stepover` (or the library's), and the program is one lateral pass per stepover offset — the rail walked by arc length at that pass's own Z, one linear feed per rail sample. `simulate` authorises the cross-section swept along the rail from the document alone, and the oracle asserts both signs on a straight rail, a closed frame and a V-bit cone, with `gouge` **exactly zero** for a correct program; moulding is proved by two golden programs (a straight rail and a closed frame, each three ways), the stepover law's coverage certificate, the multi-rail case end to end, and one measured limit recorded rather than hidden (see §4.11) | The row expected M7's distance field to be the cost; it was not — the cross-section is authored data |
| **M9** ✅ | Nesting | **Delivered.** `cam-cli nest` packs a document's authored sheets and parts — rectangles through maxrects and a guillotine pass, free profiles through a sound-but-incomplete no-fit heuristic — and stores the placements plus **one** cost record derived by `Document::nesting_utilisation`; a run that leaves an instance unplaced reports it and exits `7` writing nothing. Proved by 78 behaviour tests (31 in `cam-model`, 14 + 17 in `cam-geom`, 16 in `cam-cli`), a byte-for-byte report and document golden set, the fixture where guillotine beats maxrects, the counted cost of the profile heuristic's incompleteness, a 42-invocation differential against the milestone's cut point, and two reviewed candidates with their corrections (see §4.12) | Own heuristic engine as decided (D4); depends on stable geometry and toolpaths |
| **M10** ✅ | Documents & scripting | **Delivered.** `cam-cli script` runs a line-oriented command language against one document — eleven words, each dispatching to the function its own subcommand calls, with `line:column` diagnostics and every mutation through `Document::validate`; `cam-cli parts` imports a four-column part list and `cam-cli cutlist` exports the placements M9 stored as seven columns, deliberately two shapes and not one schema; `cam-cli report` writes a self-contained, hand-escaped HTML print sheet; and `cam-ui`'s `ScriptPanel` runs a script through the same library, never a process. No schema bump, no external dependency, and `--help` is the only difference from the pre-M10 surface (see §4.13) | Reporting and extension points; the original's gadget platform is the model — with S1's "DSL de comandos, sin motor" deciding the engine the row left open ("Rhai/Lua") |
| **M11** ✅ | Feeds & speeds | **Delivered.** The library already keys cutting data per material and per machine (`material`, `tool_entity`, `cutting_data_for(geometry, material, machine)`, proved by one tool at 120 and 60 mm/s in two materials); what is missing is the **selector**, so `emit` cuts at the tool's own record and `tools add` writes the same `100 / 30 / 18000` for every material. M11 carries `Job.material` (optional, `#[serde(default, skip_serializing_if)]`, tag stays `c2d/document@2`) plus an `emit --material` override that wins, threads it into the lookup the store already has, and leaves **`None` reproducing today's bytes exactly** — all 17 goldens keep `F100.0`. An unknown material is refused by name, never silently ignored. Deriving a rate from chipload and surface speed is **not** in this row: it would change the one number the goldens pin and put a derivation between the library and the program that nothing tests and the selector is threaded to the store's existing per-material lookup. **Evidence**: 21 golden tests untouched, 1548 workspace tests green, an unknown material refused by name, and a review that caught the flag accepting a missing name and a no-selector change nobody asked for, both corrected in 79 diff lines and validated. Two follow-ups recorded: the shell's estimate cannot honour a material until `cam-store` answers one, and `simulate` has no `--material` to reproduce a one-run override | Every strategy emits at the tool's fixed rate today, which is the most user-visible gap left: the machine is told how fast to cut by a record nobody selected (see §4.14) |
| **M12** ✅ | Export | **Delivered.** The workspace has no writer of any interchange format — `cam-import` says so in its own header, and `cam-ui`'s icon catalogue has declared `export-dxf` and `export-svg` since M1 with no command behind them. M12 adds a `cam-export` crate mirroring the reader's shape: **DXF** keeps layer identity (one `LWPOLYLINE` per contour, each vertex's bulge verbatim, `70 = 1` when closed, `$INSUNITS = 4`, the layer table in document order) and **SVG** writes one `<path>` per contour with `A` commands whose `rx == ry`, the Y reflection the reader inverts, and a `viewBox` in the document's own millimetres. The acceptance test is the **round trip against our own reader**: exported and imported again, the same shape and the same layer order, compared up to the 10 mm placement the reader performs — never byte equality. SVG's documented loss is that it carries no layers. Toolpaths are **not** written: a toolpath is not geometry, and writing one would make "the same thing that went in" unprovable | A drawing tool that can read five formats and write none is a one-way door; the reader's own conventions are what make the writer provable (see §4.15) |
| **M13** ✅ | The cycle closes inside the app | **Delivered**, with one task routed to the owner rather than closed. Two dead ends hid behind finished chrome: the app bar's primary action was `add_enabled(false, …)` with a hover excuse (`cam-ui/src/chrome.rs:323`) and the 3D half printed *"Simulate one with `cam-cli simulate` and hand it in"* (`view3d.rs:1390`), so a window that could import, author and display a program sent the person to a terminal for the one artefact the machine runs. M13 makes the program the app's own: `GenerateGcode` posts the open document through `cam-post` inside `apply_action`, the same place `SaveFile` writes (`S7`, corrected after measurement — it had said the host would do it), `Post…` holds the dialect for the session and refuses a file that is not a post-processor while keeping the one in hand, the 3D half builds the drawing's own program with `program_for_snapshot` over a pristine `Heightfield` behind a `Simulate` chip and a `Simulate again` in the overlay row, and the toolpaths are a list whose selection — not `toolpaths.first()` — is what the card, the editor and `open_tool_library` read, with the last of those a door the first version left aiming at the first toolpath (see §4.16). **T4 dissolved on measurement**: the 3D stock already paints through `StockTop`/`StockBottom` via `stock_shade`, and `design/system.md:53`'s own value is a cool `#d6dce6`; the tan is the reference palette, so changing ours re-opens four contrast proofs and belongs to the owner | The most visible gap left after twelve milestones was not missing machinery but missing *seams*: every part worked, and the person still could not finish a job without a terminal (see §4.16) |

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
| Raster pocket: rows on the `min.y + k·stepover` grid, finished with one row on `max.y` when the grid stops short of it, clipped by true line/arc intersections | `crates/cam-toolpath::pocket` | 33 rows on the same square at 3 mm stepover — 3, 6, … 96 on the grid plus the finishing row at 97 — and 50 rows unchanged where the far edge already sits on the grid; analytic chord widths on a circle within 1e-6, and the tangent finishing row of the radius-17 region cuts a chord of about 0.037 mm; a tangent row emits no unpaired span; no arc moves in raster mode |
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
| Raster pocket over a region with islands: rows clipped by `Region::raster_spans`, so a row crossing an island is two spans | `crates/cam-toolpath::pocket` | 5 tests; 100 mm square plus centred r10 island at a 3 mm stepover gives **33 rows and 41 spans** — 8 rows cut two spans ending at `50 ± √(13² − (y−50)²)`, 25 cut one span `3…97` (the finishing row at `y = 97` is 47 mm from the island's centre, outside its 13 mm reach), and the row at `y = 63` is exactly tangent, so the kernel answers `None` and the retry resolves it just outside; a full-width island band gives 25 rows and 25 spans; a narrow strip at 7 mm gives 15 rows and 25 spans with no unpaired span; every cutting move stays ≥ `3 − 1e-6` mm from the island. **`tests/pocket.rs` needed no edit** (12/12) and a differential over eight island-free raster fixtures came back as 875 identical output lines |

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

### 4.9 M6 delivered (evidence)

M6 is the milestone whose plan was wrong in an instructive way. The roadmap row called the six strategies
"profile/pocket variants with a different Z law — cheap once M4 and M5 exist"; two facts found before the
first write said otherwise, and they are why the milestone has nine task rows rather than six (L3).
**(1) No tool geometry reached the toolpath layer.** `ToolSpec` carried a number, a diameter, a feed rate,
a plunge rate and a spindle speed, while `included_angle`, `flat_diameter` and `tip_radius` had sat unused
in the tool database since migration 1, and `cam-sim` swept a constant-radius cylinder per segment — so a
cone had no carrier at all, and a chamfer swept as a cylinder would have reported a *correct* bevel as a
gouge (D-M6-3). **(2) `Move::Arc` is constant-Z by contract** and the post fixture's arc blocks carry
`[X][Y][I][J]` and no `[Z]`, so thread milling could not ride on the existing move or block: M6 added
`Move::Helix`, four helical block kinds, and a refusal when a post defines none (D-M6-8). Neither was
absorbed silently, and both are recorded as the milestone's own cost.

The feature document is `odd/tasks/m6-depth-variants.md`; the strategies, their Z laws, their
refusals, the simulation's claim for each, the differential and the one defect M6 recorded without fixing
are in [docs/m6-toolpaths.md](../docs/m6-toolpaths.md). The workspace stands at **975 tests across 70 test
targets** — 180 more than the 795 the branch cut from — green under `./scripts/check.sh` (`fmt --check`,
`clippy -D warnings`, the whole suite). In the four crates M6 touches that is cam-model 47, cam-toolpath
123, cam-sim 99 and cam-cli 104; `cam-sim/tests/target.rs` holds 43 tests, 23 of them M6's, and
`cam-sim/tests/oracle.rs` holds 23, of which 17 are the six strategies' (the other six are M5's).

| Deliverable | Where | Proof |
| --- | --- | --- |
| Six `Operation` variants and their parameter records inside `c2d/document@2`: `Chamfer`, `Engrave`, `Fluting`, `Texture`, `Inlay`, `Thread`, with `ChamferParams::depth` and `TextureParams::grooves` the only derivations | `crates/cam-model` | 47 tests in `tests/document.rs`; every kind round-trips losslessly, a `fluting` document without `ramp` and a `texture` without `spacing`/`angle` load as the pattern they describe (the fields default), an unknown `kind` is still a hard load error, and each refusal names its own value — a zero chamfer width or an angle outside `(0, 180)` would otherwise emit an infinite Z, and a negative ramp, a non-positive spacing, a degenerate inlay stepover, a thread with no pitch and an inlay record naming both halves are refused at load. An `inlay` pair is two toolpaths sharing one operation, and its defaults *are* the pocket's, taken from `PocketParams::default` rather than restated |
| The tool's shape on the move vocabulary, and a sweep that grows with depth | `crates/cam-move::ToolShape`, `crates/cam-sim::sweep`, `crates/cam-sim::target` | `ToolShape::{Cylinder, VBit}` replaces `Segment.tool_diameter`, with `radius_at(depth)` and `rise_at_radius(radius)`; the replacement was behaviour-neutral before any cone test existed (**795 tests**, the number the branch cut from). A 90° V-bit's floor is graded at 3 and 5 mm from the line and reaches 6 mm, a 2 mm flat has a flat bottom and a 7 mm reach, and the D-M5-5 agreement test with a cone reports **gouge exactly zero at zero tolerance** — the sweep's sample can only be further from the path than the path is, so a cone cut is never reported deep |
| Chamfer: a bevel whose depth is derived once from the bit's angle, in one pass with the tip on the vector | `crates/cam-toolpath::chamfer`, `crates/cam-cli` | 7 strategy tests, 3 oracle tests and its binary cases; `6 mm` on a 60° bit is **10.392 mm** deep and `3 mm` on a 90° bit is **3.000**, derived in `cam-model` so program and claim cannot disagree; the oracle asserts **both flanks** (`L·w·d`, not the one flank `L·w·d/2` A9 first named — L9) around a 40 mm ring within `4πR·cell/2·d` and deviates by nothing; a separate oracle covers the one-sided case on the stock's edge; a flat tip is refused at **all three** boundaries that can see it — before authoring, after the document exists, and in the segment's own shape (L11) |
| Quick engrave: one constant-depth pass on the contour, open or closed | `crates/cam-toolpath::engrave` | 6 strategy tests, 3 oracle tests and its binary cases; the closed form is `L·d²·tan(θ/2)` — **3325.6 mm³** for a 6 mm pass over 160 mm with a 60° bit — and an end mill removes a uniform band; the claim carries the reach test a cone self-clamps and a cylinder does not (L12); `cut_depth` at or below `start_depth` is refused naming both |
| Fluting: one pass per vector with a real ramped entry and exit, and the ramp rides an arc as a **helix** | `crates/cam-toolpath::fluting`, `crates/cam-move::Move::Helix` | 10 strategy tests, 3 oracle tests and its binary cases; `2·R·d·(L − ramp) = 5400 mm³` analytic against **5548.5** measured, the 148.5 being mostly `π·R²·d = 169.6 mm³` of toes a real tool leaves where each ramp meets the surface — four samplings of one geometry inside the published `960 mm³` bound (the rejected staircase measured 5549.2); the claim is the **envelope** the law allows anywhere the footprint could have reached, because a tip-only claim reports a clean flute as `gouge 149.2 mm³` (L13); T7 lifted T4's `RampOnArc` refusal and the fixture that chorded at `ramp²/(8r)` now simulates with **gouge 0.000 / uncut 0.000** |
| Texture: parallel grooves at a spacing and an angle, derived once, never a clearing pass | `crates/cam-toolpath::texture`, `cam-model::TextureParams::grooves` | 8 strategy tests, 2 oracle tests and its binary cases; the pattern is clipped in the groove frame by the kernel's own `Region::raster_spans`, so arcs and islands come free and no second intersector exists; a 40 mm square at an 8 mm spacing is five rows and `2·R·d·ΣL = 7200 mm³`, measured **exactly** 7200; the 45° case asserts the emitted diagonals and the claim geometry, because a diagonal groove leaves a two-cell rind the 1 mm grid does not sample (`uncut 1.000 mm³ over 2 cell(s)`, M5's published bound — L15); the command says in its help, and in its refusal of `--stepover`, `--allowance`, `--side`, `--pass-depth`, `--tab`, `--lead-*` and `--ramp`, that the material between grooves stands |
| The inlay pair: one command, two toolpaths, fixed order, the allowance as the clearance | `crates/cam-toolpath::inlay`, `crates/cam-cli` | 9 strategy tests, 2 oracle tests and its binary cases; the female clears the region grown by `allowance` with the pocket's offset rings and the male clears `Job::outline` with the region standing as an island in raster rows; the halves are told apart by the ids they carry (D-M6-13), and a record naming both is refused rather than guessed at; the ring pattern was rejected on a **measurement** — 2300 mm² of an 8556 mm² frame never cut at a 2 mm stepover, against 80 mm² for the rows; what remains is the pocket's own coverage bound, pinned in the oracle as **`uncut 480.000 mm³ over 80 cells`**, with the female's four unreachable corner cells at `24.000 mm³ over 4` |
| Thread milling: a helical descent, a helical move, and a spiral claim | `crates/cam-toolpath::thread`, `crates/cam-move::Move::Helix`, `crates/cam-post` | 9 strategy tests, 4 oracle tests, its binary cases and the emitter's helical block test; the orbit is `(D∓t)/2`, the descent `cut_depth/pitch` revolutions as one full helical move each plus the fraction, and a 3 mm thread at a 1.5 mm pitch emits two `G03` blocks carrying `[Z]`; a post with no helical block **refuses, exit 4**, naming the four kinds instead of emitting a flat circle at the wrong depth. The first claim (the whole annulus at `cut_depth`) failed every real thread — `uncut 106.452 mm³ over 151 cells` — and the tip-only spiral reported a correct thread as `gouge 8.697 mm³`; the claim is now the spiral, taking the maximum over the footprint's angular window (D-M6-14). On the oracle's fixture: analytic `603.186 mm³`, authorised **542.288**, removed **541.000**, tip-only `527.788` — the 62.186 to the analytic is the **crest**, which is the thread |
| Determinism and the old bytes | workspace | every new subcommand run twice prints and emits identical bytes (inlay pair, thread and the rest, asserted from the document on disk through the program); and the pre-existing command surface was run against `master`'s tip `179d6b3` in an identical sandbox — 29 invocations over `validate`, `inspect`, `toolpath` (profile, pocket offset, pocket raster with an island, drilling with a bank), `emit`, `check`, `postp validate`, `tools list`, `tools add`, `import` (report and `--out`), `simulate` at three resolutions, the error paths, an unknown command and an unknown kind — comparing exit code, stdout, stderr and **every produced file** (documents as their JSON payloads, programs byte for byte, the imported document, the library). Exactly two differ, both by design: `--help` and the unknown-`--kind` message. Everything else, including the `--islands` refusals and a four-toolpath `simulate` report, is byte-identical. A third, deliberate difference sits outside that sandbox because the CLI cannot write the document that shows it: `validate` on a document carrying islands on a profile toolpath now says "takes islands only for a pocket or an inlay" — the same exit code, the sentence reworded because an inlay male takes islands (T8) |
| No new dependency, and the editor unchanged in what it authors | workspace, `crates/cam-ui` | `Cargo.lock` is unchanged between `179d6b3` and this branch — the same 470 `[[package]]` entries. `cam-ui` gained one thing and it is not an authoring surface: the 3D preview's segment carries the tool's **shape**, so the field it cuts is the field the program's tool would leave (its own 72 tests, the pre-M6 shell expectations among them); no toolpath form, no new command, and the 2D canvas untouched |

The milestone found one defect it did **not** fix, and it is recorded rather than buried. `cam_sim::grid::grid_for`
builds the field at origin `(0, 0)` from `job.width/height`, i.e. in document coordinates, while programs
are emitted in machine coordinates (`cam_toolpath::machine_point` translates or flips them for `top_left`
and `center`), and the sweep cuts the field with the move coordinates as they are. So at any origin corner
other than `bottom_left`, and **for every strategy since M5**, the field and the program disagree about
where everything is. Measured: a single pocket over an 80 x 30 mm region at `y = 60 … 90` reports
`gouge 14376.000 mm³ over 2396 cells` and `uncut 14400.000 mm³ over 2400 cells` at `top_left`, against
`gouge 0.000` and `24.000 mm³` over 4 cells at `bottom_left`; with `origin: center` a 100 x 80 mm square
pocket's `removed` collapses from 47976.000 to 2394.000 mm³. The emitted G-code is unaffected — it is what
the machine runs — but the report and the 3D preview agree with the document only at bottom-left, and a
`bottom_left`-symmetric fixture hides it, which is why every fixture and the default sheet missed it. It
needs its own slice: a fix moves the grid's origin and the sweep's cell mapping together, and nothing
short of that is honest.

Recorded deviations, each a decision in the feature document: **D-M6-1** delivers the six cheapest first
and the post-contract change last, so the riskiest unit could not block the other five; **D-M6-2** derives
a chamfer's depth once, in the model (amended twice — the bevel is symmetric so there is no side, L7, and
a flat tip is refused by name rather than absorbed, D-M6-10); **D-M6-4** makes an engrave the profile's
`On` side with the pass law deleted, and **D-M6-5** makes a flute's ramp a length along the vector with an
arc carried as a helix; **D-M6-6** makes a texture a pattern and never a pocket, with the pattern derived
once; **D-M6-7** ships the inlay as two pocket toolpaths sharing one depth rather than two extra step
parameters, and **D-M6-13** records that the male cuts rows because the ring lattice left 2300 mm² of the
frame standing; **D-M6-9** consumes the library's `included_angle` and `flat_diameter` and refuses a
missing field by name instead of defaulting 90°; **D-M6-11** leaves the CLI's "a flag no kind uses is
silently dropped" open milestone-wide — closing it changes the contract for callers that pass a superset
today — while every M6 kind refuses by name the flags its own law forbids; and **D-M6-12** amends D-M5-3
so the sweep interpolates Z along a cutting move, which is what lets a ramp be one move and a helix be
swept honestly. Two amendments are recorded because the specification got them wrong: A9's oracle
arithmetic asserted one flank where a symmetric bevel removes two (L9), and A10's oracle for the thread
asserted the annulus where the helix leaves a spiral (L17).

### 4.10 M7 delivered (evidence)

M7 is the milestone the roadmap isolated on purpose (§4.1), and it earned the isolation: it is the only
strategy whose toolpath is not a pass sequence but a **depth field**, and it needed two things the
workspace did not have — an exact distance from a point to a region's boundary, and a medial axis of that
region whose every vertex carries a certificate rather than a hope. Its shape is the mirror image of
M6's: before writing any specification M7 asked the post-contract question M6 had answered the expensive
way, and got the other answer (L4). `FEED_MOVE` is `"[N]G01[X][Y][Z]"` and D-M6-12's sweep interpolates Z
along a cutting move, so a per-vertex Z feed is already expressible: **M7 changes no post contract at
all**. What it spent instead is geometry, plus one defect M6 recorded and did not fix.

The law is derived once and asserted in both directions. A V-bit's body is above its cone, so a tip at
`−d(p)/tan(θ/2)` can only leave material standing — `|q − p| − d(p) ≥ −d(q)` because `d` is 1-Lipschitz —
and the region is exactly the union of the medial balls `B(p, d(p))` over the axis, so the axis is the
smallest set of tips that covers it. Two consequences are tests rather than claims: **gouge is exactly
zero** for any tip set inside the region, and a dropped piece of axis costs `uncut` and nothing else.
Error is one-signed by construction, which is why the tolerance is a coverage knob and never a depth knob.

The research unit was T2, and measurement changed its design three times: the route (a scalar bisection
along a lattice edge replaced "bisectors of boundary element pairs", so **arcs stay arcs and nothing is
tessellated** — D-M7-4's premise superseded), the certificate (contacts deduped at a tenth of the
tolerance, because a shared joint was being counted as two and the 5 x 2 ellipse reported **13 branches at
0.1 mm against 1 at 0.5 mm**), and the topology (a stitch pass joins the two ends a refused candidate
leaves, so the square's four branches stayed four, the L's ten became nine and the ellipse's thirteen
became one). The property that guards all of it is
`a_finer_tolerance_never_returns_more_branches_than_a_coarser_one`.

The feature document is `odd/tasks/m7-vcarve.md`; the law, the axis's certificate, the walk, the
refusals, the tolerance's four declared costs, the simulation's claim and the three defects T0 found are
in [docs/m7-vcarve.md](../docs/m7-vcarve.md). The workspace stands at **1056 tests across 79 test
targets** — 81 more than the 975 the branch cut from — green under `./scripts/check.sh` (`fmt --check`,
`clippy -D warnings`, the whole suite). In the crates M7 touches that is cam-geom **240**, cam-model
**58**, cam-toolpath **136**, cam-sim **109** and cam-cli **121**.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The exact distance from a point to a region's boundary, arcs native, with membership in the same answer | `crates/cam-geom::region::Region::distance_to_boundary`, `cam-geom::segment::Segment::nearest_point` | 11 tests in `tests/region_distance.rs`; a point inside a square is as far from a wall as the nearest wall, the centre of a circle is exactly one radius away and **not** the apothem of a chord, an annulus reports the nearer of its two radii, a point on the boundary is exactly `0.0` away and not material, and the tolerance decides membership but never the distance; a region with no boundary answers `None` rather than `0.0` or a panic |
| The verified medial axis: branches and vertices with exact clearances, candidates dropped whole, deterministic order | `crates/cam-geom::medial` | 14 tests in `tests/medial.rs` and 3 in `tests/medial_coverage.rs`; `every_retained_vertex_of_every_fixture_is_a_certified_medial_point` asserts the empty-ball certificate on seven fixtures, and the fixtures' shapes are asserted by name (a square's four diagonals meeting at the centre, a disc's centre alone — with a caller-tessellated disc producing 72 branches, so the difference is the caller's chords, not the module's); two runs return the same branches in the same order vertex for vertex; `a_finer_tolerance_never_returns_more_branches_than_a_coarser_one` and `a_finer_tolerance_never_covers_less` pin the direction, and **coverage is measured at both tolerances on six fixtures: `0.97694`–`0.99997` at 0.5 mm and `0.99670`–`1.00000` at 0.1 mm**, with the largest missed clearance `0.20` mm at 0.5 and `0.00` at 0.1 |
| `Operation::VCarve` and its record inside `c2d/document@2`, the depth law beside the tool, refusals at load | `crates/cam-model` | 6 tests in `tests/document.rs`; the record round-trips losslessly with its angle and tolerance, `VCarveParams::depth` is `clearance / tan(θ/2)` (10 mm of clearance on a 60° bit is `10·√3` deep), **no `start_depth` field exists** (D-M7-3), an angle outside `(0, π)` and a tolerance that is not a positive length are refused at load naming the value, an open contour is refused as such, and the schema tag does not move |
| The walk: one linear feed per tip at that tip's own Z, a retract between branches, no arc move | `crates/cam-toolpath::vcarve` | 10 tests in `tests/vcarve.rs`; the emitted moves equal, move for move, a walk built from the axis's own certified vertices at `clearance / tan(θ/2)` written out by the test, so a wrong depth cannot agree with itself; a 90° bit reaches the clearance itself; a disc's program is one plunge and one retract; a `top_left` origin maps every tip into the machine frame; the tolerance changes the axis's step and never a single Z; an open contour, an angle outside the cone, a tool with no angle and a flat-tipped bit are each refused by name; a branch that passes a junction is walked through it |
| `cam-cli toolpath --kind vcarve`: the flag, its default, its 17 named refusals, and `inspect` | `crates/cam-cli` | 9 tests plus the goldens below; the stored angle and tolerance are the library's and the flag's, `--tolerance` defaults to the documented `0.1 mm` (a document authored with the flag and one without are byte-identical), a flag the kind has no meaning for is refused by name with the file byte-identical (17 of them, `--start-depth` and `--clear` among them), a tool with no angle and a flat-tipped bit exit `2` naming the tool and the flat, and `inspect` names the kind |
| An empty axis is refused by name instead of written as a document whose only outcome is `uncut` (T7, the gap T3–T6 left) | `cam-toolpath::ToolpathError::VCarveNothingToCarve`, `crates/cam-cli` | `a_vcarve_whose_axis_is_empty_exits_two_naming_the_tolerance_and_the_region`: a 20 x 20 mm region at `--tolerance 50` exits `2` with `the V-carve's 50 mm tolerance leaves nothing to carve in the 20 x 20 mm region …` and the document byte-identical; the error contract's exhaustive message list grew **23 → 24**; `emit` and `simulate` refuse the same document in the same words, because they compute the same walk |
| The simulation's claim for a V-carve, and the oracle in both signs (S11) | `crates/cam-sim::target::claim_v_carve`, `tests/vcarve.rs` | 5 tests on a 0.5 mm grid with a 0.1 mm axis: a 20 x 20 mm square removes **1332.5** of its analytic `1333.333 mm³` and a radius-15 disc **3534.152** of `3534.292`, each with `uncut 0`, `gouge 0` and the apex at `−10` and `−15`; a **deliberately thinned axis** reports `uncut 142.043 mm³ over 642 cells` and `gouge 0`; a program **20% too deep** reports `gouge 629.557 mm³ over 1937 cells`; and the same square at a **0.5 mm axis** reports `uncut 0.207 mm³ over 4 cells` with still `gouge 0` — D-M7-5's convex-corner band, measured |
| The frame is honoured at every boundary, with a test per origin corner (T0, A7) | `crates/cam-model::Frame`, `cam-sim::sweep`, `cam-toolpath`'s three arc/helix construction sites | 13 tests: 4 in `cam-model/tests/frame.rs` (each corner maps the job's corners to the machine corners it names, machine↔document is a round trip, only bottom-left is the identity, only top-left mirrors handedness), 3 in `cam-sim/tests/frame.rs` (an arc, a helix and a crash cut and are reported where the document puts them), 3 in `cam-toolpath/tests/frame.rs` (a reflected origin emits the complementary arc and helix, a translated one does not, and `every_origin_corner_emits_the_program_bottom_left_emits_once_read_back`), 2 in `cam-sim/tests/origin.rs` (a correct program gouges nothing and reports the same numbers at all three corners) and 1 in `cam-cli/tests/cli.rs` |
| Golden programs for a square and a circle region, each asserted three ways | `crates/cam-cli/tests/cli.rs` | 6 tests; a 20 x 20 mm square at a 90° bit and a 0.1 mm axis emits **413 lines** — four branches, 396 feed tips, the corner tip at `−0.2000`, the apex at `−10.0000` — and a radius-15 disc emits **12 lines**, one plunge at `−15.0000` and not one lateral move; each is pinned as built, after a `c2d` save/load round trip and with the toolpath authored through `cam-cli toolpath`, and every feed's Z was **read against the geometry** before pinning (396 of 396 exactly `−distance`, worst deviation `1.4e-15` mm) |
| Determinism and the old bytes, and no new dependency | workspace | two runs of `toolpath --kind vcarve` and of `emit` print and emit identical bytes, asserted from the document on disk through the program; the pre-existing command surface was run against the branch's cut point `9ff0b8c` in an identical sandbox — **35 invocations** over `validate`, `inspect`, `check`, `postp validate`, `tools list`, `tools add`, `toolpath` for profile, pocket offset, pocket raster with an island, drilling with a bank and thread, `emit` (to a file, to stdout, with an arc-less post, with an arc-only post and without a library), `simulate` at 1, 0.5 and 2 mm and at a resolution the cell cap refuses, `import` as a report and with `--out`, the error paths, an unknown command and an unknown kind — comparing exit code, stdout, stderr and **every produced file** (**844 file snapshots** hashed, documents as their `document.json` payloads). Exactly two differ, both by design: `--help` and the unknown-`--kind` sentence. `Cargo.lock` is unchanged: the same **470** `[[package]]` entries |

T0 found **three** defects older than the milestone, and none was in the feature document's scope. The
first is M6's recorded one, now closed: the field and the program disagreed about the origin at every
corner but bottom-left, and on M6's own fixture (an 80 x 30 mm band in a 200 x 100 mm sheet, 1 mm cells) a
**correct** program reported `gouge 14376.000 mm³ over 2396 cells` and `uncut 14400.000 mm³ over 2400
cells` at `top_left`, against `gouge 0.000` at bottom-left. After `786c3d8` (the `Frame`, the field's
mapping) and `228ecc7` (the program's handedness) the same fixture reports its bottom-left numbers
**identically at all three corners**: removed `14376.000`, gouge `0.000`, uncut `24.000 mm³ over 4 cells`.
The second and third were found by the fix itself: a reflection **reverses the handedness of an arc**, and
`cam-toolpath` passed `ccw` through unchanged while mapping only the points, so at `top_left` the emitted
program stated the **complementary** arc — a `G02` where the machine needs `G03`, a wrong cut on real
hardware since M4, invisible because every fixture was bottom-left. The third was `thread.rs`'s helix,
where only the orbit's start was in machine space: `top_left` emitted the opposite orbit **and** the
opposite hand. All three are fixed, all three have tests, and bottom-left programs are byte-identical
afterwards (five `emit` fixtures compared with `cmp`, 22 `G02`/`G03` lines among them).

Recorded deviations, each a decision in the feature document: **D-M7-1** ships V-carve to the point — no
relief pass and no flat bottom — and that is a consequence of the law rather than an omission, because at
a boundary `d = 0` so the surface tapers to zero depth exactly at the edge, sharp corners included;
**D-M7-2** separates cheap candidate generation from an exact certificate, with the nearest set deduped by
**point** so a shared vertex is one contact; **D-M7-3** defers `start_depth`, by name, because with a start
depth the cone's top reaches outside the vectors and "gouge is exactly zero" would be false for a
legitimate program; **D-M7-4** is recorded as **superseded in its premise** — it said arcs would be
tessellated for the skeleton, and the implementation bisects a scalar instead, so arcs stay arcs and no
chord tolerance exists anywhere; **D-M7-5** is **superseded in its second half** as well: it said "M7 adds
no tolerance flag and takes the job's existing tolerance", and the implementation made the tolerance a
stored, defaulted `--tolerance` (S9/S10: `emit` recomputes the axis from the document alone, so a flag that
was accepted and dropped would make the program depend on a constant nobody named); its four costs are now
declared and measured. **D-M7-6** made the origin fix a prerequisite rather than a detour, which is why
the milestone opens with a coordinate change; **D-M7-7** does not extend `Move::Arc` and does not reuse a
helical block, because a V-carve's Z is a function of position; and **D-M7-8** records the crossing point
the implementation found: the field, the target and the preview stay in the document frame and the program
is read into it once, rather than moving the grid — because M5 places a tab by **arc length from the
contour's start**, so reflecting the target's geometry would move every tab to the other side.

**This milestone's diff has had no native outside view.** The candidate was submitted twice and both times
refused in preflight with `lens_context_budget_exceeded` (`mutation_performed: false`, no authority
created, nothing abandoned): **4 425 inserted lines across 7 commits**, with an explicit `--base-ref` of the
branch's cut point `9ff0b8c`. The second attempt asked for **one** lens instead of three and was refused
identically, which is the fact worth keeping: the budget is bounded by the candidate's size, not by the
lens count. The owner's disposition was to disable the review switch for this clone and deliver M7 on its
own evidence, the path M6 took — recorded in the feature document as L12 and here rather than softened.
The milestone's delivery therefore rests on its tests, its simulation oracle in both signs, its two golden
programs and its 35-invocation differential; M6's "merged after a review the tooling refused" has become
M7's "delivered with the review switch off, at the owner's decision", which are not the same fact either.

### 4.11 M8 delivered (evidence)

M8 is the eleventh and last strategy scope D2 names, and it is the cheapest of the three strategy
milestones that followed M5 — which is worth recording, because the row that scheduled it expected the
opposite ("Needs the 3D-adjacent math from M7's distance field"). The cost was not the distance field:
S3's decision made the rail an XY path, the cross-section authored `(lateral, depth)` data and the
stepover the spacing of the lateral passes, so **M8 changes no post contract** (a pass has one Z, and
`FEED_MOVE` already carries `[X][Y][Z]`), bumps **no schema** beyond the new `Operation` variant — the
rail rides on `entity_ids`, so a document written before M8 loads and saves byte-identically — and adds
**no crate and no dependency**. What it spent is geometry: a rail walked by **arc length** with the
contour's arcs kept as arcs, a cross-section that is a *function* of lateral distance, and a stepover law
whose coverage is a certificate rather than a hope.

The owner's decision is the whole specification (S3, verbatim: "Sección Z barrida por el rail" — rail in
XY, profile of `(lateral, depth)`, stepover as overlapped lateral passes), and its five readings are
decisions with tests behind them: lateral is positive **to the right of travel** (so a counter-clockwise
closed rail's positive side is *outside* the loop, which a circle fixture pins by claiming material on
both sides of the rail and nothing in the middle); the rail may be **open or closed**; the profile's
laterals **strictly increase** because a cross-section is one depth per lateral distance; a stepover
**equal to** the profile's width is the **two edge passes**; and a stepover **wider** than the profile is
**one pass centred on it**, because with that spacing no second distinct pass exists and a pass at one
edge would cut one authored edge and leave the other. The law lives in `cam-geom` and not in the strategy
because it is lateral-offset geometry, and it states its own guarantee as one predicate —
`StepoverLaw::covers`, "every lateral position of the profile is within half a stepover of a pass" — so
the strategy and the claim rely on a sentence instead of restating the arithmetic.

The gouge argument is a **different** one from the V-carve's, and the milestone says so rather than
reusing M7's words. M7 rests on a 1-Lipschitz distance field; M8 rests on **coverage plus a monotone tool
law**: the claim and the program read the same pass polyline — `Rail::offset_walk` over the same
`Rail::samples(DEFAULT_CHORD_TOLERANCE)` walk — the sweep then samples that polyline more finely at
points that lie *on* it, and the tool's `rise` grows with the distance from it — so a cell's claimed
depth is never shallower than the program's and a correct program can never remove more than was
authorised, for a cylinder and a cone alike. The claim reads the rail, the profile and the stepover from
the **document**, never from the emitted moves, and claims are never summed (D-M5-2: the deepest wins).

**One corner of that argument was false at delivery, and it is now closed.** A *closed* rail whose vertex
list **starts at a corner** has a seam where the walk's last sample and its first are the same point
carrying two different normals; the strategy bridged them with a closing feed the claim's walked path did
not contain. On a 40 x 20 mm frame that was `gouge 2.000 mm³ over 1 cell` — the seam diagonal's own
centre — on a program that was otherwise exact. A seam **in the middle of an edge** is collinear, joins
itself and was exact in both signs, which is the fixture the frame oracle and the frame golden use. The
trigger (the drawing's own start vertex, arbitrary in a DXF and wherever the user began a rectangle) made
it a real case, so it was pinned rather than hidden by
`a_closed_rail_whose_start_vertex_is_a_corner_reports_the_seam_diagonal_as_gouge_a_known_limit`, and the
fix came with that test's inversion: the pass's polyline is now `Rail::offset_walk` in `cam-geom` — the
samples offset by the pass's lateral plus the closing edge a corner seam needs, and nothing when the seam
is mid-edge (`SEAM_EPSILON`) — which the strategy emits and the claim authorises against, so the two
cannot disagree about where a pass cuts. The corner frame is exact in both signs (`496.000` mm³ removed
against `496.000` claimed, over the seam-smooth frame's own **248** cell centres), the mid-edge frame's
numbers did not move, and a differential against `deb8b1a` over the moulding command surface — five rails
(imported corner-seam, mid-edge, circular, rounded-rectangle and open-arc frames), `toolpath`, `inspect`,
`emit`, `simulate` at 0.25, 0.5, 1 and 2 mm, and eight refusals, 42 compared sections — differs in
`simulate` alone, on the corner-seam documents: `gouge 3.000 mm³ over 24 cell(s)` → `0.000` at 0.25 mm,
`3.000` over 6 → `0.000` at 0.5, `2.000` over 1 → `0.000` at 1, exit **6** → **0**. Every emitted program
and every produced file is byte-identical (all file hashes equal), which is the fact that says the fix
moved the claim and not the program.

The feature document is `odd/tasks/m8-moulding.md`; the sweep, the stepover certificate, the walk,
the refusals and the limits are in [docs/m8-moulding.md](../docs/m8-moulding.md). The workspace stands at
**1374 tests across 101 test targets** (88 binaries and 13 doc-test targets) — 79 more than the 1295 the
branch cut from `b322bed`, and 16 of them M8's closing evidence — green under `cargo test --workspace`.
In the crates M8 touches that is cam-geom **274**, cam-model **62**, cam-toolpath **154**, cam-sim **126**
and cam-cli **140**.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The rail by arc length, the cross-section as a function, the swept band and the stepover law | `crates/cam-geom::moulding` (`Rail`, `CrossSection`, `SweptBand`, `StepoverLaw`) | 24 tests in `tests/moulding.rs`; a point asked for on an arc is on the arc and not on a chord, a corner appears twice with two normals, a cross-section's depth is read between its own authored points and holds its edge depth outside them, a profile that doubles back is refused by name, the band's own containment certificate holds for every sample of the offset family, and the law's five cases are pinned: start-and-step passes, both edges always cut, `stepover == width` as two passes, `stepover > width` as one centred pass, and `coverage_radius ≤ stepover / 2` |
| `Operation::Moulding` and its record inside `c2d/document@2`, validated at load | `crates/cam-model` | 4 tests in `tests/document.rs`; the record round-trips losslessly with its profile and stepover, **no `Toolpath` field and no schema bump** — the rail is the entity, so a document written before M8 is byte-identical — a profile of fewer than two points, a non-finite coordinate and a stepover that is not positive are refused naming the value, and a **closed** rail is accepted as well as an open one |
| The walk: one lateral pass per stepover offset, one feed per point of the pass's offset walk, a retract between passes | `crates/cam-toolpath::moulding` | 13 tests in `tests/moulding.rs`; the emitted moves equal, move for move, a walk built from the authored laterals and depths written out by the test, a closed rail closes its walk and a seamless one emits no duplicate move, the program carries the tool's real shape so the simulation sweeps the cross-section the bit cuts, and a profile shorter than two points, a lateral that does not increase, a stepover with no distance behind it, a rail with no length and a tool with no diameter are each refused by name |
| The simulation's claim, and the oracle in both signs (S6) | `crates/cam-sim::target::claim_moulding`, `tests/moulding.rs` | 17 tests in one file, 7 of them T7's closures: a straight rail removes its analytic **660 mm³** with `gouge 0` and `uncut 0`; a **closed frame** (the whole rail turning four corners and joining itself) removes its **496 mm³** with `gouge 0` and `uncut 0`; 1 mm shallow gives **`uncut 300` over 300 cells** on the rail and **`uncut 248` over 248** on the frame, both with `gouge 0`; 1 mm deep gives **`gouge 300`** and **`gouge 248`** over every claimed cell with `uncut 0`; a **V-bit cone** removes its claim to the last bit (`1027.127 mm³`) with both signs zero and reports **`gouge 603.671 mm³ over 676 cells`** 1 mm deeper; two mouldings on one rail keep the deeper claim and never the sum; and the **corner seam** frame is exact — `496.000` mm³ removed against `496.000` claimed, `gouge 0` and `uncut 0` over 248 cells, pinned by `a_closed_rail_whose_start_vertex_is_a_corner_claims_its_seam_diagonal` (post-delivery, from the `gouge 2.000 mm³` over 1 cell it was delivered with) |
| `cam-cli toolpath --kind moulding`: the flags, the 22 named refusals, `inspect`, and determinism | `crates/cam-cli` | 11 tests, with the goldens and the reports in the two rows below; the stored cross-section and stepover are the flags' (and the library's when `--stepover` is absent), `--profile` belongs to this kind and is refused by name elsewhere, `--stepover` defaults to the library's and refuses with the flag named when it has none, a flag the kind has no meaning for is refused by name with the file byte-identical (22 of them, `--tolerance` and `--clear` among them), a profile below two points and laterals that do not increase exit `2`, `inspect` prints `moulding (3 points, 2 mm stepover)`, and two runs print and emit identical bytes |
| Golden programs for a straight rail and a closed frame, each asserted three ways | `crates/cam-cli/tests/cli.rs` | 6 tests; a 60 mm open rail emits **21 lines** — 3 passes at `Z-1.0000`, `Z-3.0000`, `Z-1.0000` — and a 40 x 20 mm closed frame emits **33 lines** — 2 passes at `Z-2.0000`, the inner one with a repeated corner sample per corner (4 bare `G01` blocks) and the outer one with a 45 degree diagonal across each corner; each is pinned as built, after a `c2d` save/load round trip and with the toolpath authored through `cam-cli toolpath`, and every line was read against the law before pinning |
| Three reports end to end, and the multi-rail case | `crates/cam-cli/tests/cli.rs` | 3 tests; a correct straight-rail moulding prints `program: 12 move(s)`, `removed: 300.000 mm3`, `gouge: 0.000`, `uncut: 0.000` and exits `0`; a frame asking 2 mm out of a **1 mm stock** prints `uncut: 248.000 mm3 over 248 cell(s)` and exits **6**; and a toolpath naming **two rails** cuts and claims both (`removed: 600.000 mm3`, `gouge: 0.000`, `uncut: 0.000`, exit `0`), which is the multi-rail claim proven end to end rather than assumed |
| Determinism, and no new dependency | workspace | two runs of `toolpath --kind moulding`, of `simulate` and of `emit` are byte-identical (each golden test emits its program twice, and the authoring tests compare the documents on disk); and `Cargo.toml`/`Cargo.lock` are unchanged from the branch's cut point `b322bed` (`git diff b322bed -- Cargo.toml Cargo.lock crates/*/Cargo.toml` is empty): no crate and no dependency was added |
| The shared chord constant, and the limit M8 no longer has | `cam-sim`'s oracle, `docs/m8-moulding.md` | `DEFAULT_CHORD_TOLERANCE` (`0.001` mm) is recorded as a constant `cam-geom`, `cam-toolpath` and `cam-sim` all read — which is what makes the claim's polyline the program's — which **no test pinned** when this row was written; **and the gap was wider than the row said**, because every moulding fixture in `cam-sim` is a straight rail, where a segment samples to one chord at any tolerance, so mutating `claim_moulding`'s call site to `0.01` left all 17 of its tests green. Pinned now, on curved rails, by `a_mouldings_program_is_walked_at_the_one_chord_tolerance_the_claim_reads` (`crates/cam-cli/tests/cli.rs`: the value, the tolerance proved load-bearing, and one `G01X` per further sample of each pass) and `a_mouldings_claim_on_an_arc_rail_is_the_walk_at_the_one_chord_tolerance` (`crates/cam-sim/tests/moulding.rs`: the claimed cells and volume are the default walk's), each proved by installing the mutation and watching it fail. The closed-rail **seam corner**, delivered as a recorded limit at `gouge 2.000 mm³ over 1 cell` on a correct program, was closed post-delivery by `Rail::offset_walk`, with the test it was pinned by inverted and the CLI differential recorded above |

Recorded deviations, each a decision in the feature document: **no tolerance flag, by design** — a
moulding's error is the rail walk's chord error and nothing else, so `--tolerance` is refused by name, and
the constant that sets it is shared by three crates and pinned by none (recorded, not papered over); **no
start depth and no cut depth**, because the cross-section *is* the depth law; **a stepover wider than the
profile is one centred pass** rather than a refusal, which is the honest single answer for that spacing;
and **the seam corner**, delivered as the one place where "gouge is exactly zero" was false for a correct
program and closed afterwards rather than argued away.

The candidate's native review disposition is the delivery's own record, written by the transaction that
delivers it; what this section states is what the tests, the oracles in both signs and the two golden
programs establish.

### 4.12 M9 delivered (evidence)

M9 is not a machining strategy at all — it emits no move, so **no post contract changes** — and it is
cheap in what else it touches: the `nesting` section is an **optional top-level field**
beside `tools_ref` and `postp`, so **no schema bump** (`DOCUMENT_SCHEMA` stays `c2d/document@2` and a
document written before M9 loads and re-saves byte-identically); and it adds **no crate and no
dependency** (`Cargo.toml`/`Cargo.lock` unchanged from the milestone's cut point `931812e`). What it spent
is two packers, a certificate each, and the one arithmetic the owner asked to be **both** report and
field. The milestone is seven commits, ten files, **6 606 insertions and 7 deletions** in total.

The owner's four answers are the whole specification (S1–S4, verbatim): "Un documento maestro con N
hojas y N instancias"; "Guardar los placements en el documento"; "Los dos: reporte y campo"; "No: las
hojas se autoran y viven en el documento". So a part is a selection of the document's own entities plus
an instance count, there is no parts library and no default board, the sheets live in the document, and a
run writes the placements and the cost back into it. Two rules make that safe to store: a placement's
`position` is the **placed** footprint's lower-left corner with the quarter turn applied before anchoring
(D-M9-1, the defect T1's tests caught — turning about the part's own origin put the footprint at negative
X), and an authored section with **no placements** is a request that loads, while from the first
placement on every part's instance count must equal its placements.

Both nesters S6 names are reachable by name and each states its guarantee as **one predicate**. Maxrects
and guillotine run on the same area-descending stable list, and the auto rule prefers fewer unplaced
parts, then fewer sheets, then maxrects; `NestPlan::certifies` is the sentence "inside the sheet, and a
gap apart on it" and every test asserts it **beside** its own clearance measurement rather than instead
of one. The fixture where the two heuristics disagree is the proof that half of S6 is not decoration: a
**234 × 173 mm** board at gap 8 with parts **110 × 60**, **111 × 87** and **38 × 112** needs **2 sheets**
with maxrects (utilisation `0.25335951781038485`) and **1** with guillotine (`0.50671903562076970`),
because maxrects' split around the 111 × 87 part leaves an 86 mm top strip where the 38 × 112 needs 112
and the guillotine's full-length cut leaves a 242 × 86 strip for the 110 × 60. On the cabinetry fixture
that S7 exists for — a **2440 × 1220** board, gap 8, two 720 × 580 sides and three 560 × 520 shelves —
the plan is **5 parts on 1 sheet, 0 unplaced, `0.57403923676431068`** (1 708 800 of 2 976 800 mm²).

Free profiles take `cam-geom::nest_profile`, and the milestone records its limit as a **measurement**
rather than an adjective: the heuristic is **sound** (a placement is accepted only when the candidate's
outline is `Disjoint` from every outline already on the board and from every no-fit region on it) and
**incomplete**, which the counted fixture states — a **100 × 100** board, a 40 × 100 bar, a 50 × 50
square at gap 5 and step **20** puts the square on a **second board** although **`(45, 0)`** is legal,
and step **5** finds it at `(50, 0)`, one step past the no-fit region because contact is refused. There
is no exact NFP to reach for: building the Minkowski difference needs a boolean **union**, and `Region`
offers `subtract` and nothing to build a region *with*. Inside-ness is the board's **material**, not the
absence of a crossing — `FirstInside` outright, `Touching` only when the part inset by **`INSET_STEP`**
(0.01 mm) is itself strictly inside — and that rule is the review's one CRITICAL defect, corrected with
three REDs before any `src/` edit and a differential of the 11 pre-existing profile tests against
`3508eb9` (11 passed both sides).

Utilisation has **one** arithmetic and the milestone spent a review cycle on it. `Document::nesting_utilisation`
is placed footprint area over the area of the sheets **the placements name**, waste its complement, and
the stored `NestingCost` must equal it; the denominator is pinned with exact numbers — five 100 × 100
placements on a **500 × 400** sheet is **50 000 / 200 000 = 0.25** with a second authored-but-unused
**1000 × 1000** sheet deliberately excluded, and the T1 fixture pins **30 000 / 240 000 = 0.125**. The
strong rule then indicted **T1b's own fixture** (0.25/0.75 sums to one and is wrong about the area) and
six tests went red for it; the fixture was mended to the derived pair and `NestingCostFractionsDoNotSumToOne`
was deleted rather than kept for its message, because the sum is now implied. `cam-cli nest` writes the
record from that function and never from `NestPlan::utilisation`, which divides by every board the plan
opened — the nester's view, and the second arithmetic the rule exists to forbid.

The command is the milestone's user-visible surface: `--sheets` (repeatable, replacing the document's
own list), `--gap`, `--rotation`, `--algorithm`, `--step` and `--out`, a report on stdout, the placements
and cost written into the document, and **exit `7`** for a nesting that could not place every instance —
not `2`, because the document is valid, and not `6`, because no program was simulated: it is a finding
about the document's own request, and **nothing is written**, since `Document::validate` refuses a
placements-versus-instances disagreement and a partial nesting therefore has no honest stored form. A
flag a document's own classes give no meaning to (`--step` with no profile, `--algorithm` with no
rectangle) is refused by name, M8's rule; so are two board sizes, because both nesters pack identical
boards.

The feature document is `odd/tasks/m9-nesting.md`; the two nesters and their laws, the two
certificates, the inside-ness rule and its `INSET_STEP`, the one arithmetic, the CLI surface with exit
`7`, the refusals, both review lineages and the known limits are in [docs/m9-nesting.md](../docs/m9-nesting.md).

The workspace stands at **103 test binaries** green under `cargo test --workspace`, and in the crates M9
touches that is cam-geom **302**, cam-model **93** and cam-cli **157** — 28 more in `cam-geom` than the 274
M8 left, 31 more in `cam-model` than 62, and 17 more in `cam-cli` than 140.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The `nesting` section in `c2d/document@2`: sheets, parts, placements, cost and their refusals | `crates/cam-model` (`Nesting`, `check_nesting`) | 31 tests in `tests/document.rs`; a pre-M9 JSON literal loads, validates and re-saves **byte for byte** with `nesting` absent and the schema tag does not move, the section round-trips losslessly, and a non-positive sheet dimension, a duplicate sheet or part id, a part with no entity, a part naming a missing entity, a placement naming a missing part or sheet, a negative position and a footprint that leaves its sheet are each refused **naming the value**; two placements that overlap are refused while two that share an edge are accepted; a part whose instance count disagrees with its placements is refused while an authored section with **no** placements loads |
| One arithmetic for utilisation and waste, and a cost record that must equal it (S3) | `Document::nesting_utilisation`, `placement_footprint`, `NestingCost` | five 100 × 100 placements on a **500 × 400** sheet pin **50 000 / 200 000 = 0.25**, with a second authored 1000 × 1000 sheet that no placement names **excluded** from the denominator, and the T1 fixture pins **30 000 / 240 000 = 0.125**; a record that agrees loads while a wrong sheet count, a fraction outside `0..=1`, a utilisation that is not the derivation (`0.25` recorded against `0.125` derived) and a waste that is not the complement are each refused by name, and the tolerant boundary is `0.25 + 1e-6` accepted and `+2e-6` refused. The strong rule indicted T1b's own hand-invented 0.25/0.75 record and six tests went red for it |
| Rectangle nesting: maxrects, a guillotine pass, the auto rule, and one certificate | `crates/cam-geom::nest::{NestOptions, NestAlgorithm, NestPlan}` | 14 tests in `tests/nest.rs`; the cabinetry fixture (2440 × 1220, gap 8) packs **5 parts on 1 sheet, 0 unplaced, `0.57403923676431068`**, and the contested fixture (234 × 173, gap 8) needs **2 sheets with maxrects (`0.25335951781038485`) and 1 with guillotine (`0.50671903562076970`)**, both certified, with the auto rule picking each by name; the gap is a measured distance between neighbours, a part is turned only when it does not fit upright, a part larger than the board is unplaced rather than an error, equal areas keep the caller's order, the certificate holds at the sheet's own edges and fails when a plan breaks it, and two runs are the same plan |
| Free-profile nesting: a sound, incomplete no-fit heuristic (S6, S7) | `crates/cam-geom::nest_profile` | 17 tests in `tests/nest_profile.rs`; a circle and a rectangle pack on one board, two contours that would overlap never do, the authored gap is measured between two placed contours, a concave part sits in a notch, a part is turned only when no upright placement exists, an oversized part and a part that only **touches** a circular board from outside are unplaced, a flush part is placed and certified on a rectangle and on an L's arm while one `INSET_STEP` past the edge is refused, a second board opens only when the first cannot hold the part, the same input twice is the same plan, and the certificate fails when a plan breaks it; the incompleteness is the counted fixture — **step 20 costs a second board for a 50 × 50 square although `(45, 0)` is legal, and step 5 finds it at `(50, 0)`** |
| `cam-cli nest`: the flags, the refusals, the report, exit `7` and the write-back | `crates/cam-cli` | 16 tests, five of them whole-report goldens: a 600 × 400 panel on a 1000 × 600 board prints `utilisation: 0.400` / `waste: 0.600` and stores the record the document derives; two instances that no longer share a board open and author `S2`; a 1200 × 500 part that fits no board prints `sheets used: 0`, `utilisation: none`, `unplaced: P1, 1 of 1 instance(s)` and exits **7** with the file byte-identical; an L bracket goes through the no-fit heuristic and the report names it with its step (`0.200`, conservative by D-M9-2 where its own area would be `0.100`); a 500 × 900 part enters a 1000 × 600 board only turned, as 900 × 500 at `0.750`; `--out` writes elsewhere and leaves the input alone; `--step` on rectangles and `--algorithm` on profiles are refused by name with the file unchanged, as are `--gap -1`, `--sheets 1000`, `--sheets 0x600`, `--step 0`, `--rotation sideways`, `--algorithm triangle`, `--cut-depth`, two board sizes, a part that is not one closed contour, a document with no parts and one with no boards; and `--help` lists the command, its six flags and `7` |
| Determinism, and no new dependency | `tests/cli.rs`, `tests/nest.rs`, `tests/nest_profile.rs` | two `nest` runs print **and** write identical bytes on a mixed fixture that runs both nesters with its boards authored on the command line, `the_same_input_twice_is_the_same_plan` pins the rectangle packer, and the profile packer's determinism is a stable sort plus `Vec` order with nothing iterating a hash map; `git diff 931812e -- Cargo.toml Cargo.lock crates/*/Cargo.toml` is empty, so no crate and no dependency was added |
| The differential against the milestone's cut point (T5) | `odd/tasks/m9-nesting.md` L11 | **42 invocations per side** run from a `git archive deb8b1a` copy built with its own target directory — two `tools add` and `tools list`, three `import` forms, `validate`/`inspect`/`check`, four `toolpath` kinds, five `emit` runs and their four programs, `simulate` at 2, 3 and 5 mm, a document carrying a `nesting` section, two `nest` runs, the error paths and `--help` — compared by exit code, stdout, stderr and **21** produced files' sha256: **39 of 42 invocations and 19 of 21 files byte-identical**, and the three differences are M9's own (`nest` did not exist — exit `2` `unknown command "nest"` against exit `0` with an 11-line report at `utilisation: 0.080`; `--help` **90 → 108 lines**, 18 added and 0 removed; the `nesting` section preserved where the base drops it on a re-save, and deleting the key makes the payloads identical). The current binary run twice gave 42/42 identical result triples and 21/21 identical hashes, so the comparison is not measuring drift |

Recorded deviations, each a decision in the feature document: **D-M9-2** makes a placement's footprint
the part's turned axis-aligned bounding box, so load-time containment and overlap are conservative for a
free profile — measured `0.200` where the bracket's own area would be `0.100` — and the exact shape is
the nester's business; **D-M9-7** measures the gap between parts only, never insetting a sheet's own
edges, so a full-sheet part sits flush and is certified; **D-M9-8** makes the quarter turn a fallback and
never a tie-break, which is why the cabinetry plan is all `None`; **D-M9-10** deletes the fractions' sum
rule rather than keeping a `pub` variant nothing can produce, because the sum is implied by comparing
each fraction with the derivation; the **free-profile heuristic is incomplete by measurement** and its
strict disjointness costs one search step beyond the gap; there is **no exact NFP** because `Region` has
no union; a run packs **one board size**, and a **mixed document nests each class on boards of its own**,
because neither nester can be handed the other's placements as an occupied board. Two recorded gaps are
not deviations but omissions owned by this document: **no shell drafting form** (S7's assumption, carried
from M4 and M7 — parts and sheets are authored through `cam-cli nest`), and **`docs/format.md` carries no
row for the `nesting` section**, which T6's edit surfaces did not include.

**Both of M9's candidates were reviewed, at tier high, and both receipts are burned; the document-format
commit was not.** `review-1ab521437bba8607` covered `cc2d609..3508eb9` — 2 801 lines, three lenses,
correction budget 200 — after the **full-range** candidate was refused in preflight with
`lens_context_budget_exceeded` (`mutation_outcome: not_started`, no authority created, nothing abandoned),
the same wall M7 hit; the reduced range was accepted, which is the fact worth keeping: the budget is a
property of the candidate's size and the base ref is the lever. Its refuter returned
`correction_required` with three CRITICAL findings that were **one** defect — inside-ness accepting
`Touching`, with `certifies` repeating it, so a plan could certify a part outside a non-rectangular board
— fixed in `033a291` with **161** of the 200 authorised lines, three REDs observed before any `src/` edit,
no existing test changed, and a differential of the 11 pre-existing profile tests against `3508eb9`
(11 passed both sides). It also produced four advisory findings tagged by lineage
(`1ab52143/R1-STEP-DOS`, `R2-002`, `R2-003`, `R3-002`) and named a gap now closed: every new placement
test asserted a refusal, so the fix's **accept** branch had no positive case — `03c4790` pins a flush part
placed and certified on a rectangle and on an L's arm and one inset step past the edge refused.
`review-33ff9cdba8c8746d` covered `90d6a1f..55e759e` — 375 lines, lenses risk + reliability, budget 188
of which **12** were used — and its one finding, `R3-001`, was a **BLOCKER whose premise was false and
whose substance was true**: it held that two 600 × 400 instances cannot share a 1000 × 600 sheet so
`nest` must exit `7`, when in fact `nest` opens a second sheet — invisible to the reviewer because
`main.rs` lay outside the candidate's paths and only the tests were in the diff. The correction
(`e3cd977`, 12 lines) makes the fixture fit one sheet and adds `assert_eq!(after_first.sheets.len(), 1)`,
the assertion that would have caught the ambiguity. The honest gap is `cc2d609`: the document format, its
refusals and the one utilisation sit at the **base** of the first candidate, which git excludes, and the
full-range candidate that would have included them was the refused one — once HEAD moved past it that
range could not be re-cut. `90d6a1f`, the `nest` command, lies between the two ranges and is covered by
neither. M9's document contract therefore rests on its 31 model tests, the pre-M9 byte-identical proof
and the differential's finding that the section survives a re-save by a pre-existing command, and the
command on its 16 CLI tests; that is weaker evidence than a lineage on the *format*, and it is recorded
rather than softened — the differential above is what says the rest of the surface did not move.

### 4.13 M10 delivered (evidence)

M10 adds no machining at all: it is the milestone that answers how a person *drives* the document, and
the owner answered it four times (S1–S4, verbatim): "DSL de comandos, sin motor"; "Lectura y mutación
del documento"; "Import y export"; "CLI más panel mínimo". So the scripting surface is a **command
language** and not an engine — no Lua, no Rhai, no embedding — a script **reads and mutates one
document** and nothing else, the CSV of parts goes **both ways**, and the shell's part is one panel that
calls the same code rather than a second implementation.

What it cost is small in everything except lines moved. It changes **no post contract** and emits no
move. It bumps **no schema**: no file under `crates/cam-model` or `crates/cam-store` differs across
`955fc54..ba91f5e`, so `DOCUMENT_SCHEMA` stays `c2d/document@2` and a document written before M10 is
read and written by the same code — the differential's six documents hash identically on both sides. It
adds **no external dependency**: `Cargo.lock` holds **482** packages before and after, and its only
movement is one line, `+ "cam-cli"` in `cam-ui`'s dependency list, because the shell now calls the
library. The milestone is **eight commits, eighteen files, 8 657 insertions and 3 037 deletions**, and
the deletions are the point: 3 294 lines left `crates/cam-cli/src/main.rs` for `src/lib.rs` in `6cc3b2d`
and were **not rewritten**. Normalising the new `lib.rs` — strip the `pub ` prefixes, drop `fn main` and
its one import — gives a file **byte-identical** to the old `main.rs`: 3 294 lines against 3 294, an
empty diff. That is **D-M10-1**: the crate named `cam-cli` is also the home of the operations the shell
calls, which reads oddly and is deliberate, because the alternative was moving thousands of reviewed
lines between crates to earn a better name.

The law the milestone is built on is that **the script is a caller, not a second parser**, and the parser
was its cheap half. Every subcommand's logic lived inside its argument-parsing arm, so it was extracted
first — `validate_report`, `nest_args` + `nest_document`, `toolpath_args` + `append_toolpath`, `emit_args`
+ `emit_document`, `simulate_args` + `simulate_document` — and the subcommand's arm and the script's
`Command` entry now hold the same two names. A flag therefore means one thing in both places, and the
parity is measured in bytes rather than promised: a scripted `nest` writes the document `cam-cli nest`
writes, a scripted `toolpath` appends the same toolpath and ids, a scripted `emit` the same program, a
scripted `simulate` the same report and exit, and a script's captured buffer is byte-identical to the
binary's stdout for the same script — asserted for a run that ends well and for one that ends in a
finding, with non-vacuity needles so an empty buffer cannot pass. `parts` produces 63 bytes of stdout and
a 534-byte document through either route, `cutlist` 321 bytes.

Diagnostics are located in `cam-post`'s shape, `line {line}:{column}: {message}`, the column being the
1-based byte column of the offending token, and the message is the **subcommand's own**:
`line 4:6: --gap must be a non-negative distance in mm, got -1` is `nest`'s sentence at `--gap`'s column,
because a flag-taking command hands its tokens to the shared parser and then locates the answer by the
token the message quotes or the flag it names. Every mutation passes **one** gate — the command runs on a
clone, `Document::validate` is asked, and only an accepted clone replaces the document — so a refusal
carries the script line *and* the model's words (`line 4:1: nesting part "P1" selects entity "E1", which
is an open contour…`) and the file on disk is untouched until `save`, which validates again as it writes.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The command language: a six-line grammar, eleven words, located diagnostics and one mutation gate | `crates/cam-cli/src/script.rs` (`COMMANDS`, `lex`, `Session::mutate`) | 19 tests in `tests/script.rs`; a bad word is refused **with the vocabulary in the message** and at `line 2:1`, a bad argument count at the column past the last token, a bad value at the offending token's column (`line 4:6` for `--gap -1`), a shared parser's refusal located by its quoted token or named flag, a model refusal carrying the script line, a document that would not validate leaving the file byte-identical, comments and blank lines changing nothing, a path coming from the script or the command line and nowhere else, and `--help` listing the command and all eleven words |
| The extraction that makes a script a caller: five operations, two callers each, parity in bytes | `crates/cam-cli/src/lib.rs`, `tests/script.rs`, `tests/pieces.rs` | a scripted `nest`/`toolpath`/`emit`/`simulate` produces the subcommand's document, report, program and exit code byte for byte; the captured sink equals the binary's stdout for a successful and a failing script; the 157 pre-existing `cli.rs` tests were untouched and green through the extraction, and a differential against the base commit over `validate`, `inspect`, `check`, `tools add`, `tools list`, `toolpath`, `nest`, `emit` and `simulate` shows identical stdout, stderr, exit codes and byte-identical documents and library — the only differences being the new `SCRIPTS` help text and `script`'s own "no such file" where the base said `unknown command` |
| CSV parts **in**: four columns, a required header in a fixed order, and refusals that name the line | `crates/cam-import/src/csv.rs` (`COLUMNS`, `import`, `PartRow`) | 13 tests in `tests/csv.rs`; the header must name `name,width,height,quantity` in that order, so a **swapped width and height refuses at its own line and field instead of rotating every part**; quoting is refused by name; a duplicate name is refused naming **both** lines rather than summed, because `quantity` is already a column; blank lines and `#` comments are skipped anywhere while `part#1` stays a legal name; a row with the wrong field count, a dimension that is not positive and finite, a quantity that is not a positive integer, an empty name, an empty list and a non-UTF-8 byte each name their line, field, column and value; a BOM and CRLF endings are tolerated |
| Parts authored into the document and the cut list **out**, two formats and not one schema (S3) | `crates/cam-cli/src/pieces.rs` (`author_parts`, `cut_list_document`, `save_report`) | 10 tests in `tests/pieces.rs`; a part list authors one closed rectangle and one `NestingPart` per row, its `name` becoming the entity id because that is the only part name the model can hold, in one new layer named `--layer` or `Parts`, with `instances` from `quantity`; the cut list is `part,sheet,x,y,rotation,width,height` at **three decimals**, rotation in counter-clockwise degrees, one header row, one row per placed copy and one trailing newline; a duplicate name and a name the document already holds are each refused **at the row's line**; an import that would not validate is refused with the model's message *and* the CSV line; a document with no nesting section and one with parts but no placements are refused by the state they are in rather than written empty; both commands are reachable through the script and write the subcommand's exact bytes, and two runs of the same import and of the same cut list are byte-identical |
| The HTML print sheet, rendered by hand (S5) | `crates/cam-cli/src/printsheet.rs` (`print_sheet`, `escape_html`) | 8 tests in `tests/printsheet.rs`; the **whole sheet is an inline golden**, two runs are byte-identical, `--out` and stdout carry the same bytes, a hostile layer name and entity id (`<b>x</b>`, `</script>`) cannot inject markup because `&`, `<`, `>`, `"` and `'` are all escaped, a document with no toolpath still renders and says so, the nesting section prints the utilisation `Document::nesting_utilisation` derives, a document that does not validate is refused by name before a byte is rendered, and `--help` lists the command. Measured on a real document: **59 lines, 2 349 bytes**, the same sha256 on two runs |
| The library plus a 16-line binary, so the shell calls the same code (S4, D-M10-1) | `crates/cam-cli/src/lib.rs`, `src/main.rs`, `Cargo.toml` | the normalised `lib.rs` is **byte-identical** to the pre-split `main.rs` (3 294 lines against 3 294; the only deltas are 145 `pub` insertions, three `mod` → `pub mod`, one dropped import and the moved `fn main`); `Cargo.toml` did not change because cargo detects both targets; `cargo test -p cam-cli` **192** stayed 192 across the move, the binary still builds, `--help` exits 0, `postp validate` exits 0, and an unknown command still exits 2 |
| One exit for a report — a sink — and the minimal panel on top of it | `crates/cam-cli/src/script.rs` (`run_script_to`), `crates/cam-ui/src/script_panel.rs` | the engine has one exit, `run_script_to(&Path, &mut dyn Write, &mut dyn Write)`: reports to the first sink, the `save` notice to the second, every report through `crate::write_report`, and `cam-cli script` passing stdout and stderr so the binary's own split is preserved; two bypasses found before the panel was written are closed — a scripted `cutlist` without `--out` went straight to stdout and `save` used `eprintln!`, now one `pieces::save_report` shared with `parts_command`. `ScriptPanel::run` touches no egui and calls the library (never a process), `ui()` is the only function needing a frame, and four headless frame tests pin the four observable rules: the report painted, the located `line L:C` error painted, a failed run keeping the previous result, and the cut-list-plus-`wrote …` capture |
| No schema bump, no new external dependency | `Cargo.lock`, `Cargo.toml`, `crates/cam-model`, `crates/cam-store` | `git diff --name-only 955fc54..HEAD` names no file under `crates/cam-model` or `crates/cam-store`, so the document format is untouched and `DOCUMENT_SCHEMA` stays `c2d/document@2`; `Cargo.lock` holds **482** packages before and after and moves **one line** (`+ "cam-cli"` in `cam-ui`'s list); no crate was added. The milestone's own invariant 6 was **wrong** as first written — "the lock must not move" is unsatisfiable for an edge between two workspace crates, because the lock records exactly that list — so it was amended to "no new external dependency, and the package count does not move", which is what it always meant |
| The differential against the milestone's cut point (T5) | `odd/tasks/m10-documents.md` L7 | **31 invocations per side** from a `git archive 955fc54` copy built with its own `CARGO_TARGET_DIR`, the current binary first run twice in two directories so the harness was proven deterministic before it judged anything (both `diff -r` clean over all 31 steps), comparing **323 comparison points per side** — exit code, stdout, stderr and the sha256 of every file left behind, six documents taken twice as zip bytes and as `document.json`, the programs and the library. **The only difference is `--help`**: 108 lines → 152, **44 added and none removed** (`parts`, `cutlist`, `report`, `script` and the `SCRIPTS:` block); everything else is identical, exit codes included (21×0, 4×2, 1×4, 4×6, 1×7 on each side) and **no file hash differs anywhere** |
| The totals, measured | workspace | **1 506 tests, 0 failed**, in 110 `test result` lines (96 test binaries plus 14 doc-test suites, which hold none) against 1 452 at M9: `cam-cli` **194** (was 157), `cam-import` **136** (was 123), `cam-ui` **268** (was 264), and 54 of the new tests in the five new binaries — `script` 19, `pieces` 10, `printsheet` 8, `csv` 13, `script_panel` 4 |

**The review record is uneven, and it is recorded that way.** One lineage covers the language and the
CSV: **`review-bd99706da5d0c7f1`** took **3 800 lines**, tier high, three lenses (risk, readability,
reliability), correction budget 200, and ended **approved** with the receipt **burned** — it covers T1,
T2a, T2b and the `d76b26e` correction, and opened no correction of its own. Its predecessor
**`review-61e0b405dc7f498b`** was **superseded**, not approved: its refuter returned three findings that
were one defect (`R1-001` risk, `R2-001` readability, `R3-001` reliability) — `command_nest` parsed a
positional document and handed `None` to `refuse_document_destination`, so `open a` + `nest b` nested
**a** and a following `save b` overwrote **b** with a's document — fixed in `d76b26e` (**36** of the 200
authorised lines, one line of code plus the test) and then superseded because T2b was committed while it
was open: the frozen scope grew from **7 paths to 9**, the provider returned `recover` /
`scope_changed`, and the **owner authorised** the recover. The approved lineage emitted **nine advisory
findings**, all informational and none corrected (`bd99706d/R2-001` … `R3-003`, listed with their lenses,
severities and line locations in [docs/m10-documents.md](../docs/m10-documents.md#the-review-record-and-what-it-does-not-cover)).

**What is not reviewed is stated as measured.** T3 (the print sheet), T3b (the crate split) and T4a (the
sink) were **not** reviewed: `assess` on `d76b26e..HEAD` measured `reviewDue: false` with
`reviewDueReason: under_budget` at **7 774 changed lines**, and again after T4a at **8 015**, both at risk
`medium` — that measurement is the record, and no disposition was invented on top of it. The print sheet
therefore rests on its 8 tests and its whole-sheet golden, the split on the mechanical byte-identical
proof and the 194 `cam-cli` tests the move did not touch, and the sink on its byte-parity tests and a
differential against the base that reports zero differences in stdout, stderr, exit code and produced
files. T4b was cut as its own candidate for the same reason M9 cut its geometry slice: the accumulated
range measures `reviewDue: true` (`high_risk`, `process_boundary` on `crates/cam-ui/src/lib.rs`) at
**8 770 lines over 14 paths**, past the reviewer budget, while T4b alone is **781 lines** on
`ff1b526..ba91f5e` — that lineage, `review-12219c3f04a57822`, tier high with the risk and reliability
lenses, returned `correction_required` on one **CRITICAL** finding: `ScriptPanel::run` decided by
`!text.is_empty()` instead of by the result, so a successful script that printed nothing left the
previous run's text on screen and a failure that had written part of a report overwrote it. The four
frame tests could not see it because they covered a non-empty success and an empty failure and nothing
between; the correction decides by the result, and its receipt was not burned when this record was
written.

Recorded deviations, each a decision in the feature document: **D-M10-1** makes the crate named
`cam-cli` the operations home the shell calls, rather than moving reviewed lines into a better-named
`cam-script` crate; **invariant 6** was amended from "`Cargo.lock` must not move" to "no new external
dependency and a package count that does not move" once a workspace edge made the original wording
unsatisfiable — the invariant was wrong, not the change (L6); the language is **eleven words and not
every subcommand**, so `inspect`, `check`, `import`, `postp validate`, `tools` and `report` are refused
by the vocabulary itself; a scripted command takes **no document path and no `--out`**, both refused by
name because a destination that was accepted and ignored still looks applied; `sheet` and `part` have
**no CLI counterpart**, so the model's `validate` is their only gate; a located error from a shared
parser is reconstructed from its message (quoted token, then named flag, then the command word) and
falls back to the word when a message names neither; a **refused write is now a refusal and not a
panic** — `write_report` maps a failed `write_all` to a named failure where `print!` used to panic, so a
report into a broken pipe exits `2` instead of aborting, and no test provokes `EPIPE`; the panel was a
**standalone module** with no shell tab in M10 and **is mounted now** — `Command::Script` opens it as a
modal over the canvas (`odd/tasks/script-panel-mount.md`) — and it runs **synchronously** on the frame
that pressed `Run`; the sheet **omits** the
program, the simulation and the file name and says so under its own `Omitted` heading; the two CSV
shapes are **two formats, not one schema**, so `cam_import::csv::import` deliberately does not read a
cut list back; and **`docs/format.md`'s command-line table is stale** from M9 onwards — it lists neither
`nest` nor M10's four commands and its exit table stops at `6` while `nest` exits `7` — recorded rather
than patched, because M10 changed no document format and the format document's own rows were not part
of this milestone's contribution. **Patched after M12**, when the audit went to fix it and found the
staleness itself stale: the table already carried all fifteen commands and the whole exit set, because
the milestones that added them filled their own rows. What was genuinely missing is what the patch
adds — `import`'s six trace and tolerance flags, and `tools add`'s `--name-format`.

### 4.14 M11 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| `Job.material`, optional and omitted when absent, `Copy` lost and every `..Job::default_sheet()` site moved with it, the tag still `c2d/document@2` | `crates/cam-model` | a document written before M11 loads, keeps its fields and re-saves byte-identically; the round-trip and frame tests moved in the same commit rather than being relaxed |
| The selector threaded to the lookup the store already had: `program_for`, `program_for_snapshot` and `program_tool` take `Option<&str>`, and `None` is the pre-M11 program | `crates/cam-program` | all 17 golden programs keep `F100.0` and `S18000`; the library's per-material keying was already proved before the milestone (`crates/cam-store/tests/tools.rs`: one tool at 120 mm/s in one material and 60 in another) |
| `cam-cli emit --material`, the flag winning over the document's field, and an unknown material refused by name rather than silently cut at the tool's own rate | `crates/cam-cli` | `emit_cuts_as_the_material_it_is_asked_for` pins the four cases (no selector → `F100.0`; flag `mdf` → `F120.0` and no `F100.0`; document `mdf` → `F120.0`; flag `hardwood` over a document that says `mdf` → `F60.0`); `emit_refuses_a_material_the_library_has_no_record_for` asserts `oak`, exit `2` and the tool's name; 21 golden tests pass untouched |
| One function answers "which material" for both `emit` and `simulate`, so the two commands cannot ask the document different questions | `selected_material` in `crates/cam-cli/src/lib.rs` | the same selector is the only caller path for both commands |
| The review | lineage **`review-c54c930ddcde3f9f`** | tier `high`, 667 lines over 26 paths, four lenses, `correction_required` on five corroborated findings, one bounded correction of **79 diff lines** (budget 200), a targeted validator that passed quoting the frozen findings one by one, closure **`approved`**, receipt burned (`sha256:93d41308…`) |

Two defects were found and fixed at the root, both of them mine. The flag's parser used
`rest.get(index + 1).cloned()`, which made a **trailing** `--material` indistinguishable from *no* selector:
`emit` exited `0` and cut at the rate the operator had explicitly asked not to use, and a following flag was
swallowed as the name. It now refuses both shapes — a missing value and a value starting with `-` — with
exit `2` and "emit --material needs a material name". The second is the more instructive: the correction had
also made the **no-selector** path prefer the library's null-material row, which changes what an existing
library shape emits (`F60.0` → `F100.0`) and contradicts S3; it was reverted, and the comment says what the
code does instead of claiming a byte-identity it did not deliver. Both were invisible to a green gate, and
the review caught them by reading the spec the tests did not.

Recorded and not decided by this milestone, each a successor finding rather than a silent gap: `R4-002` and
`R3-003` — the shell's cutting-time estimate passes no material, so a document that names one emits with
that material's rates while the window estimates from the tool's own record; the snapshot needs a material
path `cam-store` can answer, which is a change to that crate. `R4-003` — `simulate` has no `--material`, so
after `emit --material <name>` the simulation rebuilds the program for the document's material and cannot
reproduce the emitted program. And a measured limit carried in the feature document: **no shipped post block
references `[FP]`**, so a plunge rate cannot be seen through the emitter at all; it is pinned at the
`cam-program` level, and changing a post contract to make it visible is not this milestone's business.

### 4.15 M12 delivered (evidence)

| Deliverable | Where | Proof |
| --- | --- | --- |
| A `cam-export` crate mirroring the reader's shape: bytes in millimetres out, no filesystem, no document mutation, and the same document always exporting to the same bytes | `crates/cam-export` | the crate writes a file the caller owns, keeps every bulge verbatim so the arc the kernel holds is the arc the file carries, and its identity is `(Format, error, dispatch)` beside `cam-import`'s |
| **DXF**: one `LWPOLYLINE` per contour, each vertex's bulge verbatim, `70 = 1` when closed, `$INSUNITS = 4`, the layer table in document order | `crates/cam-export/src/dxf.rs` | `crates/cam-export/tests/dxf.rs`: layer identity survives, a closed contour closes, the unit header is written |
| **SVG**: one `<path>` per contour with `A` commands whose `rx == ry`, the Y reflection the reader inverts, a `viewBox` in the document's own millimetres | `crates/cam-export/src/svg.rs` | `crates/cam-export/tests/svg.rs`: a circle stays two arcs and is not doubled by the explicit closing `A`; the documented loss — SVG carries no layers — is asserted on a two-layer document rather than omitted |
| The acceptance test is the **round trip against our own reader**, never byte equality: exported and imported again, the same shape and the same layer order, compared up to the 10 mm placement the reader performs | `crates/cam-export/tests/round_trip.rs` | both sides framed by **one origin for the whole side**, the comparators taking slices rather than pairs, plus vertex and segment counts per contour so a phantom zero-length segment cannot pass |
| `cam-cli export <document> --format dxf\|svg [--out]`, writing and announcing with `--out` and printing to stdout without it | `crates/cam-cli` | a format this build does not write is refused by name (`export writes dxf or svg, not "pdf"`, exit `2`) and a missing `--format` is refused rather than defaulted |
| The review | lineage **`review-a7e624d841d3d0c6`** | tier `high`, 2156 lines over 15 paths, four lenses, closure **`approved` on the first admitted event** with no correction, receipt burned (`sha256:193e1fd6…`); nine findings, all informational by the provider's own statement, and seven of them real defects |

The seven fixes are worth naming because each was small and one was destructive: `--out` canonicalising to
the document being read **replaced a document with DXF and reported success** (now refused, with the document
asserted byte-identical afterwards); `fs::write` truncating before it completes (now a sibling temporary
renamed into place); `print!` panicking on a closed pipe, so `export | head` exited `101` with a backtrace
(now `write_all` on the locked stdout with the error mapped — and the closed-pipe case is the one thing this
repository's harness cannot reproduce, which is recorded rather than claimed); a round-trip comparator that
normalised **each contour to its own bounding box**, so a writer shifting one contour relative to another
would still pass and S4 was not proved at all; a point-set comparison blind to a phantom zero-length segment;
`export`'s doc claiming the writer "refuses nothing the model allows", which hid a real refusal surface; and
the SVG module's identity omitting `min.y`, which made the sentence explaining the Y reflection false for any
page not anchored at zero.

What the review did **not** find is as telling as what it did: neither writer lost an arc, a layer order or a
closed contour, and the round trip held through both formats on the first run — the byte-level tests the two
worker units were built against did their job. **Toolpaths are not written**: a toolpath is not geometry, and
writing one would make "the same thing that went in" unprovable.

## 5. Verification strategy

Per ODD, change behaviour with applicable runnable checks, test-first:

1. **Golden-file G-code tests** — the primary regression harness. `cam-cli` on a fixture document
   must emit byte-identical output; every strategy and every output block gets a golden file.
2. **Property tests** (`proptest`) on the geometry kernel — invariants that catch the classic 2.5D
   bugs: an outward offset of a closed loop never self-intersects below the tool radius; chaining
   never produces duplicate endpoints within tolerance; reversing a polyline preserves winding sign.
3. **Round-trip tests** — document → JSON → document is lossless; SQLite tool library migrates
   forward and never loses a tool.
4. **Post-processor conformance** — a fixture set covering all 13 directives and all 20 blocks (the
   four helical kinds M6 added among them), plus a parser that rejects malformed files with a located
   diagnostic (`line:column`, expected token).
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
| **The simulation's field and the program used different origins** (M6 L17) — **closed in M7, T0, commits `786c3d8` + `228ecc7`.** `cam_sim::grid::grid_for` built the field at `(0, 0)` from `job.width/height` while programs are emitted in machine coordinates, so at any origin corner other than bottom-left the report and the preview disagreed with the document about *where* the cut is, for every strategy since M5 | Before: a correct program on an 80 x 30 mm band in a 200 x 100 mm sheet reported `gouge 14376.000 mm³ over 2396 cells` and `uncut 14400.000 mm³ over 2400 cells` at `top_left`, against `gouge 0.000` / `uncut 24.000 mm³ over 4 cells` at bottom-left. After: **the bottom-left numbers at all three corners** (`removed 14376.000`, `gouge 0.000`, `uncut 24.000 over 4`). The emitted G-code was never affected; the report and the 3D preview were | Closed as the roadmap's own recorded next step, in one slice: one `cam_model::Frame` honoured at every boundary — the field's grid and cell mapping, the program's coordinates, and the **handedness of every arc and helix** — with a **test per origin corner** and 13 tests in all. D-M7-8 records the crossing point the implementation found: the field, the target and the preview stay in the document frame and the program is read into it once, because M5 places a tab by arc length from the contour's start, so reflecting the target would move every tab at `top_left` |

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
| D-M6-12 | The sweep **interpolates Z** along a cutting move, so a ramp is one linear move and not a staircase (amends D-M5-3) | M6, T4b | D-M5-3 removed a move's *destination* depth over its whole travel, which was conservative while every cutting move held one depth and is simply wrong for a ramp. T4's first implementation worked around it by emitting the ramp as 0.1 mm Z steps, which puts the simulator's limitation into the program the machine runs and cannot express a helix. The sampler now takes `max(2·travel/cell, 16·|Δz|/cell)` steps, so a graded floor is resolved to half the judgement; a constant-Z move's count is unchanged and its results are bit-identical (33,600 of 33,600 cell values against `a4487f0`) |
| D-M6-14 | A thread's target is the **spiral**, not the annulus | M6, T7 | A helical descent leaves the material between revolutions standing, and that material *is* the thread. Authorising the whole annulus at `cut_depth` is an upper bound of up to one pitch, and it made `simulate` exit `6` on **every** correct thread whose pitch is coarser than `cell/8` — measured as `uncut 106.452 mm³ over 151 cells`, i.e. a gate that fails every real thread. The claim is the floor itself per cell (the deepest pass covering it), taken as the maximum over the tool's angular window because a cell is reached from angles up to `α(r)` later than its own; the tip-only form reports a correct thread as `gouge 8.697 mm³`, the same one-sided error `claim_flute` was amended for |
| D-M7-1 | M7 is V-carve **to the point**: no relief pass, no flat bottom | M7, T7 | The owner's two scope decisions (S2). Corner relief is not needed for the carve to meet the vectors — at a boundary `d = 0`, so the surface tapers to zero depth exactly at the edge, sharp corners included — and what a machine leaves in a corner is the tip's own finite radius, which `simulate` reports as `uncut`. Flat Depth / `DoFlatBottom` / `UseAreClearTool` change what the operation *is*, and the milestone's risk is the skeleton |
| D-M7-2 | The medial axis is the toolpath, and the **empty-ball test** is the only thing that certifies it | M7, T2 | Candidate generation is cheap and unreliable, the certificate is exact and cheap: generate, verify each candidate with the kernel's exact distance, keep only certified points, chain them, and drop whatever fails **whole** — never nudged, because a nudged tip is a lie about its own clearance. The nearest set is deduped **by point**, or every corner of every polygon would certify itself |
| D-M7-3 | `start_depth` is deferred, so the law stays exact | M7, T5 | With `start_depth > 0` the cone's top reaches `s·tan(θ/2)` **outside** the vectors, so the authorised surface is no longer `−d/tan(θ/2)` inside the region alone and the milestone's cleanest assertion — `gouge` exactly zero — would be false for a legitimate program. `--start-depth` is refused by name rather than stored as a field that defaults to zero |
| D-M7-4 | Arcs stay arcs: nothing is tessellated, and no chord tolerance exists anywhere — **superseded in its premise by T2** | M7, T2 | As written this decision said arcs would be tessellated for the skeleton, reasoning that every bisector involving an arc is a conic the kernel has no type for. That is right about *constructing* a bisector and wrong about *needing* one: the axis is found by bisecting the **scalar** `dist(p,i) − dist(p,j)` along a lattice edge and certifying with exact nearest points, so no bisector is built and the arc's own geometry is asked throughout. The corrected decision is the one in force; the original reasoning is kept because it is exactly why the alternative looks necessary |
| D-M7-5 | The tolerance is the **lattice step**, and its four costs are declared — **the "no tolerance flag" half superseded by implementation** | M7, T2, T5 | The same *kind* of knob the product calls `VCarve Toolpath Tolerance`, but not the same knob: the product simplifies a computed path afterwards, M7 refines the axis to a step. This decision first said "M7 adds no tolerance flag and takes the job's existing tolerance"; the implementation made it a stored, defaulted `--tolerance` (0.1 mm), because `emit` recomputes the axis from the document alone (S9, S10) and a flag accepted and then dropped would make the program depend on a constant nobody named. The four declared costs, measured: the chording between certified vertices; a piece shorter than the step missed; the convex-corner band (`0.20` mm of missed depth at 0.5 against `0.00` at 0.1); and a curvature extremum truncating a branch (the 5 x 2 ellipse's axis reaches `±3.5` against its analytic `±4.2` at 0.5, `±4.8` at 0.1) |
| D-M7-6 | T0 — the origin fix — is a prerequisite, not a detour | M7, T0 | M7's acceptance test is a simulation report (S11), and while `grid_for` and the sweep disagree about the origin at three of four corners an oracle can only be written at the one corner where the bug is invisible — which is precisely how it survived M5 and M6. The fix is the roadmap's own recorded next step (§6) |
| D-M7-7 | `Move::Arc` is not extended and no helical block is reused | M7, T4 | A V-carve's constant-`d` contour is not a constant-Z curve, so an arc move cannot carry it, and `Move::Helix` is a descent at a constant radius, not a bisector. The walk emits linear feeds with per-vertex Z, which the existing post blocks and the D-M6-12 sweep already handle (S12, S13). Chording the level sets into arc blocks would trade the law's exactness for a shorter file, and M7 has no evidence that file length matters at this scale |
| D-M7-8 | The frame is mapped where a program meets the field, not by moving the grid | M7, T0 | The §6 sketch proposed moving `grid_for`'s origin and the sweep's cell mapping together. The implementation found a cheaper and safer crossing point, and the reason is a tab: M5 places a tab by **arc length from the contour's start**, and reflecting a contour reverses its traversal, so mapping the *target's* geometry into the machine frame would move every tab to the other side at `top_left`. The field, the target and the preview stay in the document frame, and the program is read into it once, in `sweep::apply_move` |

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
- **L5 (after M10, two features and no milestone).** M0–M10 were the whole plan, so the work that followed
  started from the debt the milestones left rather than from a plan row. **The advisory debt**
  (`odd/tasks/debt-advisories.md`): six advisories closed — four with the reviewer's own words recovered
  from a sibling lineage, plus `1ab52143/R1-STEP-DOS` (the search step is now bounded by
  `MAX_SEARCH_ANCHORS`, four million, the same ceiling the simulator's cell grid and the medial axis's
  lattice use) and `R3-CAP-BOUNDARY` (the cap's inclusive edge, proved by installing the mutated
  comparison) — ten unrecoverable because the review store keeps a receipt's lineage and not its findings.
  **The script panel's mount** (`odd/tasks/script-panel-mount.md`): `ScriptPanel` is drawn as S9's modal
  from a palette command, `Esc` closes it, and a test pins that the frame's canvas is byte-identical with
  it open and closed — the recorded claim that the wiring "would move every existing shell golden" was
  measured and is false, since this crate keeps no pixel goldens at all. Six advisories were carried in
  and four were answered: three by the fixes above and one — a **BLOCKER filed against
  `egui::Id::unique`** by the mount's refuter — by contradicting it with evidence, because the constructor
  exists (`egui-0.37.0/src/id.rs:91`) and the spelling it demanded is deprecated. That authority was
  **abandoned** rather than left open: `operator_disposition` by the owner, recorded as
  `gentle-ai.review-reclaim-record/v1` on 2026-11-05 and moved to
  `review-transactions/quarantine/review-5eb75a623cbfc72e-441271739`, because no correction could satisfy
  the finding without failing `clippy -D warnings` (`odd/tasks/script-panel-mount.md`, L6-L8).
- **L6 (after M10, the third feature: the panel edits the shell's document).**
  `odd/tasks/script-panel-document.md`, delivered in three commits — `95e2cf1` (the change),
  `ec89684` (its review) and `d95445e` (`R4-001`) — which close the residual the mount left open: a script
  run from the panel used to edit a file on disk and reach the document only when the file was reopened.
  The owner chose the shape in two decisions, both quoted verbatim in that document: the panel runs against
  the shell's document ("El panel edita el documento del shell") and the mutations live in memory, with the
  file written only by the shell's Save ("Sólo memoria; el archivo lo escribe Save"). `run_script_on` takes
  the caller's `&mut Document` and the whole run is one transaction; `open` and `save` are refused on the
  line that tries; the shell commits through `AppState::replace_document_undoable`, and only when the run
  changed something. Evidence: gate green at 1526 tests, `cam-ui` shell 128 and panel 8, `cam-cli` 26, and a
  differential of `cam-cli script` — a command the request never named — over eleven scripts against the
  previous tree with stdout, stderr, exit code and the written document identical. Its native review,
  lineage `review-cf16da5a0b71e7b8` over `f19cf4c..95e2cf1`, closed **approved** on the last admitted event
  with zero of its 200-line correction budget spent; the receipt is burned. Its six advisories are all
  informational — `R4-001` was closed in its own candidate once its anchor reproduced the edge — and their
  wording did not survive the store, which is the blindness L5 above already names, one review later.
- **L7 (the blindness, measured and answered).** Four losses of the same kind — M9's, M10's, the debt
  review's own `R3-001` and L6's six — stopped being an anecdote and became a document:
  `odd/reports/review-findings-survival.md`. It measures what each contract leaves behind in
  `.git/gentle-ai` — a `v2` lineage keeps a 4.6–23.9 KB `review-state.json` with the admitted artifacts and
  their prose inside, a `compact-v2` lineage keeps a **259-byte** terminal-consumption record and nothing
  else, rejected results are the only ones written to disk, and an abandoned authority keeps more than an
  approved one. It also records the three changes that would end the loss — as a record, not a report: this
  repository is not a fork of that tooling and has no upstream to file against. The second half is binding
  here:
  findings are written down in the session that receives them **before** the receipt is burned, an inferred
  subject is marked as an inference, no finding is closed by a plausible-looking edit, and the quarantine
  residue is read before any finding is declared unreadable.

### 4.16 M13 delivered (evidence)

The four gaps M13 named were all in the **seams**, not in the machinery, which is why they survived twelve
milestones of green gates. Measured before writing anything: the primary action could not run
(`crates/cam-ui/src/chrome.rs:323` registers `Generate G-code` as `add_enabled(false, …)`), the 3D half would
not simulate (`crates/cam-ui/src/view3d.rs:1390` prints a sentence naming `cam-cli`), the shell had no
toolpath selection at all (`chrome.rs:850-855` described `toolpaths.first()` unconditionally), and the disused
`Document.postp` ("resolved by the store", `cam-model/src/lib.rs:909-911`) has no reader in the workspace, so
nothing in the app could name the dialect a program is written in.

What was delivered, in four commits: `0c194cb` (the program, and the `Post…` control that supplies the
dialect), `89ed3b3` (the slice boundary, recorded rather than inferred), `c4da057` (simulation in the shell,
with `Simulate` on the empty half and `Simulate again` in the overlay row) and `a078015` (the toolpath list,
the one resolver the card, the editor and the tool-library modal now share, and the stale-`first()` door that
slice closed). Ten behaviour tests, gate green at **1604** workspace tests across 117 targets.

Three limits are recorded instead of discovered later:

- **A snapshot cannot select a material.** `program_for_snapshot` refuses one by name
  (`crates/cam-program/src/lib.rs:95-113`), so the shell's program cuts at the tools' own rates and diverges
  from `cam-cli emit --material` exactly as the shell's cut-time estimate already does (M11's follow-up).
  Passing the document's material would have made every drawing that names one refuse to produce a program;
  a test pins that a named material still writes.
- **Two sentences still name `cam-cli`**, both in `chrome.rs` and both drawn only while the RELIEF AND 3D
  task is open (`chrome.rs:890`, `:911`). They are outside every slice's surfaces and need their own task.
- **The launch demonstrator stays.** `cam-desktop` still hands the shell a sample pocket, because the sample
  document holds no toolpaths of its own; its test pins M5's field/sweep identity, and replacing it is a
  separate decision about what the first frame should demonstrate.

`T4` became a decision rather than a fix: the 3D stock **already** paints from the design system's roles
(`theme.rs:221`'s `stock_shade` mixing `StockTop` into `StockBottom`, `view3d.rs:963`'s `StockEdge` outline),
and the near-white slab in the owner's screenshot is `StockTop #d6dce6` itself — the reference app's tan is
its own palette. Changing ours would edit `design/system.md`, both palettes and the four contrast proofs that
keep twelve pairs above AA, so it waits for the owner and the milestone says so rather than closing it.
