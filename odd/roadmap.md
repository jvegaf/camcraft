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
| **M3** | Import & interchange | DXF, SVG, PDF, EPS, raster trace; validation report of open/self-intersecting vectors | Users bring geometry; the original's own filter list is the target set |
| **M4** | Core toolpaths | Profile (inside/outside/on, tabs, passes, lead-in/out), Pocket (raster + offset, allowance, stepover, clear stepover), Drilling (peck, retract, drill bank) | The three strategies that define 2.5D routing; they exercise every post-processor block |
| **M5** | Simulation & 3D preview | Heightfield material removal, gouge/uncut reporting, 3D stock view, animated preview | G-code without simulation is unverifiable by the user; also the only honest acceptance test for M4–M7 |
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
| Simulation performance on large jobs | UI stalls | Tiled heightfield + `rayon`, resolution decoupled from job size |
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
