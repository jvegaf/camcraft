# Feature: 2.5D CAM application — Milestone M0 (foundation + vertical slice)

Program plan: `odd/roadmap.md`. This document is the executable specification for M0 only.
Read this file until `## Log` before implementing any task. Stack: Rust, no UI crate in M0.

## Specs

Requirements as stated by the owner (verbatim; S1 and S3 restated by the name-hygiene sweep):

- **S1** — A desktop 2.5D CAM application (restated; the owner's wording named a third-party product).
- **S2** — "quiero que sea cross platform Windows/Linux"
- **S3** — The development plan is generated through ODD: milestones, each with its own feature document
  and its own acceptance criteria.
- **S4** — Decision D1 (user, verbatim option label): "Rust + wgpu/egui (o Rust + Qt bindings)"
- **S5** — Decision D2 (user, verbatim option label): "Todo menos 3D y Laser"
- **S6** — Decision D3 (user, verbatim option label): "Formato propio abierto (JSON/ZIP) + import
  DXF/SVG/PDF/EPS"
- **S7** — Decision D4 (user, verbatim option label): "Implementar nesting heurístico propio en v1"

Everything else in this document is a **derived acceptance criterion** for M0 — an engineering
consequence of S1–S7 with no new scope. Each one is falsifiable and is the thing a task is checked
against.

### Derived acceptance criteria (M0)

- **A1** — Internal lengths are millimetres as `f64`; no unit conversion happens inside geometry or
  toolpath code; a document's declared unit is metadata only.
- **A2** — Arcs are first-class through the whole pipeline: an arc authored in a document reaches the
  post-processor emitter as an arc and can produce `G2`/`G3` output. No stage pre-tessellates.
- **A3** — The document format is a ZIP container holding `document.json` with an explicit
  `"schema": "c2d/document@1"`. Round-trip (read→write→read) is lossless. Unknown schema versions are
  rejected with a message naming the found and the supported version.
- **A4** — The tool library is SQLite with a `schema_version`/migration mechanism, and models tool
  **geometry**, tool **cutting data** and their join per (material, machine) as separate records.
- **A5** — The post-processor engine parses the `.pp` dialect: 13 directives
  (`POST_NAME`, `FILE_EXTENSION`, `UNITS`, `LINE_ENDING`, `LINE_NUMBER_START`,
  `LINE_NUMBER_INCREMENT`, `LINE_NUMBER_MAXIMUM`, `DIRECT_OUTPUT`, `SUBSTITUTE`, `ROTARY_WRAP_X`,
  `ROTARY_WRAP_Y`, `RAPID_PLUNGE_TO_STARTZ`, `PRINT_DIRECT`), `VAR <name> = [<src>|A|C|<prefix>|<decimals>]`
  formatting, and the 15 output blocks (`HEADER`, `FOOTER`, `INITIAL_RAPID_MOVE`, `RAPID_MOVE`,
  `FIRST_RAPID_MOVE`, `PLUNGE_MOVE`, `FIRST_FEED_MOVE`, `FEED_MOVE`, `RETRACT_MOVE`,
  `FIRST_CW_ARC_MOVE`, `CW_ARC_MOVE`, `FIRST_CCW_ARC_MOVE`, `CCW_ARC_MOVE`, `FEED_RATE_CHANGE`,
  `NEW_SEGMENT`, `TOOLCHANGE` — **16 blocks**; the earlier "15" in this document was a miscount, and
  the `VAR` form also carries an optional fifth scale field, both corrected during T8/T9 and recorded
  in the analysis report §6). Malformed input fails with a `line:column` diagnostic, never silently.
- **A6** — Rounding and number formatting happen **only** in the emitter, driven by the `decimals`
  field of each `VAR` declaration.
- **A7** — No third-party `.pp` file is committed to this repository; fixtures are authored by us.
- **A8** — The whole M0 behaviour is reachable through `cam-cli` with a stable exit-code and stdout
  contract, so every check runs headless on Windows and Linux.
- **A9** — Identical input produces byte-identical G-code across runs and across platforms.

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `9405bb8` | eight crates, dependency direction enforced by `Cargo.toml`, `fmt`/`clippy -D warnings`/`test` all 0 |
| T2 | `3a0aaae` | `cam-units`; 10 tests green, incl. 3 property tests |
| T3 | `7ffbd98` | `cam-geom` primitives + chaining; 13 tests green (4 property tests). 1,390 lines: oversized for one review, but the crate splits cleanly per file (point / arc / contour / chain) |
| T4 | `6b15cad` | arc-preserving offset; 12 tests green (3 property tests). Two real defects found and fixed at the root (see L9) |
| T5 | `3d298f6` | document model; 9 tests green. Schema, dimensions, duplicate ids, dangling references and open geometry all rejected |
| T6 | `3e0758c` | `.c2d` ZIP+JSON persistence; 8 tests green, incl. unknown-entry preservation and atomic overwrite |
| T7 | `62fd277` | SQLite tool library; 7 tests green, incl. a real v1→v2 migration of a hand-built legacy file |
| T8 | `83dead6` | `.pp` parser; 18 tests green, one per directive/block plus every rejection path with its line |
| T9 | `a3cb385` | emitter; 13 tests green, incl. the byte-for-byte profile program and the arc refusal |
| T10 | `1621854` | profile strategy; 20 tests green (passes, sides, allowance, tabs, leads, origin mapping) |
| T11 | `78ad9e9` | CLI + end-to-end golden program through the real binary; 12 tests green |
| T12 | `HEAD` | `README.md`, `docs/format.md`, roadmap status, corrected evidence |

## Tasks

One line per task: ID — spec — route — commit.

- **T1** — A1, A8 — inline — `chore: scaffold cargo workspace for the cam pipeline`
- **T2** — A1, A6 — inline — `feat(units): millimetre-exact length and unit system`
- **T3** — A2 — inline — `feat(geom): arc-first-class 2d primitives and chaining`
- **T4** — A2 — inline — `feat(geom): arc-preserving contour offset`
- **T5** — A3 — inline — `feat(model): document model with explicit schema version`
- **T6** — A3 — inline — `feat(store): zip+json document persistence`
- **T7** — A4 — inline — `feat(store): sqlite tool library with forward migrations`
- **T8** — A5, A7 — inline — `feat(post): post-processor dsl parser with located diagnostics`
- **T9** — A5, A6, A9 — inline — `feat(post): data-driven g-code emitter`
- **T10** — A2, A6 — inline — `feat(toolpath): profile strategy with passes, tabs and leads`
- **T11** — A8, A9 — inline — `feat(cli): headless pipeline commands with golden g-code test`
- **T12** — A3, A5, A8 — inline — `docs: m0 quickstart and format reference`

Route notes: M0 is a strict dependency chain (T2→T3→T4, T5→T6, T8→T9, T9+T10→T11), so no two tasks
are independent units of behaviour; parallel writers do not apply. Re-evaluate the route at T7 (tool
library) and at T12 (docs) if either grows beyond a single reviewable unit.

### Task detail

**T1 — Workspace skeleton.** `cargo` workspace with `crates/cam-units`, `cam-geom`, `cam-move`,
`cam-model`, `cam-store`, `cam-post`, `cam-toolpath`, `cam-cli`. Each crate has a doc comment stating
its responsibility and what it must not depend on (`cam-geom` must not depend on `cam-store`; nothing
in M0 depends on a UI crate). Dependency direction: `cam-units` < `cam-geom` < `cam-move` <
`cam-model` < `cam-store`; `cam-post` depends on `cam-move`; `cam-toolpath` on `cam-geom` +
`cam-move`; `cam-cli` on all. `cargo test`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check` green. Checks: the three commands above, exit code 0.

**T2 — Units.** `Length` (millimetres `f64`), `Angle`, `UnitSystem::{Mm, Inch}`, conversion helpers
used only at the document boundary, and a `Format` type that produces fixed-decimal text without
locale dependence. TRY: property test that converting mm→inch→mm is stable to 1e-9 and that
`Format` never emits `-0.000` for a value that rounds to zero.

**T3 — Geometry primitives.** `Point2`, `Vec2`, `LineSeg`, `Arc` (center, radius, start/end angle,
direction), `Polyline` with per-vertex bulges, `Bezier`. Chaining: turn a bag of segments/arcs into
closed loops within a tolerance; report loops that cannot close. Winding, signed area, bounding box,
tessellation (`arc → segments` honoring a chord tolerance). TRY: property tests — chaining a
tessellated circle yields one closed loop; reversing a polyline flips the sign of the signed area;
tessellating an arc never deviates from the arc by more than the chord tolerance.

**T4 — Offsetting.** Arc-preserving offset of a closed loop by `±radius` (tool radius), inside/outside
semantics as in `ProfileSide` (`inside`/`outside`/`on`). Handle self-intersection removal at the
offset distance. TRY: property test with randomized simple polygons that an offset loop is still
simple (no self-intersection) and its area changes monotonically; golden-shape tests for a rectangle,
a circle, and a rectangle-with-arc-corners asserting the offset keeps arcs as arcs.

**T5 — Document model.** `Job` (width, height, thickness, units, origin corner, safe Z), `Layer`,
`Entity` (line, polyline, arc, circle, bezier, text placeholder), `Toolpath` (strategy id, entity
refs, tool id, params map), stable string IDs, `"schema": "c2d/document@1"`. Serde with
`deny_unknown_fields` only where forward-compatibility allows. TRY: unit test that a document with an
entity referencing a missing layer fails validation with an error naming both IDs.

**T6 — Document persistence.** ZIP container: `document.json` at root, `assets/` passthrough.
Atomic write (temp file + rename). Reject a document whose `schema` is newer than supported with the
found and supported values in the message. TRY: round-trip test on a document with one layer, one
entity and one toolpath; test that a corrupt zip exits non-zero with a located message.

**T7 — Tool library.** SQLite (`rusqlite`) with forward-only migrations under a `schema_version`
table. Tables: `tool_tree_entry` (self-referencing folder tree with `sibling_order`),
`tool_geometry` (tool_type, units, diameter, included_angle, flat_diameter, num_flutes, flute_length,
tip_radius, outline), `tool_cutting_data` (feed_rate, plunge_rate, spindle_speed, stepdown, stepover,
clear_stepover, tool_number), `tool_entity` (material_id, machine_id, tool_geometry_id,
tool_cutting_data_id), `material`, `machine` (name, make, model, controller_type, max_width,
max_height, support_rotary, support_tool_change). Tool display names support the template form
`{Tool Type} ({Diameter|F}{Units Short})`. TRY: migration test from an empty database to head; test
that the same tool geometry carries different cutting data for two materials and both survive a
reopen.

**T8 — Post-processor parser.** Lexer plus directive/`VAR`/block parser for A5, with a diagnostic
carrying `line:column`, the offending token and the expected set. Comments are `+`-prefixed lines.
Fixtures authored by us: one covering all 13 directives, one per output block, and a set of malformed
files (unknown directive, unterminated block, `VAR` with wrong arity, invalid axis letter, non-numeric
decimals). TRY: each malformed fixture fails with the expected diagnostic position.

**T9 — Emitter.** Takes the parsed post-processor plus a move stream and produces text. Implements
`A` (always) vs `C` (emit only on change) semantics, prefix letters, per-variable decimals, block
selection (`FIRST_*` variants apply once per toolpath, `*_ARC_MOVE` only when the stream carries an
arc), line numbering start/increment/maximum with wrap, and `LINE_ENDING` escapes (`[13][10]`).
TRY: golden-file test per block; a "no `-0.000`" test; a test that the same stream emitted twice
produces identical bytes.

**T10 — Profile toolpath.** Consumes validated closed loops, applies side/inside/outside/on,
allowance, `pass_depth` (multiple Z passes with ordered depth), tabs (position + width + height) and
lead-in/lead-out, producing the shared move stream (`Rapid`, `Feed`, `Arc`, `Plunge`, `Retract`,
`ToolChange`). TRY: analytic tests — a 20 mm deep pocket with 6 mm pass depth yields 4 passes at
6/12/18/20; a tab raises Z by exactly its height over exactly its arc length; an "on" profile emits
no offset.

**T11 — CLI.** Subcommands: `validate <doc>`, `inspect <doc>` (counts, entities, toolpaths),
`tools list|add`, `postp validate <file>`, `emit <doc> --post <file> --out <path>`. Stable contract:
`0` success, `2` invalid input, `3` unsupported schema, `4` post-processor error; human messages on
stderr, machine output on stdout. Help text must describe every command, option and exit code.
TRY: end-to-end golden test — a fixture document with one rectangle layer and one outside profile
through a fixture post-processor produces the exact expected `.nc`, exit 0, empty stderr; and a
rejected document exits 2 leaving no output file behind.

**T12 — Close M0.** `docs/format.md` (document schema, tool library schema, post-processor grammar),
`README.md` quickstart (`cargo run -p cam-cli -- emit ...`), update `odd/roadmap.md` status, and
record the commit holding each task. No behaviour change.

## Log

- **L1 (user requirement).** A cross-platform 2.5D CAM application, planned through ODD, with
  Windows/Linux support named as the extra requirement. The owner's sentence also named a third-party
  product; the repository name hygiene sweep (`odd/tasks/repository-name-hygiene.md`) removed that name
  and the words that went with it.
- **L2 (user decisions, verbatim option labels):** "Rust + wgpu/egui (o Rust + Qt bindings)";
  "Todo menos 3D y Laser"; "Formato propio abierto (JSON/ZIP) + import DXF/SVG/PDF/EPS";
  "Implementar nesting heurístico propio en v1".
- **L3 (analysis):** the plan's technical basis, recorded: the tool and post-processor record shapes and
  the post-processor grammar M0 implements — 13 directives / 17 variables / 15 blocks.
- **L4 (plan):** `odd/roadmap.md` — stack justification, architecture, milestones M0–M10, risks.
- **L5:** scope correction recorded, not silently absorbed: D2 includes V-Carve, Inlay, Moulding and
  the other depth-variant strategies, so those are scheduled as M6–M8 rather than folded into the
  core toolpath milestone; nesting (D4) is M9. M0 deliberately contains no strategy beyond `profile`
  so the vertical slice stays reviewable.
- **L6 (deviation from the roadmap crate list):** a ninth crate, `cam-move`, holds the canonical
  move vocabulary (`Rapid`, `Feed`, `Arc`, `Plunge`, `Retract`, `ToolChange`, `MoveStream`). Without
  it, either `cam-post` would depend on the whole `cam-toolpath` engine or the move types would live
  inside the emitter, and both invert the intended dependency direction: the toolpath produces the
  stream, the emitter consumes it, and each must be testable alone. The roadmap's crate list is
  amended accordingly rather than left contradicting the code.
- **L7 (execution, T1–T2):** delegation unavailable — `gentle-ai-worker` failed twice with
  "assistant reported an error" and no detail, so both tasks were executed inline by the parent. T2
  produced two RED observations, and both were defects in the *tests*, not the implementation:
  (a) `Format::fixed(2).apply(1.005)` cannot be `1.01` because 1.005 is 1.00499999999999989 in
  binary — the expectation assumed decimal semantics; (b) the negative-zero property rejected 1024
  generated cases, so the strategy was rebuilt with `prop_flat_map` to generate only values that
  provably round to zero. Both corrected and the binary-value behaviour is now pinned by
  `format_rounds_the_binary_value_not_the_decimal_literal`.
- **L8 (slice boundary):** the T1+T2 slice measured 493 authored changed lines (mostly the crate
  scaffolds). Recorded here per the review-workload rule; T3 and T4 are split into their own
  commits so the geometry review stays small.
- **L9 (execution, T3–T4).** T3 came in at 1,390 lines and T4 at ~690: both are past the ~400-line
  review slice, so the review-workload risk is named here rather than discovered later. Mitigation
  applied from T5 on: one task per commit, and the remaining tasks are individually smaller. T3 can
  still be reviewed per file, since `point`/`arc`/`contour`/`chain` have no cross-dependencies beyond
  the shared error type.
  T4 exposed two genuine defects that the tests caught and that the implementation now explains:
  (a) `Contour::tessellate` did not close the ring, leaving a duplicated final vertex, which became
  a zero-length segment and then a *spurious* self-intersection that flattened every offset; (b)
  offsetting a circle built from two half arcs tried to intersect a circle with itself, because two
  concentric offsets are a smooth continuation, not a corner. Both are now handled at the root,
  with the reasoning recorded in the source. clippy also caught `Option::is_none_or` (stable in
  1.82) against the workspace MSRV of 1.80, which was the only lint failure.
- **L10 (execution, T5–T11).** Every task landed as its own commit with all gates green before the
  commit was made (`scripts/check.sh`: `fmt --check`, `clippy -D warnings`, `cargo test`). The suite
  stands at **123 tests across 13 suites**. Three implementation findings worth keeping:
  (a) T10 exposed a genuine kernel defect — shrinking a contour past its own inradius returned a
  valid-looking *mirrored* contour, so a tool that does not fit would have cut the wrong path; the
  offset now refuses when a shrinking offset's vertices sit far closer to the original boundary than
  the offset distance, and the reason is recorded in the source as a guard rather than a topological
  solution.
  (b) T11 pinned a property that matters for leads and tabs: the offset contour starts at the corner
  nearest the authored start vertex, so a lead-in does not move to a different corner when a tool
  radius is applied. A cam-geom test now locks that.
  (c) The `emit` contract refuses to invent a tool: without a library or `tools_ref` it exits 2
  naming the missing tool data, instead of defaulting to some diameter.
- **L11 (scope note).** M0 shipped exactly the acceptance criteria A1–A9 and no more: one strategy,
  no UI crate, no importers. The 16-block dialect is implemented but `FEED_RATE_CHANGE` is never
  emitted yet, because no M0 strategy changes feed rate inside a segment; the block is parsed and
  documented so M4 does not have to re-open the format.
