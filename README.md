# campilot

A cross-platform (Windows + Linux) 2.5D CAM application for CNC routers, built as an evidence-based
answer to a simple question: *what would the reference CAM application look like if it were designed today, in Rust,
with the domain separated from the window?*

**Status: M0 complete.** The foundation and the vertical slice work end to end — a document with a
closed contour, a tool from a SQLite library and a post-processor produce real G-code, byte for byte
tested. There is no UI yet; that is M1.

## What works today

```
part.c2d ──▶ document model ──▶ profile toolpath ──▶ move stream ──▶ post-processor ──▶ part.nc
   (ZIP+JSON)     (validated)      (offsets, passes,        (Rapid/Feed/   (.pp dialect)
                                    tabs, leads)             Arc/Retract)
```

- **Arc geometry end to end.** Arcs are first-class from the document through the offset to the
  emitter: a circle cut with a profile toolpath emits `G02`/`G03`, never a polyline.
- **Tool library** in SQLite, with tool geometry, cutting data and their join per (material, machine)
  kept as separate records.
- **Data-driven post-processors.** The `.pp` dialect recovered from the analysed product is
  implemented (13 directives, `VAR` formatting with the dialect's `1.4`-means-four-decimals spelling
  and its optional scale factor, 16 output blocks). No third-party post-processor file is vendored.
- **Refusal instead of guessing.** A tool that does not fit, an open contour assigned to a profile
  cut, a post that cannot express arcs, a document with an unknown schema: each fails loudly with the
  reason and a stable exit code.

## Quickstart

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
| `cam-toolpath` | Machining strategies; M0 ships `profile` |
| `cam-cli` | Headless pipeline and the verification surface |

Dependency direction is one-way: units → geom → move → model → store, with `post` and `toolpath` on the
move vocabulary and `cli` on everything. No crate depends on a UI crate.

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

The program plan, milestone sequence and risk register are in [odd/roadmap.md](odd/roadmap.md).

## Testing

123 tests across 13 suites, all deterministic:

```bash
cargo test --workspace
```

They cover, in order of what they protect: golden G-code (byte for byte, through the real binary),
property tests on the geometry kernel (offset simplicity, arc fidelity, area monotonicity), the
document and tool-library round trips and migrations, and every parser diagnostic by line and column.

## Next

M1 is the 2D editor shell: viewport, pan/zoom/snap, entity picking, undo/redo, layer dock — all of it
riding on `cam-app`, which does not exist yet because nothing so far needed a UI. See
[odd/roadmap.md](odd/roadmap.md) §4.
