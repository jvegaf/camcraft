These sections describe the pre-pivot general-machining product and its crate architecture. `docs/SYSTEM_DESIGN_CAMCRAFT.md` supersedes them, and they are kept only as history.

## 1. Product definition

A cross-platform desktop application for 2.5D CNC routing: draw or import 2D vectors, assign
machining strategies that produce toolpaths with controlled depth passes, simulate material removal,
and emit machine-specific G-code through user-editable post-processors.

Non-goals for this program (explicit):

- Binary or project compatibility with closed proprietary project formats: their internals are an
  unresolved container, so interoperability goes through the interchange formats we do read.
- 3D model machining (STL relief roughing/finishing), relief carving from a grayscale image, sculpting.
- Laser strategies (`Laser Cut - Fill`, `Laser Sketch Engraving`).
- Windows-only APIs: no MFC, no GDI, no Codejock, no `WINSPOOL` printing path.
- Lua gadget (plugin ABI) compatibility with any existing CAM package: a scripting surface is planned, and it will be its own dialect.

## 2. Stack decision and its justification

`Rust` + `wgpu` was chosen. The evidence supports it: every domain subsystem a 2.5D CAM application
needs (tool database, post-processor DSL and store, toolpath math, nesting layout) is plain portable
logic with no Windows dependency, while the *only* genuinely platform-bound parts are the UI shell
(the windowing toolkit and the native widget set behind it) and the 3D viewport (the graphics API and
its driver stack). Replacing those two is exactly what a cross-platform requirement demands.

**UI shell sub-decision (resolved inside D1):** `egui` via `eframe` for M1, with the shell kept
strictly behind an adapter boundary (`cam-ui` depends on `cam-app`; `cam-app` depends on nothing
UI-related). Revisit trigger — switch to `cxx-qt` (Qt 6) if any of these happen: native file-dialog
or OS integration requirements become blocking, docking/table-heavy panels dominate the UI work, or
text input/IME becomes a recurring defect source. Because the domain never imports the shell, that
switch must not touch a single toolpath or post-processor line.

Why the alternatives lose on this evidence:

- **C++20 + Qt 6** is the conventional desktop-CAM stack, and carries its cost: manual memory discipline
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
| Units | Internal geometry is `f64` **millimetres**. Document stores the user's unit preference; rounding happens **only** at post-processor emission using the `VAR ... |decimals` spec | Post-processor variables carry their own decimal spec (`[X|C|X|1.4]` vs `1.3`) and sheet libraries mix metric and imperial; rounding early would compound error across passes |
| Arcs | Arcs are **first-class entities**, not tessellated polylines. Polylines carry nonzero bulges | Native G2/G3 output exists in 8 of the 15 post-processor blocks; flattening at import would make arc output impossible |
| Tessellation | Only at render and simulation time, never in the toolpath pipeline | Keeps emitted geometry equal to authored geometry |
| Tolerance | A single `Tolerance` value threaded through geometry ops, exposed as a job setting | Tolerance is usually a per-toolpath string, so two toolpaths can disagree about the same geometry; one job setting prevents that |
| Tool/toolpath model | Tool geometry, cutting data and machine are **separate** records joined per (material, machine) | A tool's shape is constant while its rate depends on the material and the machine, so one record per concern is the normalization that makes a library worth having |
| Post-processor | Implement a reader for the block/directive dialect machine posts are written in (13 directives, 17 variables, 15 blocks, `[var|A/C|prefix|decimals]`) **plus** our own authoring format | A data-driven emitter is the difference between one machine and seventy-five; a block/directive grammar is what makes a post data rather than code |
| Post-processor licensing | **Do not vendor third-party `.pp` files.** Ship our own fixtures; support an optional user-supplied directory | Those files are third-party content; the *grammar* is what we reimplement |
| Undo/redo | Whole-document snapshots (documents are kilobytes by nature) with a bounded history and coalescing | Command inversion across geometry edits is a defect factory; snapshot + structural sharing is cheap here |
| Parallelism | `rayon` for independent toolpaths, pass carving and simulation tiles | Toolpaths, depth passes and simulation tiles write disjoint outputs, so coarse-grained parallelism is the safe kind here |
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
`schema_version` table using a forward-only migration style: a version row, applied in order, never rewritten in place.

