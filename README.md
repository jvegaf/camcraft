# campilot

A cross-platform (Windows + Linux) 2.5D CAM application for CNC routers, written in Rust with the CAM
domain separated from the window: one document model, one set of toolpaths and one emitted program,
whether the work is driven headlessly through `cam-cli` or from the editor.

**Status:** The document model, importers, geometry kernel, post-processor dialect, tool library, nesting, simulation, and editor shell are in place. The general-machining strategies and their `cam-cli toolpath`/`emit`/`simulate` pipeline have been removed; PCB operations are being rebuilt separately.

## What works today

```
dxf/svg/pdf/eps/png ──▶ document model
     (cam-import)        (c2d/document@2, validated)
```

- **Arc geometry end to end.** Arcs are first-class from the document through the offset: geometry keeps `G02`/`G03`-style arcs as arcs, never a polyline.
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
- **A command language, a cut list and a print sheet.** `cam-cli script file.cam` runs one command per
  line against one document — `open`, `sheet`, `part`, `nest`, `validate`, `save`, `parts`, `cutlist` — and every one of them hands its tokens to the **same flag
  parser** and the same function its `cam-cli` subcommand calls. The parity is bytes: a
  scripted `nest` writes the document `cam-cli nest` writes, and a
  script's captured output is identical to the binary's stdout. A refusal is located and named —
  `line 4:6: --gap must be a non-negative distance in mm, got -1` — and every mutation passes
  `Document::validate` on a clone, so a refused script leaves the file exactly as it was. A part list is
  four columns — `name,width,height,quantity` — with the header required in that order (so a swapped
  width and height refuses instead of rotating every part), quotes refused by name and duplicate names
  refused rather than summed; `cam-cli parts doc.c2d --csv list.csv` authors one rectangle and one nesting
  part per row of one. `cam-cli cutlist doc.c2d` writes the placements M9 stored as seven columns at
  three decimals, and `cam-cli report doc.c2d` writes a self-contained HTML print sheet — every style
  inside the file, every
  id escaped by hand, the nesting's utilisation from the document's own arithmetic, and an `Omitted`
  section naming what a sheet cannot honestly report. The shell's half is one panel: `cam-ui`'s
  `ScriptPanel` calls `cam_cli::script::run_script_to` as a **library**, shows the run's report and its
  located error in one area, keeps the previous result when a run fails, and never spawns the binary.
- **Simulation.** `cam-sim` sweeps a move stream over a heightfield of the stock and can report removed material, gouge, uncut material and crashes.
- **Region geometry, arc-native.** Point-in-contour is an exact winding number, not a tessellated
  guess; a region is a boundary list under even-odd parity with normalization and subtraction; and the
  offset that cuts it keeps a distance proof measured exactly against every original boundary, which is
  what stops a ring from gouging a wall that no vertex of it sees.
- **Import with a report.** DXF, SVG, PDF, EPS and PNG raster trace, each one either converted or
  listed with a reason and a source location — a dropped curve is never silent.
- **Tool library** in SQLite, with tool geometry, cutting data and their join per (material, machine)
  kept as separate records.
- **Data-driven post-processors.** The `.pp` dialect is implemented (13 directives, `VAR` formatting
  with the dialect's `1.4`-means-four-decimals spelling and its optional scale factor, 20 output
  blocks — the four helical ones M6 added among them). No third-party post-processor file is vendored.
- **Refusal instead of guessing.** A tool that does not fit, an open contour assigned to a profile
  cut, a post that cannot express arcs, a document with an unknown schema: each fails loudly with the
  reason and a stable exit code.
- **An editor you can drive and test without a GPU.** Click to select an edge, drag a marquee, wheel
  to zoom about the cursor, middle-drag to pan, `Ctrl+Z`/`Ctrl+Shift+Z`/`Delete`/`F` for undo, redo,
  delete and fit. The whole shell runs headless in CI.

## The editor

