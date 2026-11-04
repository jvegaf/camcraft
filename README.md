# campilot

A cross-platform (Windows + Linux) 2.5D CAM application for CNC routers, built as an evidence-based
answer to a simple question: *what would the reference CAM application look like if it were designed today, in Rust,
with the domain separated from the window?*

**Status: M9 complete.** The domain runs end to end — a document, a tool from a SQLite library and a
post-processor produce real G-code, byte for byte tested — for **profile**, **pocket** (offset rings
and raster rows, with islands), **drilling** (pecks, both retract modes, drill-bank selection), the six
depth-variant strategies — **chamfer** (a V-bit bevel whose depth is derived from the bit's angle),
**quick engrave**, **fluting** (with a ramped entry and exit), **texture** (parallel grooves at a
spacing and angle, deliberately not a pocket), the **inlay** pair (female cavity and male plug from one
command) and **thread milling** (a helical descent, emitted as a helical arc) — and **V-carve**: the
medial axis of the region walked with the tip at `−d/tan(θ/2)` for each tip's own distance `d` to the
boundary, so the toolpath is a depth **field** rather than a fixed-Z pass, and **moulding**: a rail with
an authored cross-section swept along it, one lateral pass per stepover offset, so the toolpath is a
family of Z heights taken from the profile rather than from the bit. Around it: M1 built the
editor shell, M2 the vector authoring and editing commands, M3 the DXF, SVG, PDF, EPS and raster
importers, M4 the toolpaths, authored headlessly through `cam-cli toolpath`, and M5 the simulation that
checks them — the emitted program swept over a heightfield of the stock, a report of removed, gouge and
uncut material and of every crash, and a 3D preview the shell plays. Every one of the eleven strategies
is measured against that simulation, in both signs.

## What works today

```
dxf/svg/pdf/eps/png ──▶ document model ──▶ toolpath ──▶ move stream ──▶ post-processor ──▶ part.nc
     (cam-import)        (c2d/document@2)   eleven      (Rapid/Feed/     (.pp dialect)
                          (validated)       strategies   Arc/Helix/Retract)
                                                        │
                                                        ▼
                                            cam-sim heightfield ──▶ report · 3D preview
```

- **Arc geometry end to end.** Arcs are first-class from the document through the offset to the
  emitter: a circle cut with a profile toolpath emits `G02`/`G03`, never a polyline.
- **Eleven strategies, one contract.** Profile (inside/outside/on, tabs, passes, lead-in/out), pocket
  (offset rings or raster rows, allowance, stepover, nesting islands) and drilling (peck depth, both
  retract modes, drill-bank selection), then M6's six depth variants: chamfer (derived V-bit depth),
  quick engrave, fluting (ramped), texture (grooves at a spacing and angle, never a clearing pass), the
  inlay pair and thread milling, M7's **V-carve**: the region's medial axis, walked at the depth each
  tip's own distance to the boundary demands, emitted as varying-Z linear feeds because a V-carve's Z is
  a function of position and no arc move can carry it — and M8's **moulding**: an authored cross-section
  swept along a rail at a stepover, one lateral pass per offset, each at the depth its own profile point
  demands. Each is authored by `cam-cli toolpath` and reaches G-code through `cam-cli emit`, so the test
  surface is the program a machine would run.
- **The V-carve's proof is the distance field itself.** A V-bit's body is above its cone, so a tip at
  `−d/tan(θ/2)` can only leave material standing and never cut deeper than the field: `gouge` is
  **exactly zero**, not "within a tolerance", for any tip set inside the region, and a dropped piece of
  axis costs coverage (`uncut`) and nothing else. The simulation authorises `d(centre)/tan(θ/2)` per
  cell from the document alone, and the oracle asserts both signs — a 20 x 20 mm square removes
  `1332.5` of its analytic `1333.333 mm³` and a radius-15 disc `3534.152` of `3534.292`, with zero gouge
  and zero uncut, while a **deliberately thinned axis** reports `uncut` and still zero gouge and a
  **20% too deep** program reports `gouge 629.557 mm³`.
- **The moulding's proof is coverage plus a monotone tool law.** The claim and the program read the same
  pass polyline — `Rail::offset_walk` over the same arc-length walk, closing edge included — the sweep's
  samples lie on that polyline, and a tool's `rise` grows with distance — so a correct moulding can never
  remove more than its authored cross-section authorised: `gouge` is **exactly zero**, and a pass dropped
  or a cross-section authored 1 mm shallower costs `uncut` and nothing else. The simulation authorises the
  cross-section swept along the rail from the document alone, and the oracle asserts both signs on a
  straight rail, a **closed frame** and a V-bit cone (660 and 496 mm³ exact with zero gouge and zero
  uncut; `uncut 300`/`248` 1 mm shallow; `gouge 300`/`248` and `gouge 603.671` 1 mm deep). A closed rail
  whose drawing *starts* at a corner is exact too: the seam's corner diagonal is an edge the program cuts,
  so the claim carries it, and what was measured at delivery as `gouge 2.000 mm³` over one cell is pinned
  closed in [docs/m8-moulding.md](docs/m8-moulding.md#known-limits-each-a-decision).
- **Nesting's proof is a certificate per placement.** `cam-cli nest` packs a document's authored sheets
  and parts and stores the placements and one cost record, and each nester states its promise as one
  predicate instead of restating the arithmetic. Rectangles go through **maxrects**, with a **guillotine**
  pass that is not decoration: on a 234 × 173 mm board at gap 8 it fits three parts on **one** sheet
  where maxrects needs **two** (`0.5067` against `0.2534` utilisation), and both plans are certified.
  Everything that is not a rectangle goes through a free-profile **no-fit heuristic** that is sound — a
  placement is refused unless it is disjoint from every part already on the board **and** from every part
  grown by the gap — and, measured, incomplete: a 50 × 50 square on a 100 × 100 board at step 20 opens a
  second board although `(45, 0)` is legal, and step 5 finds it. The one arithmetic is
  `Document::nesting_utilisation` — placed footprint over the sheets the placements **name**, never every
  authored sheet — and the report, the stored field and load-time validation all read it, so a
  hand-written `0.25` where the placements derive `0.125` is refused naming both numbers. A nesting that
  cannot place every instance exits `7` and writes nothing, because a partial nesting has no honest
  stored form.
- **A simulation, and every strategy measured against it.** `cam-cli simulate` sweeps the emitted
  program over a heightfield of the stock and reports the removed material, the through-cut cells,
  the gouge, the uncut material and every crash that rams the tool. The error bound is the
  simulation's own documented coverage bound (`perimeter × cell / 2`), and the same sweep, on the
  field a caller hands in, drives a depth-shaded 3D preview the shell plays.
- **Region geometry, arc-native.** Point-in-contour is an exact winding number, not a tessellated
  guess; a region is a boundary list under even-odd parity with normalization and subtraction; and the
  offset that cuts it keeps a distance proof measured exactly against every original boundary, which is
  what stops a ring from gouging a wall that no vertex of it sees.
- **Import with a report.** DXF, SVG, PDF, EPS and PNG raster trace, each one either converted or
  listed with a reason and a source location — a dropped curve is never silent.
- **Tool library** in SQLite, with tool geometry, cutting data and their join per (material, machine)
  kept as separate records.
- **Data-driven post-processors.** The `.pp` dialect recovered from the analysed product is
  implemented (13 directives, `VAR` formatting with the dialect's `1.4`-means-four-decimals spelling
  and its optional scale factor, 20 output blocks — the four helical ones M6 added among them). No
  third-party post-processor file is vendored.
- **Refusal instead of guessing.** A tool that does not fit, an open contour assigned to a profile
  cut, a post that cannot express arcs, a document with an unknown schema: each fails loudly with the
  reason and a stable exit code.
- **An editor you can drive and test without a GPU.** Click to select an edge, drag a marquee, wheel
  to zoom about the cursor, middle-drag to pan, `Ctrl+Z`/`Ctrl+Shift+Z`/`Delete`/`F` for undo, redo,
  delete and fit. The whole shell runs headless in CI.

## The editor

```bash
cargo run -p cam-desktop          # sample document at launch; Open loads a .c2d, Save writes it back
```

```
┌───────────────────────────────────────────────────────────────────────────────────────────┐
│ campilot   untitled.c2d · modified                     mm · 2D | 3D · ? · Generate G-code │
├───────────┬──────────────────────────────────────────────┬────────────────────────────────┤
│  FILE     │  canvas: grid · stock outline · geometry     │  job, units, selection         │
│  DRAW     │  snap markers · selection highlight          │  history depth                 │
│  EDIT     │                                              │                                │
│  …        │  rail: five collapsible groups,              │                                │
│  campilot │  each heading folds its group                │                                │
│  v0.0.1   │                                              │  inspector: Tools|Selection|Job│
│  Saved …  │                                              │                                │
└───────────┴──────────────────────────────────────────────┴────────────────────────────────┘
   status: cursor X/Y · zoom px/mm · snaps · last message
```

The title bar carries the document's name and nothing it can also be reached from; every command lives in
the rail, grouped under the reference app's five words, and each heading folds its group. The rail's foot is
the brand block: the product, the crate version, and the document's own last write (`Saved 2026-11-03 19:30
UTC`) or `Unsaved changes` while the drawing has edits nobody wrote — the file's stamp, never a build stamp.

What works: pan, zoom about the cursor, zoom-to-fit, grid with a 1/2/5 spacing, picking by screen
distance, marquee selection, snapping to endpoints/midpoints/arc centres/grid, undo/redo with a bounded
history, layer visibility, and the authoring half of the product — line, polyline, arc, circle, ellipse,
rectangle, polygon and star by dragging, single-stroke text, node editing, join, trim, offset,
mirror/rotate/scale, array copy and align, plus importing a file by path and opening a `.c2d` you picked
in the system's file dialog, each as one undoable step. Open is a chip in the left rail's `FILE` group: the
shell only asks the window, which owns the dialog (`rfd`, in `cam-desktop` and not in `cam-ui`), and the file
it answers with is loaded through `cam_store::document::DocumentFile::load` — the archive scan, migration and
`validate()` `cam-cli`'s own loader performs for every command that reads a document — so the title bar names
that file where it said `untitled.c2d`, a refused one changes nothing and says `refused: …`, and a good one is
one undo away.

Save and **Save as** sit beside Open in that group, and they are the other half of that sentence. `Save` writes
the file the title bar names, and with no file yet it is refused — the status line reads `refused: this drawing has no file yet —
use Save As to name one, and Save writes to it from then on` — instead of inventing a path; `Save as` asks the
host for one through the route Open takes (an intent out of the shell, `rfd`'s save dialog in `cam-desktop`, a
path back in as `UiAction::SaveFile`) and becomes the current file, so the bar names what was just written. Both
write through `cam_store::document::DocumentFile::save`, which validates the document first and writes a sibling
`.{name}.tmp` before renaming it into place, so an interrupted save leaves the previous archive on disk rather
than half of each.

Both also carry the archive entries this build does not interpret, which is why the window does **not** call
`save_document`: that function — the one `cam-cli` uses — builds a `DocumentFile` with no extras on purpose, and
a load → edit → save cycle in the window must not drop an `assets/…` entry a later build understands. A save
from an entry point with no shell to hold those entries is refused rather than performed. The dirty flag now
means what it says: a freshly opened document is not modified (it is exactly what is in its file), a save clears
it, and opening a document over unsaved changes is **refused** with a sentence telling you to save or undo first,
where it used to discard the edits with undo as the only way back. A failed save changes nothing — the flag and
the recorded path stay as they were and the status line says `refused: …`, naming the file it could not write.

Opening also resolves the tool library the document names, so the card has something to resolve against.
`tools_ref` is a reference **relative to the document's own directory** — what [docs/format.md](docs/format.md)
already says the field means — so an open joins it that way, loads it through
`cam_store::tools::ToolLibrary::open` and hands the entries to the current-tool card and the cutting-time
estimate; a document with no `tools_ref` opens in exactly the sentence it printed before, with an empty
snapshot. A library that is missing, unreadable or not a library **does not cost the drawing**: it still opens,
the card goes on saying the number it was asked for is not in the library it was given, and the status line
appends the library and the reason to the open's own sentence. A reference that is **absolute, or that carries
a `..`**, is refused before anything is read — the window reads nothing outside the document's directory and
creates nothing anywhere, which is what makes the guard load-bearing: `ToolLibrary::open` creates the parent
directory and the file it is pointed at. `cam-cli` joins the same way without that guard, so a document's
`tools_ref` may still be absolute or climb out of the directory when the CLI reads it.

What is not there yet: **toolpath forms** (M4's and M6's strategies are still authored through
`cam-cli toolpath`, and M5's 3D preview shows a program handed to it — one authored the way `cam-cli`
authors one — rather than a panel that creates one), and Save has no keyboard shortcut — the keys the shell
answers to are `Ctrl+Z`, `Ctrl+Shift+Z`, `Delete`/`Backspace`, `F`, `F1`, `F2` and `Esc`.

Known cosmetic detail: auto-framing fits the canvas as measured when the panel layout settles, so the
first fit can be a few percent tighter than `Zoom to fit` (`F`), which is exact.

## Quickstart

Every recipe below also exists in the `justfile`; `just` on its own lists them.

```bash
just check                 # the three gates: fmt --check, clippy -D warnings, all tests
just run                   # the editor window
just cli emit part.c2d --post my_machine.pp --out part.nc
just cli simulate part.c2d --library tools.sqlite   # removed, gouge, uncut, crashes
just status                # branch, sync with origin, uncommitted files
```

Without `just`:

```bash
# Build and run every gate (fmt, clippy -D warnings, all tests)
./scripts/check.sh

# Inspect a document
cargo run -q -p cam-cli -- inspect part.c2d

# Validate a post-processor
cargo run -q -p cam-cli -- postp validate my_machine.pp

# Create a tool library and add a 6 mm end mill as T1
cargo run -q -p cam-cli -- tools add tools.sqlite --id endmill6 --diameter 6 --number 1
cargo run -q -p cam-cli -- tools list tools.sqlite

# Emit G-code
cargo run -q -p cam-cli -- emit part.c2d --post my_machine.pp --library tools.sqlite --out part.nc
```

Schemas, the post-processor grammar and the CLI contract are specified in
[docs/format.md](docs/format.md).

## Layout

| Crate | Responsibility |
| --- | --- |
| `cam-units` | Millimetres, angles, locale-independent fixed-decimal formatting |
| `cam-geom` | Points, arcs, bulged contours, chaining, tessellation, arc-preserving offset, regions and their exact boundary distance, the verified medial axis, and the moulding rail's arc-length walk with its cross-section and stepover law |
| `cam-move` | The move vocabulary: the contract between toolpaths and the emitter |
| `cam-model` | Job, layers, entities, toolpaths, schema version, validation |
| `cam-store` | `.c2d` documents and the SQLite tool library, both with migrations |
| `cam-post` | `.pp` parser and the data-driven G-code emitter |
| `cam-toolpath` | Machining strategies: profile, pocket (offset and raster, with islands), drilling, the six M6 depth variants, the V-carve and the moulding |
| `cam-sim` | Material-removal simulation: the stock's heightfield, swept removal, gouge and uncut reporting |
| `cam-cli` | Headless pipeline and the verification surface |
| `cam-import` | DXF, SVG, PDF, EPS and PNG raster import, with a report of what was converted or skipped |
| `cam-app` | Application state machine: commands, selection, undo/redo. No UI dependencies |
| `cam-view` | Backend-agnostic scene and grid, picking, marquee and snapping in screen pixels |
| `cam-ui` | `egui` shell: docks, canvas, properties, status bar, authoring tools |
| `cam-desktop` | The native window for Windows and Linux |

Dependency direction is one-way: units → geom → move → model → store, with `post` and `toolpath` on the
move vocabulary; `sim` on geom, move and model; `app` on model + geom; `view` on app, geom and sim; `ui`
on view and sim; `desktop` and `cli` on everything. Nothing below `cam-ui` knows that `egui` exists.

## Why the design looks like this

The decisions come from a REA analysis of the reference CAM application
(`evidence/analysis-report.md`), and each one is traceable to something observed in the artifact:

| Observation | Decision |
| --- | --- |
| Only the UI shell (MFC + Codejock) and the viewport (OpenSceneGraph) are Windows-bound | Port the domain, replace the shell: Rust + `wgpu`, `egui` behind an adapter |
| Tool library is SQLite with geometry, cutting data and (material, machine) as separate records | Same normalisation, with forward-only migrations |
| Post-processors are data: 75 files, 13 directives, 16 blocks | Implement the dialect; never hard-code a machine |
| Arc output exists in 8 of the 16 blocks | Arcs stay arcs through every stage; no pre-tessellation |
| Nesting is a licensed third-party engine (298 exported functions) | Own heuristic engine, scheduled as M9 |
| `.crv` projects are OLE2 compound documents with unknown internals | Own open format; interoperate through DXF/SVG/PDF/EPS import |
| Only the shell and the viewport are Windows-bound; the domain is portable | Replace the shell (Rust + `egui`), keep the domain; the draw list is backend-agnostic so a `wgpu` renderer can land with the 3D view |

The program plan, milestone sequence and risk register are in [odd/roadmap.md](odd/roadmap.md).

## Testing

1452 tests across 103 test targets, all deterministic — including the editor, which runs real `egui`
frames headless and asserts on both the actions they produce and the text they paint:

```bash
cargo test --workspace
```

They cover, in order of what they protect: golden G-code (byte for byte, through the real binary,
including pocket-with-island and drilling programs), property tests on the geometry kernel (offset
simplicity, arc fidelity, area monotonicity, winding against a fine tessellation, a distance proof that
never keeps a piece it has not measured), the simulation's oracle for every strategy M4, M6, M7 and M8
ship (a pocket's removed volume against its analytic area within the coverage bound, a chamfer's both
flanks, a ramped flute's closed form, a thread's spiral and its crest, a V-carve's distance field on a
square and a disc, a moulding's swept cross-section on a straight rail, a closed frame and a V-bit cone,
each with a program cut a millimetre shallow reported as uncut and one cut a millimetre deep reported as
gouge), the document and tool-library round trips and migrations (including `c2d/document@1` loading and
saving as `@2`), every parser diagnostic by line and column, and the editor's behaviour — undo restoring a document exactly, picking measured in
pixels at any zoom, snapping, and synthetic clicks, drags, wheel and keys driving the real shell.

The native window itself is the one thing not covered by automation: it needs a display. It is
verified by building and by launching it under a compositor, which is how the screenshot in
`docs/images/` was taken.

## Next

M10 is what the plan still holds: HTML print sheets, CSV part lists and the scripting surface. M9,
delivered, was nesting: sheets and parts, rectangle **maxrects + guillotine** first and a sound-but-
incomplete **free-profile no-fit heuristic** second, with one arithmetic for utilisation and waste that
the report, the stored cost record and load-time validation all read. M8 was moulding: a rail and a
cross-section swept along it with a stepover, the first strategy whose answer depends on a cross-section
rather than a single depth.
It was scheduled after V-carve on the expectation that it needed the distance field M7 built, and it did
not: the cross-section is authored data, so M8 changed no post contract, no schema and no dependency, and
spent its effort on the rail's arc-length walk and the stepover law's coverage certificate instead. M7
itself carried one defect forward and closed it rather than leaving it: `cam-sim` used to build its field
at the document origin while programs are emitted in machine coordinates, so at any origin corner other
than bottom-left the report and the preview disagreed about where the cut was — for every strategy since M5 — and the fix (`cam-model::Frame`, honoured at every
boundary including the handedness of every arc) has a test per origin corner. See
[odd/roadmap.md](odd/roadmap.md) §4 and §4.6–4.11 for what M4, the region work, M5, M6, M7 and M8
delivered.
