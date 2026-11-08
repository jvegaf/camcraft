# Feature: 2.5D CAM application — Milestone M4 (core toolpaths)

Program plan: `odd/roadmap.md` (M4 = "Core toolpaths"). This document is the executable specification
for M4 only. Read this file until `## Log` before implementing any task.
Stack: Rust workspace; `cam-model` (parameters), `cam-toolpath` (the strategies), `cam-cli` (the
contract). Branch: `feat/m4-toolpaths`, cut from `master` at `1bf8711`; baseline
`./scripts/check.sh` green with **550 tests** before the first write.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — "arranca M4" (this session's instruction).
- **S2** — Roadmap M4 row (verbatim, `odd/roadmap.md:141-143`): "Core toolpaths | Profile
  (inside/outside/on, tabs, passes, lead-in/out), Pocket (raster + offset, allowance, stepover, clear
  stepover), Drilling (peck, retract, drill bank) | The three strategies that define 2.5D routing;
  they exercise every post-processor block".
- **S3** — The parameter vocabulary M4 implements, verbatim: "`ProfileSide` (`Profile Inside` / `Profile
  Outside` / `Profile On`), `ProfilePass`, `PocketAllowance`, `PocketDepth`, `Pocket Pass`, `Pass Depth`,
  `stepover`, `clear_stepover`, `Safe Z`, `Start Depth`, `Cut Depth`, `Cut Rate`/`Plunge Rate`" — the
  fields a profile, pocket or drilling operation is addressed by, in the document and on the command line.
- **S4** — Strategy vocabulary recovered by REA (verbatim, `evidence/analysis-report.md:191-192`):
  "`2D Profile Toolpath`, `Pocket Toolpath`, `Drilling Toolpath`, `Drill Bank Toolpath`".
- **S5** — Tool cutting-data columns recovered by REA (verbatim, `evidence/analysis-report.md:96`):
  "stepover, clear_stepover", stored per tool next to "stepdown".
- **S6** — Rule 1 of the architecture, in force since M0 (verbatim, `odd/roadmap.md:95`): "**`cam-cli`
  is the contract.** Any behaviour that cannot be produced headless through the CLI is not considered
  testable, and therefore not done."
- **S7** — Rule 2 of the architecture, in force since M0 (verbatim, `odd/roadmap.md:98`):
  "**Determinism is a feature.** Identical input must produce byte-identical G-code. No `HashMap`
  iteration may reach an output path; ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S8** — Decision D6 (verbatim, `odd/roadmap.md:287`): "Arcs first-class, no pre-tessellation".
- **S9** — The plan's deferred list (verbatim, `odd/roadmap.md:152-153`): "rotary wrapping
  (`ROTARY_WRAP_X/Y` in the DSL is parsed but wrapping toolpaths are a later milestone), **drill-bank
  hardware integration**". This is what bounds S2's "drill bank": the *geometry and selection* half is
  in M4, the machine-command half is deferred by the plan itself.
- **S10** — Decision D2 (user, verbatim option label): "Todo menos 3D y Laser".
- **S11** — Decision D8 (verbatim, `odd/roadmap.md:295`): "`cam-cli` as the test surface".

Owner requirements still in force from earlier milestones:

- **S12** — Decision D1 (user, verbatim option label): "Rust + wgpu/egui (o Rust + Qt bindings)".

Everything else in this document is a **derived acceptance criterion** for M4 — an engineering
consequence of S1–S12 with no new scope. Each one is falsifiable and is the thing a task is checked
against.

### Derived acceptance criteria (M4)

- **A1** — `cam-model` grows exactly two new `Operation` variants, `Pocket(PocketParams)` and
  `Drilling(DrillParams)`, with `PocketStrategy::{Raster, Offset}` and `RetractMode::{Amount(Length),
  Full}`. They serialise as `{"kind":"pocket",…}` and `{"kind":"drilling",…}` next to the existing
  `profile`, round-trip losslessly through `c2d/document@1`, and an unknown `kind` stays a hard load
  error (never a skipped toolpath).
- **A2** — `Document::validate` treats pocket and drilling like profile: a toolpath citing an open
  contour is refused by name. Everything else it already enforces is unchanged, and every M0–M3
  document still loads with byte-identical behaviour.
- **A3** — Pocket (`pocket`) cuts a region in `Offset` and `Raster` mode with `allowance`, `stepover`,
  `pass_depth`, `start_depth` and `cut_depth`, at the S3 parameter names. The tool centre stays
  `radius + allowance` inside the boundary in both modes, so an allowance of `a` leaves exactly `a`
  millimetres of wall material (A3 is the falsifiable form of "PocketAllowance").
- **A4** — Offset pocket emits one closed ring per stepover, outermost first, until the inward offset
  collapses; rings keep their arcs as arcs (S8) and the ring count is reported. A tool that does not
  fit the region at all is refused by name (`OffsetCollapsed`), never emitted as a degenerate path.
  Corrected while implementing (L8): the offset kernel's own refusals do not end the ringing. A shrink
  wider than a feature can come back as a well-formed smaller ring with unchanged winding, after which
  the rings grow — a gouging ring and an endless loop. A ring therefore also has to prove that every
  vertex really sits the requested distance inside the boundary (D-M4-9).
- **A5** — Raster pocket emits parallel rows at `stepover` spacing spanning the region's extent, each
  row clipped to the region exactly: span ends are true line/arc intersections, not tessellated
  approximations, and a row whose clipping is ambiguous (tangent, or collinear with a boundary segment)
  is nudged and retried once or skipped — never emitted with an unpaired span. Two details settled by
  the implementation (L8): rows sit on the grid `min.y + k*stepover` and the last one is the last grid
  line that fits, so a 94 mm-tall region at 3 mm gives 32 rows ending at `min.y + 93` (the "…97" this
  criterion first stated was arithmetic that cannot be reached on that grid); and the retry nudge is
  `1e-5` mm, ten times the kernel tolerance, because retrying exactly at the ambiguity band's edge can
  come back ambiguous again.
- **A6** — Both pocket modes chunk depth with the same rule profile already uses (equal steps of
  `pass_depth`, the last pass exactly at `cut_depth`) and cut every ring/row at each depth, shallow to
  deep, retracting to `safe_z` between them.
- **A7** — Drilling (`drill`) plunges at the centre of each selected closed contour, refuses a
  non-circular contour by name, and orders the holes deterministically (nearest neighbour from the
  lowest-Y-then-lowest-X hole, ties broken by entity id) so the same document always produces the same
  program (S7).
- **A8** — Drilling `peck_depth > 0` plunges in pecks of at most `peck_depth`, retracting between them
  by `RetractMode::Amount(d)` or to `safe_z` for `RetractMode::Full`; `peck_depth == 0` is one plunge.
  Between holes the tool always rapids at `safe_z`.
- **A9** — Drill bank (S2, bounded by S9): when `DrillBank { count, pitch }` is set, each hole names the
  bank drill that cuts it and the emitted head position is the hole compensated by that drill's offset;
  the selection keeps the head nearest the previous head position, is deterministic, and is reported per
  hole. Corrected while implementing (L7): the head offsets only upwards from the hole, so what is
  bounded is the incremental head step, not the head-to-hole distance — a hole farther than
  `(count-1)*pitch` from every drill is served by the last one. Machine-specific
  bank commands (a `G` code, an `M` code, a toolchange per drill) are **not** emitted: S9 defers
  drill-bank hardware integration, and the move vocabulary of `cam-move` has no bank command.
- **A10** — `cam-cli toolpath <doc.c2d> --kind profile|pocket|drilling …` appends one toolpath to an
  existing document and writes it back (`--out`, default in place), taking `--entities`, `--tool`,
  `--id`, `--name` and the kind's parameters; omitted `--stepover`/`--pass-depth` fall back to the
  tool's `stepover`/`stepdown` from the library (S5), and `--clear` falls back to `clear_stepover`.
  Bad input exits `2` and never leaves a half-written document.
- **A11** — `cam-cli inspect` names the new operations (`pocket (raster)`, `pocket (offset)`,
  `drilling`, `drilling (bank)`), and `cam-cli emit` produces a program for them through the existing
  move vocabulary — every strategy reaches G-code through the contract (S6, S11).
- **A12** — Golden G-code: `emit` on a document with a pocket and a drilling toolpath produces
  byte-identical output on every run and after a document save/load round trip.
- **A13** — Every earlier milestone is unchanged: the whole existing suite stays green, `cam-cli emit`
  output for a profile-only document is byte-identical, and `import`/`check`/`inspect`/`postp`/`tools`/
  `validate` behave exactly as before.
- **A14** — The temporary `ToolpathError::NotImplemented` placeholder used while the two strategies are
  built in isolated worktrees is gone from the final tree, proven by a test that asserts the error set
  the strategies can produce, so M3's L6 defect (a stub surviving as a contract) cannot repeat.

### Decisions recorded for M4

- **D-M4-1 — New `Operation` variants stay inside `c2d/document@1`.** `cam-model`'s module docs call a
  new kind "a schema-version event"; taken literally that would bump the tag for every strategy in
  M6–M8 as well. The tag exists to protect the *meaning of existing fields*, and the enumerations are
  internally tagged, so an older build already fails loudly by name on an unknown `kind` instead of
  silently skipping the toolpath. M4 therefore adds variants without touching the tag, and the rule is
  restated precisely in `crates/cam-model/src/lib.rs` and `docs/format.md`. Revisit trigger: any change
  to a field's meaning or units, which *is* a `@2`.
- **D-M4-2 — A pocket is one region per selected contour; islands are out of scope.** Pocketing closed
  vectors while treating a closed vector inside another as an island needs boolean subtraction on
  arc-bearing contours. `cam-geom` has offsetting, intersecting and validation but no
  boolean ops (they are named in the roadmap's crate description, not built). Island-free pockets are
  what M4 delivers; a nested selection pockets each contour separately and the CLI says so. Recorded as
  a known limit, not a silent behaviour.
- **D-M4-3 — `stepover` is a toolpath parameter, not a `ToolSpec` field.** The same tool pockets
  differently in MDF and in hardwood, and `PocketParams` is where S3 puts `stepover`. The library's
  `stepover`/`clear_stepover` are *defaults* the CLI fills in when the flag is omitted (A10), which is
  also why adding fields to `ToolSpec` — and churning every existing profile test — is avoided.
- **D-M4-4 — `clear_stepover` is the default for a clearing pocket, not a second tool inside one
  toolpath.** In the recovered schema `clear_stepover` lives in `tool_cutting_data`, i.e. it is a
  per-tool rate like `stepdown`, and "clear with a big tool, finish with a small one" is expressed as two
  toolpaths (`tool_entity` joins a tool to a material/machine; `PocketAllowance` is per
  toolpath). `cam-cli toolpath --kind pocket --clear` therefore reads `clear_stepover` instead of
  `stepover`; no two-tool toolpath is introduced. Confirmed as the reading of S5 only; the observable
  semantics of that "Clear stepover" field are **[U]** and this doc does not
  claim otherwise.
- **D-M4-5 — Drilling accepts circles only.** A hole is the centre of a closed contour whose segments
  are all arcs about one centre, the same construction `primitive::circle` produces. Any other closed
  contour is refused by name rather than drilled at a guessed "geometric centre", because a hole in the
  wrong place is a ruined part. A non-circular closed contour has no single centre to drill at, so
  refusing it is a rule rather than a fallback.
- **D-M4-6 — Linking moves stay conservative.** Between rings, rows and holes the tool retracts to
  `safe_z` and rapids. Staying down for the next ring (the usual CAM optimisation) needs gouge-free
  linking through material and belongs with M5's simulation, which can prove it. Cost recorded: more
  rapids than a hand-written program.
- **D-M4-7 — Raster rows run along X at the given stepover; no raster angle in M4.** S2 names "raster +
  offset", not raster direction. A `raster_angle` is an additive parameter for a later milestone.
- **D-M4-9 — A ring must prove it is the offset it claims to be.** `cam-geom`'s offsetter refuses an
  offset that crosses the shape's interior, but not every over-shrink is refused: a 3 mm square inset
  by 2 mm comes back as a well-formed 1 mm ring with its winding unchanged, and the next ring grows.
  Trusting that would emit a ring that cuts the wall and never terminate. Every ring is therefore
  checked against the original boundary (each vertex at least `distance − 0.002 mm` away, the slack
  being one chord sagitta of the kernel's 0.001 mm flattening tolerance), and the same check decides
  whether the raster inset region is usable. This is a guard in the strategy, not a fix in the kernel:
  the kernel limit stays recorded in the roadmap (§4.2) and belongs to whoever builds arrangement-based
  offsetting.
- **D-M4-10 — A raster row with several spans retracts between spans, not only between rows.** A row
  crossing a non-convex region twice cannot link its spans at depth without crossing material, so
  D-M4-6's rule applies inside a row as well. Cost: more rapids on concave pockets.
- **D-M4-8 — No toolpath UI in M4.** The roadmap's deliverable for M4 is the strategies and their
  blocks, and S6 makes the CLI the contract; the toolpath panel is UI work that rides on `cam-app`
  commands, which do not exist for toolpaths yet. Recorded so the missing panel is a decision, not a
  gap: the shell keeps its authoring surface from M2, and `cam-cli toolpath` is how a toolpath is
  created in this milestone.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **T1** — A1, A2, A10, A11 (inspect/emit dispatch), A13, A14 (stub set-up) — inline — three
  commits: `docs(m4)`, `feat(model)`, `feat(cli)`
- **T2** — A3, A4, A5, A6 — writer (isolated worktree, `crates/cam-toolpath/src/pocket.rs` +
  `tests/pocket.rs`) — `feat(toolpath): offset and raster pocket strategies`
- **T3** — A7, A8 — writer (isolated worktree, `crates/cam-toolpath/src/drill.rs` + `tests/drill.rs`)
  — `feat(toolpath): drilling with peck and retract`
- **T4** — A9 — inline — `feat(toolpath): drill bank selection and head compensation`
- **T5** — A12, A14 — inline — `test(cli): golden g-code for pocket and drilling`
- **T6** — docs — inline — `docs(m4): pocket and drilling parameters, and roadmap evidence`

Route notes: T1 is the shared surface (model, CLI, dispatch) and must land first, because T2 and T3 are
two independent units of behaviour over disjoint module + test surfaces and are therefore the writer
reason (parallel units, isolated worktrees — the M3 L4 pattern). T4 builds on T3's module and stays
inline; T5 and T6 depend on all of it.

### Task detail

**T1 — Model, CLI, dispatch.** In `crates/cam-model/src/lib.rs`:

```rust
pub enum PocketStrategy { Raster, Offset }          // serde rename_all = "snake_case"
pub struct PocketParams { pub strategy: PocketStrategy, pub allowance: Length,
                          pub stepover: Length, pub pass_depth: Length,
                          pub start_depth: Length, pub cut_depth: Length }
pub enum RetractMode { Amount(Length), Full }        // serde rename_all = "snake_case", internally tagged
pub struct DrillBank { pub count: u32, pub pitch: Length }
pub struct DrillParams { pub peck_depth: Length, pub retract: RetractMode,
                         pub start_depth: Length, pub cut_depth: Length,
                         pub bank: Option<DrillBank> }
pub enum Operation { Profile(ProfileParams), Pocket(PocketParams), Drilling(DrillParams) }
```

`Default` for both new param structs: pocket `Offset`, allowance 0, stepover 3 mm, pass depth 6 mm,
start 0, cut 18; drilling peck 0 (single plunge), `RetractMode::Full`, start 0, cut 18, no bank. The
`RetractMode` serde shape is `{"mode":"amount","amount":1.0}` / `{"mode":"full"}` — a struct-like
variant because a tagged newtype over a bare number cannot carry the tag (L3). `Document::validate` refuses an open contour for every operation
that cuts one (generalise the existing profile check and its error message; keep the error variant
name). D-M4-1's restatement goes into the crate docs, and `docs/format.md`'s `Extensibility` row is
updated in T6.

In `crates/cam-toolpath`:

```rust
pub mod pocket;   // T2 fills it
pub mod drill;    // T3 fills it
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ToolpathError { /* …existing… */,
  #[error("stepover must be greater than zero")] InvalidStepover,
  #[error("peck depth must not be negative")] InvalidPeckDepth,
  #[error("entity {entity:?} is not a circle, so it has no drill centre")] NotACircle { entity: String },
  #[error("drill bank needs at least one drill and a positive pitch")] InvalidBank,
  #[error("not implemented yet")] NotImplemented,   // temporary: removed in T5
}
```

`pocket.rs` starts as `pub struct PocketRequest<'a> { pub contour: &'a Contour, pub params:
&'a PocketParams, pub tool: &'a ToolSpec, pub job: &'a Job }`, `pub struct Pocketpath { pub moves:
Vec<Move>, pub rings: usize }` and `pub fn pocket(request: &PocketRequest<'_>) -> Result<Pocketpath,
ToolpathError>` returning `Err(ToolpathError::NotImplemented)`. `drill.rs` starts as `pub struct
DrillRequest<'a> { pub holes: &'a [DrillHole], pub params: &'a DrillParams, pub tool: &'a ToolSpec,
pub job: &'a Job }` with `pub struct DrillHole { pub entity: String, pub centre: Point2 }`, `pub struct
Drillpath { pub moves: Vec<Move>, pub order: Vec<String>, pub drill_index: Vec<Option<u32>>, pub
rapid_length: Length }` and `pub fn drill(...)  -> Result<Drillpath, ToolpathError>` likewise. Also
`pub fn circle_centre(contour: &Contour) -> Option<Point2>` (all segments arcs, one common centre, total
sweep ≈ ±360°) and a shared `pub(crate) fn pass_depths(start: f64, cut: f64, step: f64) ->
Result<Vec<f64>, ToolpathError>` extracted from `profile` so both new strategies chunk depth the same
way (A6) without copying the rule.

`cam-cli` gains `toolpath` (A10) and routes `build_segment` by operation kind, calling
`cam_toolpath::pocket::pocket` / `cam_toolpath::drill::drill`, plus `circle_centre` to build
`DrillHole`s (the entity id travels with the hole so a refusal and the drill-bank report can name it).
`inspect` prints the operation with its strategy per A11. Tests: model serde round trip for both new
kinds including `RetractMode` and `bank`; an unknown `kind` still fails to load; an open contour in a
pocket or drilling toolpath is refused naming the entity; `toolpath --kind profile` on a fixture
document appends and reloads; `toolpath --kind pocket` without T2 implemented exits with the
`NotImplemented` message (this test is replaced in T5 by the real golden test); `--entities` naming a
missing id exits `2` leaving the file untouched; `inspect` on a document with all three kinds prints
the three expected lines; defaults come from the tool library when `--stepover`/`--pass-depth` are
omitted, and `--clear` reads `clear_stepover`.

**T2 — Pocket.** `pocket::pocket` implements A3–A6 in `crates/cam-toolpath/src/pocket.rs`, tests in
`crates/cam-toolpath/tests/pocket.rs`. Non-negotiables: the inset is `diameter/2 + allowance` for both
modes; arcs stay arcs in offset rings (S8); row/ring geometry is computed once and cut at every depth
pass (A6); every retract goes to `job.safe_z` (D-M4-6); ordering is explicit and stable (S7).
Practical shape of the two modes:

```text
Offset: for k = 0, 1, 2, … offset the boundary by -(inset + k*stepover) with
        cam_geom::offset::offset_contour; stop at the first k whose offset is refused
        (that is the collapsed centre); k = 0 refused → ToolpathError::OffsetCollapsed.
        rings = that list, outermost first.
Raster: offset the boundary by -(inset) once → region (refused → OffsetCollapsed);
        rows at y = region.min.y + k*stepover while y <= region.max.y (last row clamped
        to max.y); for each row, intersect a long horizontal line with every segment of
        region through cam_geom::intersect::intersect, take the x of every returned
        point, sort, dedupe within the offset tolerance, pair them (even-odd) into spans.
        Odd point count (tangency) or a Coincident result → retry once at y + 1e-6,
        then skip the row.
Both:  moves = for each depth in pass_depths(start_depth, cut_depth, pass_depth) {
          for each ring/row { Rapid to its start at safe_z; Feed plunge to -depth;
                              walk it (arcs as arcs); Retract to safe_z } }
        rows alternate direction (zigzag) so consecutive rows connect at the same end;
        Pocketpath.rings reports the ring/row count.
```

Tests (every one asserts moves, not internals): a 100x100 square with a 6 mm tool and 4 mm stepover
yields 12 offset rings at insets 3, 7, … 47, each a rectangle of side `100 - 2*inset`; the same square
rastered gives 32 rows at y = 3, 6, … 96 spanning x 3…97; a 5 mm pass depth on an 12 mm cut gives passes
`[5, 10, 12]` and every ring/row appears once per pass; `allowance = 1.0` moves every transition 1 mm
inward; a 4 mm tool in a 3 mm pocket is refused as `OffsetCollapsed`; a circle's raster spans match the
analytic chord length of the inset circle within 1e-6; a raster row exactly tangent to a circle produces
no unpaired span; an arc-bearing rounded rectangle keeps `Move::Arc` moves in its offset rings and none
in its raster rows; a zero stepover is refused as `InvalidStepover`; an open contour is refused; the
`z` of every cutting move is exactly `-depth` of its pass.

**T3 — Drilling.** `drill::drill` implements A7 and A8 in `crates/cam-toolpath/src/drill.rs`, tests in
`crates/cam-toolpath/tests/drill.rs`. Ordered holes: start at the hole with the lowest `y` (ties:
lowest `x`, ties: entity id), then repeatedly take the nearest unvisited hole (ties: entity id) —
`Vec` scans, no `HashMap` (S7). Per hole: `Rapid` to the hole at `safe_z`; plunge from `start_depth` to
`cut_depth` in steps of `peck_depth` (a single plunge when `peck_depth.mm() == 0`); between pecks a
`Retract` to `-(plunge_depth) + amount` for `Amount`, or to `safe_z` for `Full`; and always `Retract` to
`safe_z` before the next hole. `Drillpath.order` is the entity ids in cutting order, `drill_index`
is `None` for every hole in T3 (T4 fills it) and `rapid_length` is the summed XY length of the moves to
holes. Tests: four holes on a square cut in nearest-neighbour order with the exact expected `order`
vector; `peck_depth = 3` on a 7 mm cut produces plunge moves at `-3, -6, -7` with the retracts between
them at the expected Z for both `RetractMode`s; `Full` retracts reach `safe_z` and `Amount(1.0)` does
not; a single `peck_depth = 0` is one plunge; a square contour is refused as `NotACircle` naming the
entity; a negative peck depth is refused; a 0-hole request is refused; `circle_centre` accepts a
two-half-turn circle and the 4-arc circle `primitive::circle` builds, and rejects a rounded rectangle;
two runs on the same input produce identical `Vec<Move>`; `rapid_length` equals the hand-computed
nearest-neighbour tour length for the square.

**T4 — Drill bank.** In `drill.rs`, when `params.bank` is `Some(DrillBank { count, pitch })`, every hole
is assigned the drill index `k ∈ 0..count` whose offset `k * pitch` keeps the head nearest the previous
head position (first hole: nearest the machine origin), and the emitted XY is `hole.x - k*pitch`,
`hole.y`. `count == 0` or `pitch <= 0` is `InvalidBank`. `drill_index` reports `Some(k)` per hole and
`Drillpath` gains `bank_travel: Length` (the head's XY travel) so the CLI can print the selection.
`inspect` prints `drilling (bank)` and `cam-cli toolpath --kind drilling --bank <count>:<pitch>`
accepts the option. Tests: a bank of 3 drills at 32 mm pitch picks the expected index for holes at
x = 0, 10, 40 and 100 and emits the compensated x; the head never moves further than `(count-1)*pitch/2`
from a hole along X; a second run is identical; `count = 0` and `pitch = 0` are refused; a bank of one
drill at any pitch emits exactly the un-banked program.

**T5 — Golden G-code, and the stub removed.** `crates/cam-cli/tests/cli.rs` gains: a document with one
offset pocket and one drilling toolpath emitted through `tests/fixtures/post_mm.pp` asserted **byte for
byte** (the whole program as one expected string), the same assertion after a save/load round trip, and
`cam-cli toolpath --kind pocket`/`--kind drilling` end to end from a fixture document through `emit`.
`ToolpathError::NotImplemented` is deleted; no test ever pinned it (L4), and a test asserts the
strategies' error set explicitly (A14). The profile-only golden from M0 is unchanged (A13).

**T6 — Docs.** `docs/m4-toolpaths.md`: the two strategies and what each produces, the parameter table
with the S3 names, the allowance/stepover/step-down rules, peck/retract semantics, drill-bank selection
and its deferral of machine commands (A9, S9), the known limits from D-M4-2, D-M4-5, D-M4-6, D-M4-7 and
D-M4-8, and the `cam-cli toolpath` synopsis with its defaults from the tool library. `docs/format.md`
gains the two `operation` examples, the `RetractMode` serde shape and the restated `Extensibility` row
(D-M4-1). `odd/roadmap.md` marks M4 delivered with its evidence table and the deviations, and the
`docs/format.md` command-line contract table gains the `toolpath` row.

## Delivery slices

| Slice | Tasks | Subject |
| --- | --- | --- |
| 1 | T1 | Model parameters, CLI `toolpath`, dispatch, stubs — **delivered** (`61e1df5`, `a263b28`, `9eb1326`) |
| 2 | T2, T3, T4 | The two strategies plus the drill bank, in parallel over isolated worktrees — **delivered** (`432e0d4`, `5201ab4`, `229a68d`) |
| 3 | T5, T6 | Golden G-code, docs — **delivered** (`11ce793`, `7c61d80`), plus the closing fixes in `26b23f7` |

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| T1 | `a263b28`, `9eb1326` | 23 tests (6 model, 8 `circle_centre`, 9 CLI binary). The plan is its own commit `61e1df5`, so the slice is plan + model + CLI rather than one 1750-line unit. Model: both operations round-trip, `RetractMode::Amount` serialises as `{"mode":"amount","amount":…}`, an absent bank is omitted, both new kinds refuse open geometry, an unknown `kind` still fails to load. Kernel: `circle_centre` accepts 2 and 4 arc circles either winding and refuses lines, mixed shapes, open contours and arcs with different centres; `pass_depths` extracted so the strategies share one depth rule. CLI: `toolpath` appends and reloads, ids are deterministic, every refusal exits 2 with the file byte-identical, library `stepover`/`stepdown`/`clear_stepover` supply the defaults, `--out` leaves the input alone, `inspect` names each kind and strategy |
| T2 | `35cdf4f` (merge `432e0d4`) | 12 tests. 12 rings at insets 3, 7, … 47 and 32 rows at y = 3, 6, … 96 on a 100 mm square with a 6 mm tool; `[5, 10, 12]` pass chunking with every ring once per pass; allowance shifts every ring and row by exactly 1 mm; analytic chord widths on a circle within 1e-6; arcs kept in rings and absent from rows; tangent row emits no unpaired span; refusals by name. Four spec disagreements found and fixed in the spec rather than worked around (L8) |
| T3 | `1382e97` (merge `5201ab4`) | 13 tests. Nearest-neighbour tour with a document-space seed (lowest y, then lowest x, then id; ties within 1e-9 by id) over `Vec` scans; pecks reuse `pass_depths` so the last one lands exactly on `cut_depth`, and `peck_depth == 0` bypasses it for a single plunge; one `Retract` per peck, the last always at `safe_z`; `rapid_length` measured in machine space from `(0,0)`. Square test observed `order = [h1,h2,h3,h4]` and `rapid_length = 110.0 mm`. The T3 test list asked for a `NotACircle` refusal here, but `drill()` never sees contours — `cam-cli` resolves them through `circle_centre` first and `tests/drill_targets.rs` already covers the square — so it stays a CLI-layer error and is not duplicated |
| T4 | `9fa671e` (merge `229a68d`) | 7 bank tests added, the T3 placeholder `a_drill_bank_is_ignored_in_this_task` removed because it pinned exactly what T4 reverses; `drill_index = [Some(0), Some(0), Some(1), Some(2)]` for holes at x = 0, 10, 40, 100 with 3 drills at 32 mm, `bank_travel = 60 mm` against `rapid_length = 120 mm`. A differential check against the base commit showed the un-banked program byte-identical through `toolpath`, `emit` and `inspect`, with banked `emit` differing only in the compensated X |
| T5 | `11ce793` | 5 tests: `tests/errors.rs` (exhaustive `match`, exact `Display` per variant, "no refusal says not implemented") and three whole-program goldens (as built, after a save/load round trip, and with both toolpaths authored through `cam-cli toolpath`). `NotImplemented` deleted; the M0 profile golden untouched. Its RED for A14 was observed by re-adding the variant twice — once to break exhaustiveness, once to break the message rule |
| T6 | `7c61d80` | `docs/m4-toolpaths.md` (318 lines: operations, parameter mapping, inset/ring/raster/depth/linking rules, drilling, drill bank, six known limits with their decisions, the error table, the `toolpath` synopsis with every default's source, three worked examples); roadmap M4 row marked delivered plus §4.6 with the evidence table and the corrections; `docs/format.md`'s `References` row generalised from profile to all three cutting operations. Every CLI transcript in the doc was replayed against the real binary |

## Log

- **L1** — Owner instruction (verbatim, this session): "arranca M4". Read as: author the M4 feature
  document under ODD and start implementing it on a feature branch off `master`.
- **L2** — Plan: 6 tasks, branch `feat/m4-toolpaths` from `master` (`1bf8711`). Baseline
  `./scripts/check.sh` green with 550 tests before the first write. Plan commit `61e1df5`.
- **L3** — Slice 1 evidence: T1 in `a263b28` (model + kernel spine) and `9eb1326` (CLI), 23 tests, 573
  total, gates green. Three corrections found by the tests themselves rather than by reading:
  `RetractMode` cannot be a tagged **newtype** over `Length`, because a bare number cannot carry the
  tag — it is a struct-like variant with an `amount` field; the first `circle_centre` tests used
  `tan(π/4)` where a quarter turn needs `tan(π/8)`, so the "circle" they built was two half turns and
  the test was measuring itself, not the rule; and the `toolpath` parser advanced two tokens for a
  positional argument, so the document path swallowed its first option. RED/GREEN was observed per
  A2 (the pocket arm removed from the validate match fails its test) and per A10 (all nine CLI tests
  failed with the command missing before it was written).
- **L4** — Scope correction recorded instead of a stub test: T1 does **not** add the temporary
  "`pocket` exits with `NotImplemented`" test the task detail proposed. Pinning a stub is exactly the
  M3 L6 defect, and A14 already requires the placeholder to be gone by T5. The stub is covered by
  T2/T3 replacing it and by T5's golden test, and `ToolpathError::NotImplemented` is removed there.
- **L6** — Slice 2 evidence, drilling: T3 in `1382e97`, merged `5201ab4`; 13 tests, 586 total, gates
  green. RED was observed for real: all 13 tests failed against the stub with "not implemented yet".
  One spec deviation kept rather than papered over: `drill()` receives `DrillHole`s, not contours, so
  the "square is refused as `NotACircle`" case cannot live in `tests/drill.rs`; it is pinned where the
  resolution actually happens, in `tests/drill_targets.rs` and the CLI layer. The tour's seed is taken
  in document space, which differs from machine space only for `TopLeft`, and only for which hole goes
  first — distances are identical under any origin transform.
- **L7** — Slice 2 evidence, drill bank: T4 in `9fa671e`, merged `229a68d`; 7 tests, 592 total, gates
  green. Two spec wordings were wrong and are corrected in place rather than worked around: A9's
  "the head never moves further than `(count-1)*pitch/2` from a hole" cannot hold when the emitted
  position is `centre.x - k*pitch` with `k >= 0` (a hole 92 mm from the origin with 2×32 mm reaches the
  last drill 64 mm away), so the bound tested is the incremental head step; and the T3 test that
  asserted a bank is ignored had to be deleted, not adapted, because it pinned the behaviour this task
  exists to reverse. `Drillpath` also grew `bank_travel`, because with a bank `rapid_length` (distance
  between holes) and the program's actual rapid travel are different numbers, and conflating them would
  have made the report lie. The CLI's selection summary is best-effort by construction: it prints the
  real strategy's answer when the tool and geometry allow it and says why otherwise, while the append
  itself cannot fail on it.
- **L8** — Slice 2 evidence, pocket: T2 in `35cdf4f`, merged `432e0d4`; 12 tests, 604 total, gates
  green. RED was real (all 12 failed against the stub). Four spec defects were found by implementing
  rather than by reading, and all four are corrected above instead of quietly satisfied:
  (a) the ring stop rules were insufficient — a shrink wider than a feature returns a valid smaller
  ring with unchanged winding and the rings then grow, so D-M4-9 adds a distance proof, without which
  a 3 mm square pocketed by a 4 mm tool emits a wall-gouging ring and loops forever;
  (b) the raster row example said "32 rows at y = 3, 6, … 96", arithmetic that cannot hold on the
  declared grid (32 rows end at 96);
  (c) the retry nudge `1e-6` equalled the kernel tolerance, so a retried row could land back on the
  ambiguity band and be dropped for nothing — it is now `1e-5`;
  (d) "a 4 mm tool in a 5 mm pocket is refused" is false: a 2 mm inset leaves a 1 mm region the tool
  fits, which is an ordinary pocket with uncleared corners. The refusal test uses a 3 mm pocket.
  Also recorded: rows sit exactly on the inset region's lower edge, so the first row is the one that
  needs the retry, and `Pocketpath.rings` counts rows that emitted at least one span.
- **L9** — Slice 3 evidence: T5 `11ce793` (the stub deleted, the error contract, three whole-program
  goldens: 609 tests) and T6 `7c61d80` (the guide and the roadmap evidence), both merged. The golden
  program is pinned as one 42-line string asserted whole, and it was read line by line against the
  geometry before being pinned — ring insets 3 and 9, arc centres equal to the contour's, both rings at
  Z-6.0000, holes in tour order with pecks at -3 and -6.
- **L10** — Three gaps the writers flagged were closed at the root rather than documented as warts.
  (1) `ToolpathError::OpenContour` said "profile toolpaths need closed geometry" while pocket and
  drilling return it too; it now says "cutting toolpaths need closed geometry", which is a deliberate
  reword of an observable message, recorded here because A13 asks earlier behaviour to stay unchanged —
  a message about the wrong operation is the same class of defect M0 and M3 both fixed at the root.
  (2) A drilling toolpath over a rectangle was accepted by `toolpath` and refused only later by `emit`,
  so A10's "bad input exits 2" did not cover it; the append now resolves every entity through
  `circle_centre` and refuses before writing (D-M4-5), with a test that fails if the check is removed.
  (3) The feature document's own task text still carried the two examples its criteria had corrected
  (the 4 mm tool in a 5 mm pocket, and the bank's head-to-hole bound); both now match L7 and L8.
- **L5** — Slice 2 launched as two writers in isolated worktrees (`../campilot-m4-pocket` on
  `m4/pocket`, `../campilot-m4-drill` on `m4/drill`, both from `9eb1326`), because pocket and drilling
  are two independent units of behaviour over disjoint module and test surfaces.
- **L11** — A5's lattice is superseded, by the owner's decision recorded in
  `odd/tasks/raster-far-edge-pass.md`. M5's differential found what "the last one is the last grid line
  that fits" costs: on a 100 mm square at a 4 mm stepover the rows stop short of the offset region's far
  edge and the strip the tool's own disc could have cut is never cut, `uncut: 984.000 mm3
  over 164 cell(s)` (`odd/tasks/m5-simulation.md` D-M5-12, L10). The parent offered that as a
  decision because delivering it changes this document's contract, and the owner answered verbatim
  **`si, acepto`**; the contract changes. `raster_rows` now finishes the lattice with exactly one row on
  `region.max.y` when the grid `min.y + k*stepover` stops short of it, and emits no extra row when a grid
  row already lands there, so A5's "32 rows ending at `min.y + 93`" is no longer the behaviour: the
  94 mm-tall region at 3 mm gives **33 rows** (3, 6, … 96 on the grid plus the finishing row at 97), the
  island fixture 33 rows and 41 spans, the full-width band 25 and 25, the narrow strip 15 and 25, and the
  radius-17 circle 10 rows — 9 on the grid plus a tangent finishing row whose retried chord is
  about 0.037 mm. The no-duplicate half is pinned where it can fail: the 1…99 lattice keeps its 50 rows and
  every raster program whose far edge already sits on the grid stays byte-identical. A5, the T2
  pseudo-code's "last row clamped to max.y" and the T2 test line above are left as written — this log is
  a period record, and the superseded lattice is exactly what those entries recorded.