```bash
cargo run -p cam-desktop          # opens with no project; the start screen offers New project, Open project, and recent projects
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

**The tool library belongs to the application, not to a drawing.** There is one library — `tools.sqlite` in
the application's data directory (`$CAM_PILOT_DATA_DIR` when it is set, otherwise the platform's per-user data
directory: `$XDG_DATA_HOME/campilot` or `~/.local/share/campilot` on Linux, `%APPDATA%\campilot` on Windows) —
and it is the same library for every document the window opens. On first launch the window creates the file and
writes the **factory catalogue**: 22 tools across the six kinds, T1 to T22, with speeds and feeds derived by one
rule rather than hand-tuned per tool (feed 100 mm/min, plunge 30, spindle 18000 rpm, depth of cut half the
diameter capped at 3 mm, stepover 40 % of it) — starting values, not a machine's numbers. A library you empty
stays empty: seeding is a first-launch answer, and `Restore defaults` is the one press that puts the catalogue
back. Opening a document therefore changes nothing about the library, and the current-tool card and the
cutting-time estimate resolve against that one file.

`tools_ref` is still a document field and `cam-cli` still honours it — a reference **relative to the document's
own directory**, which is what [docs/format.md](docs/format.md) says it means — so a document may name a library
for the command line. The **window** does not read it: a drawing owning a library is exactly what the
application-level rule rules out. From the command line the field is joined without a containment guard, so it
may be absolute or climb out of the directory; the CLI creates the parent directory and the file it is pointed
at when it means to write, which is why the shell has no such path to guard any more.

What is not there yet: toolpath authoring, and Save has no keyboard shortcut — the keys the shell
answers to are `Ctrl+Z`, `Ctrl+Shift+Z`, `Delete`/`Backspace`, `F`, `F1`, `F2` and `Esc`.

Known cosmetic detail: auto-framing fits the canvas as measured when the panel layout settles, so the
first fit can be a few percent tighter than `Zoom to fit` (`F`), which is exact.

## Quickstart

Every recipe below also exists in the `justfile`; `just` on its own lists them.

```bash
just check                 # the three gates: fmt --check, clippy -D warnings, all tests
just run                   # the editor window
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
| `cam-sim` | Material-removal simulation: the stock's heightfield, swept removal, gouge and uncut reporting |
| `cam-cli` | Headless pipeline and the verification surface — a library with a 16-line binary, so the shell's scripting panel calls the same functions (D-M10-1) |
| `cam-import` | DXF, SVG, PDF, EPS and PNG raster import, with a report of what was converted or skipped |
| `cam-app` | Application state machine: commands, selection, undo/redo. No UI dependencies |
| `cam-view` | Backend-agnostic scene and grid, picking, marquee and snapping in screen pixels |
| `cam-ui` | `egui` shell: docks, canvas, properties, status bar, authoring tools |
| `cam-desktop` | The native window for Windows and Linux |

Dependency direction is one-way: units → geom → move → model → store, with `post` and `toolpath` on the
move vocabulary; `sim` on geom, move and model; `app` on model + geom; `view` on app, geom and sim; `ui`
on view and sim; `desktop` and `cli` on everything. Nothing below `cam-ui` knows that `egui` exists.

## Why the design looks like this

Each decision below has a reason that stands on its own — in the CAM domain, or in this repository's
own tests and measurements — and none of them rests on a preference:

| Reason | Decision |
| --- | --- |
| The CAM domain is portable and only the window is not: nothing below `cam-ui` knows that `egui` exists, and the draw list is backend-agnostic | Port the domain, replace the shell: Rust + `wgpu`, `egui` behind an adapter |
| One geometry feeds many cuts: the same 6 mm end mill feeds differently in MDF and in hardwood, and differently again on a light machine, so geometry, cutting data and the (material, machine) pair change independently | Three normalised records joined per pair, with forward-only migrations |
| A post-processor is data, never code: a `.pp` file declares thirteen directives and twenty output blocks, and the emitter reads all of them | Implement the dialect; never hard-code a machine |
| An arc's centre is what the controller needs and the first thing a chord loses, so no stage tessellates ahead of the emitter | Arcs stay arcs through every stage; no pre-tessellation |
| A nesting plan is worth trusting only when it states its promise as one predicate a test can check, and the repository takes no new dependency unless a task proves it necessary | Own heuristic engine, scheduled as M9 |
| A project file must be readable, diffable and versioned by its own schema tag — `c2d/document@1` loads and saves as `@2` — and interchange belongs to published formats | Own open format; interoperate through DXF/SVG/PDF/EPS import |

The program plan, milestone sequence and risk register are in [odd/roadmap.md](odd/roadmap.md).

## Testing

1506 tests, all deterministic — including the editor, which runs real `egui` frames headless and asserts
on both the actions they produce and the text they paint:

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

M10 is delivered: the command language, the CSV part list and cut list, the HTML print sheet and a
minimal scripting panel. M9 was nesting: sheets and parts, rectangle **maxrects + guillotine** first and
a sound-but-incomplete **free-profile no-fit heuristic** second, with one arithmetic for utilisation and
waste that the report, the stored cost record and load-time validation all read. M8 was moulding: a rail
and a cross-section swept along it with a stepover, the first strategy whose answer depends on a
cross-section rather than a single depth.
It was scheduled after V-carve on the expectation that it needed the distance field M7 built, and it did
not: the cross-section is authored data, so M8 changed no post contract, no schema and no dependency, and
spent its effort on the rail's arc-length walk and the stepover law's coverage certificate instead. M7
itself carried one defect forward and closed it rather than leaving it: `cam-sim` used to build its field
at the document origin while programs are emitted in machine coordinates, so at any origin corner other
than bottom-left the report and the preview disagreed about where the cut was — for every strategy since M5 — and the fix (`cam-model::Frame`, honoured at every
boundary including the handedness of every arc) has a test per origin corner. See
[odd/roadmap.md](odd/roadmap.md) §4 and §4.6–4.13 for what M4, the region work and M5–M10 delivered.
