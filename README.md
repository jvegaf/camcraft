# campilot

A cross-platform (Windows + Linux) 2.5D CAM application for CNC routers, built as an evidence-based
answer to a simple question: *what would the reference CAM application look like if it were designed today, in Rust,
with the domain separated from the window?*

**Status: M4 complete.** The domain runs end to end — a document, a tool from a SQLite library and a
post-processor produce real G-code, byte for byte tested — for **profile**, **pocket** (offset rings
and raster rows, with islands) and **drilling** (pecks, both retract modes, drill-bank selection).
Around it: M1 built the editor shell, M2 the vector authoring and editing commands, M3 the DXF, SVG,
PDF, EPS and raster importers, and M4 the toolpaths, authored headlessly through `cam-cli toolpath`.
Simulation is next (M5).

## What works today

```
dxf/svg/pdf/eps/png ──▶ document model ──▶ toolpath ──▶ move stream ──▶ post-processor ──▶ part.nc
     (cam-import)        (c2d/document@2)   profile/     (Rapid/Feed/     (.pp dialect)
                          (validated)       pocket/       Arc/Retract)
                                            drilling
```

- **Arc geometry end to end.** Arcs are first-class from the document through the offset to the
  emitter: a circle cut with a profile toolpath emits `G02`/`G03`, never a polyline.
- **Three strategies, one contract.** Profile (inside/outside/on, tabs, passes, lead-in/out), pocket
  (offset rings or raster rows, allowance, stepover, nesting islands) and drilling (peck depth, both
  retract modes, drill-bank selection). Each is authored by `cam-cli toolpath` and reaches G-code
  through `cam-cli emit`, so the test surface is the program a machine would run.
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
  and its optional scale factor, 16 output blocks). No third-party post-processor file is vendored.
- **Refusal instead of guessing.** A tool that does not fit, an open contour assigned to a profile
  cut, a post that cannot express arcs, a document with an unknown schema: each fails loudly with the
  reason and a stable exit code.
- **An editor you can drive and test without a GPU.** Click to select an edge, drag a marquee, wheel
  to zoom about the cursor, middle-drag to pan, `Ctrl+Z`/`Ctrl+Shift+Z`/`Delete`/`F` for undo, redo,
  delete and fit. The whole shell runs headless in CI.

## The editor

```bash
cargo run -p cam-desktop          # opens the window with a sample document
```

```
┌──────────────────────────── commands: undo · redo · zoom to fit · add rectangle ──────────┐
│ layers              │                  canvas                  │        properties        │
│ L1 — Sample         │  grid · stock outline · geometry         │  job, units, selection   │
│ 2 entity(ies)       │  snap markers · selection highlight      │  history depth           │
└─────────────────────┴───────────────────────────────────────┴────────────────────────────┘
   status: cursor X/Y · zoom px/mm · saved/modified · undo depth
```

What works: pan, zoom about the cursor, zoom-to-fit, grid with a 1/2/5 spacing, picking by screen
distance, marquee selection, snapping to endpoints/midpoints/arc centres/grid, undo/redo with a bounded
history, layer visibility, and the authoring half of the product — line, polyline, arc, circle, ellipse,
rectangle, polygon and star by dragging, single-stroke text, node editing, join, trim, offset,
mirror/rotate/scale, array copy and align, plus importing a file by path as one undoable step.

What is not there yet: **toolpath forms** (M4's strategies are authored through `cam-cli toolpath`;
the window gains forms with M5, when a simulation can show what they cut), simulation and the 3D stock
view (M5), and a file menu for saving and opening (the formats and the CLI exist; the window still takes
documents from its caller).

Known cosmetic detail: auto-framing fits the canvas as measured when the panel layout settles, so the
first fit can be a few percent tighter than `Zoom to fit` (`F`), which is exact.

## Quickstart

Every recipe below also exists in the `justfile`; `just` on its own lists them.

```bash
just check                 # the three gates: fmt --check, clippy -D warnings, all tests
just run                   # the editor window
just cli emit part.c2d --post my_machine.pp --out part.nc
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
| `cam-geom` | Points, arcs, bulged contours, chaining, tessellation, arc-preserving offset |
| `cam-move` | The move vocabulary: the contract between toolpaths and the emitter |
| `cam-model` | Job, layers, entities, toolpaths, schema version, validation |
| `cam-store` | `.c2d` documents and the SQLite tool library, both with migrations |
| `cam-post` | `.pp` parser and the data-driven G-code emitter |
| `cam-toolpath` | Machining strategies: profile, pocket (offset and raster, with islands), drilling |
| `cam-cli` | Headless pipeline and the verification surface |
| `cam-import` | DXF, SVG, PDF, EPS and PNG raster import, with a report of what was converted or skipped |
| `cam-app` | Application state machine: commands, selection, undo/redo. No UI dependencies |
| `cam-view` | Backend-agnostic scene and grid, picking, marquee and snapping in screen pixels |
| `cam-ui` | `egui` shell: docks, canvas, properties, status bar, authoring tools |
| `cam-desktop` | The native window for Windows and Linux |

Dependency direction is one-way: units → geom → move → model → store, with `post` and `toolpath` on the
move vocabulary; `app` on model + geom; `view` on app + geom; `ui` on view; `desktop` and `cli` on
everything. Nothing below `cam-ui` knows that `egui` exists.

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

707 tests across 56 test targets, all deterministic — including the editor, which runs real `egui`
frames headless and asserts on both the actions they produce and the text they paint:

```bash
cargo test --workspace
```

They cover, in order of what they protect: golden G-code (byte for byte, through the real binary,
including pocket-with-island and drilling programs), property tests on the geometry kernel (offset
simplicity, arc fidelity, area monotonicity, winding against a fine tessellation, a distance proof that
never keeps a piece it has not measured), the document and tool-library round trips and migrations
(including `c2d/document@1` loading and saving as `@2`), every parser diagnostic by line and column, and
the editor's behaviour — undo restoring a document exactly, picking measured in pixels at any zoom,
snapping, and synthetic clicks, drags, wheel and keys driving the real shell.

The native window itself is the one thing not covered by automation: it needs a display. It is
verified by building and by launching it under a compositor, which is how the screenshot in
`docs/images/` was taken.

## Next

M5 is simulation: heightfield material removal, gouge and uncut reporting, a 3D stock view and an
animated preview. It is the only honest acceptance test for the strategies that already ship — a green
suite says the toolpath satisfies its rules, not that the part came out right — and it is what the
deliberately conservative linking between rings and rows was waiting for. See
[odd/roadmap.md](odd/roadmap.md) §4 and §4.6–4.7 for what M4 and the region work delivered.
