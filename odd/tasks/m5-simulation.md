# Feature: the reference CAM application-class CAM application — Milestone M5 (simulation & 3D preview)

Program plan: `odd/roadmap.md` (M5 = "Simulation & 3D preview"). This document is the executable
specification for M5 only. Read this file until `## Log` before implementing any task.
Stack: Rust workspace; new `cam-sim` crate for the material-removal model, `cam-cli` for the contract,
`cam-view` and `cam-ui` for the 3D surface and its animation. Branch: `feat/m5-simulation`, cut
from `master` at `0d96b36`; baseline `./scripts/check.sh` green with **707 tests** before the first write.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — "m5" (this session's instruction).
- **S2** — Owner's choice of 3D backend (verbatim option label and description, this session):
  "egui mesh, cero dependencias nuevas" — "Proyección y sombreado propios sobre egui::Mesh. Testeable
  headless, determinista. wgpu queda como trigger de performance registrado."
- **S3** — Owner's choice of shell scope (verbatim option label and description, this session):
  "Sólo simulación + preview" — "M5 = cam-sim + reporte + vista 3D + animación + `cam-cli simulate`.
  La ventana muestra el preview del programa ya autorado por `cam-cli`."
- **S4** — Roadmap M5 row (verbatim, `odd/roadmap.md:142`): "Simulation & 3D preview | Heightfield
  material removal, gouge/uncut reporting, 3D stock view, animated preview | G-code without simulation
  is unverifiable by the user; also the only honest acceptance test for M4–M7".
- **S5** — Roadmap crate description for `cam-sim` (verbatim, `odd/roadmap.md:76-77`):
  "`cam-sim/` Material-removal simulation: heightfield (dexfuture: dexel) + per-tool removal; reports
  gouges and uncut material".
- **S6** — Roadmap verification rule 5 (verbatim, `odd/roadmap.md:411-412`): "**Simulation as oracle** —
  for each strategy, a test asserts the simulated removed volume matches the analytic expectation
  within tolerance; a too-shallow pocket fails the test, not a screenshot."
- **S7** — Roadmap risk register (verbatim, `odd/roadmap.md:429`): "Simulation performance on large
  jobs | UI stalls | Tiled heightfield + `rayon`, resolution decoupled from job size".
- **S8** — Rule 1 of the architecture, in force since M0 (verbatim, `odd/roadmap.md:90`): "**`cam-cli`
  is the contract.** Any behaviour that cannot be produced headless through the CLI is not considered
  testable, and therefore not done."
- **S9** — Rule 2 of the architecture, in force since M0 (verbatim, `odd/roadmap.md:93`):
  "**Determinism is a feature.** Identical input must produce byte-identical G-code. No `HashMap`
  iteration may reach an output path; ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S10** — What the README promises for M5 (verbatim, `README.md:172-175`): "M5 is simulation:
  heightfield material removal, gouge and uncut reporting, a 3D stock view and an animated preview. It
  is the only honest acceptance test for the strategies that already ship — a green suite says the
  toolpath satisfies its rules, not that the part came out right — and it is what the deliberately
  conservative linking between rings and rows was waiting for."
- **S11** — Decision D-M4-6, which names M5 as its own closure condition (verbatim,
  `odd/tasks/m4-toolpaths.md:145-148`): "**D-M4-6 — Linking moves stay conservative.** Between
  rings, rows and holes the tool retracts to `safe_z` and rapids. Staying down for the next ring (the
  usual CAM optimisation) needs gouge-free linking through material and belongs with M5's simulation,
  which can prove it. Cost recorded: more rapids than a hand-written program."
- **S12** — Decision D-M1-2, the deviation this milestone inherits (verbatim,
  `odd/tasks/m1-editor.md:143-148`): "**D-M1-2 — The canvas is painted with `egui`'s painter in
  M1; the draw list is backend-agnostic.** The roadmap named `wgpu` for `cam-view`. Splitting the
  *scene* (which is testable math) from the *painter* keeps the wgpu decision intact while making M1
  verifiable without a GPU: `cam-view` emits `Scene`, and a wgpu backend consumes the same `Scene` when
  the 3D simulation (M5) actually needs the GPU. Revisit trigger: entity counts that stall the CPU
  painter, or M5's heightfield view — whichever comes first."
- **S13** — The move vocabulary's Z convention (verbatim, `crates/cam-move/src/lib.rs:20-21`): "Z
  position, in millimetres. Positive is up; cutting depths are positive magnitudes." The strategies
  emit a cutting move at `z: Length::from_mm(-depth)` (`crates/cam-toolpath/src/lib.rs:184`), so the
  stock's top surface is `z = 0` and its bottom is `z = -job.thickness`.
- **S14** — M4's decision on where a toolpath is authored (verbatim, `odd/tasks/m4-toolpaths.md:163-166`):
  "**D-M4-8 — No toolpath UI in M4.** The roadmap's deliverable for M4 is the strategies and their
  blocks, and S6 makes the CLI the contract; the toolpath panel is UI work that rides on `cam-app`
  commands, which do not exist for toolpaths yet. Recorded so the missing panel is a decision, not a
  gap: the shell keeps its authoring surface from M2, and `cam-cli toolpath` is how a toolpath is
  authored."

Owner requirements still in force from earlier milestones:

- **S15** — Cross-platform Windows/Linux (D1, owner decision at plan time).
- **S16** — Decision D2 (user, verbatim option label): "Todo menos 3D y Laser". This bounds S4: the 3D
  here is a *preview of the stock*, not the deferred 3D machining strategies.
- **S17** — Decision D7 (verbatim, `odd/roadmap.md:294`): "Do not vendor the vendor `.pp` files".

### Derived acceptance criteria (M5)

What "M5 delivered" means, testable through `cam-cli` unless the criterion names the window (S8):

- **A1 — One heightfield per job.** A grid of cells covers the job's XY extent; each cell holds the
  remaining stock's top surface in millimetres, initialised to the stock top (`0`) and clamped at the
  stock bottom (`-job.thickness`). Cell indexing is row-major and deterministic; the grid carries its
  own origin and cell size, so no query depends on a global tolerance.
- **A2 — Removal is swept, and arcs are cut as arcs.** A cutting move (`Feed` or `Arc`) removes material
  to its `z` for every cell whose centre lies inside the tool's swept footprint along that move; an
  `Arc` is swept along its own circle (centre and radius from the move), never along its chord. A cell
  is never raised, and no move removes material above `z = 0`.
- **A3 — Three volumes, one tolerance.** The simulation reports removed volume, remaining volume and
  the count of through-cut cells; every report is derived from the same cell set in one deterministic
  order.
- **A4 — A rapid below the stock top is a crash, not a cut.** Any `Rapid` that ends below the current
  surface is reported with its move index, position, and toolpath name. It is never silently treated as
  a cutting move: a program that rams the tool must fail the simulation.
- **A5 — One target surface, two deviations.** The simulation derives, from the document alone, the
  depth each cell is *authorised* to lose: the pocket region (boundary minus islands) to the pocket's
  final depth less the wall its allowance keeps, each hole's circle to the drilling depth, each
  profile's swept band to the profile's final depth, minus the material a tab declares it keeps, and
  **zero everywhere else**. Then gouge is
  "removed deeper than authorised" and uncut is "authorised deeper than removed", each reported as a
  volume and a cell count, per toolpath and in total.
- **A6 — Resolution is decoupled from the job, and bounded.** The cell size is a caller's parameter
  with a documented default; a request that would exceed the grid's cell cap is refused by name,
  naming the requested size, the grid it implies and the cap — never an allocation attempt. Refining
  the resolution must reduce the coverage error of the reported volumes, monotonically, on a fixed
  fixture.
- **A7 — The report is the contract.** `cam-cli simulate` prints a deterministic report; the same
  document produces byte-identical output on every run, and the exit code distinguishes a clean program
  from one whose simulation found defects.
  The profile half of A8 runs on a circular contour: at a convex corner the mitered band and the union
  of discs disagree by design, and D-M5-11 records the bound rather than hiding it behind a fixture.
- **A8 — The oracle.** For each strategy M4 shipped, one test asserts the simulated removed volume
  against the analytic expectation: a pocket whose toolpath cuts everything within its region, a raster
  pocket, drilled holes, and a profile kerf. The error bound is the simulation's own documented coverage
  bound (perimeter × cell / 2), not a number chosen to fit. A pocket cut too shallow must report uncut
  volume, and one cut too deep must report gouge volume — the tests that S6 asks for.
- **A9 — A 3D view of the stock, from the same numbers.** A camera with yaw, pitch, distance and target
  projects the stock box and the heightfield surface into screen space; the surface is shaded by depth;
  the toolpaths are drawn as 3D lines. The projection and the mesh are pure data (`cam-view`), testable
  headless (S2: "Testeable headless, determinista").
- **A10 — An animated preview.** The window plays the program: play, pause and step, with material
  removed progressively as the cursor advances through the moves. Time enters the logic as an injected
  delta, never read from a clock inside the state, so a test drives frames and asserts the same result.
- **A11 — No new dependency.** The 3D surface is painted with `egui::Mesh` through the existing
  painter; `Cargo.lock` gains no crate (S2).
- **A12 — The 2D editor is untouched.** The authoring, picking, snapping, import and undo behaviour
  pinned since M1–M3 keeps passing unchanged; the 3D view is an addition to the shell, not a rewrite of
  it (S3).
- **A13 — The tool is a flat cylinder.** The tool the simulation removes with is a disc of the tool's
  diameter and nothing else, because `ToolSpec` (`crates/cam-toolpath/src/lib.rs:34-46`) carries
  `number`, `diameter`, `feed_rate`, `plunge_rate`, `spindle_speed` and no shape. V-bit and tip-geometry
  simulation is a recorded limit, not an omission (A5's target model stays depth-based, so it does not
  need one).

### Decisions recorded for M5

- **D-M5-1 — The simulation consumes the `MoveStream`, not the strategies' intentions.** The input is
  the same structure `cam-cli emit` hands the post-processor. Rationale: S6 calls the simulation an
  oracle, and an oracle that re-reads the intent it is meant to check is not one. Consequence recorded:
  a defect the strategies share with the emitter is simulated faithfully — that is the point.
- **D-M5-2 — The target surface is derived from the document, per cell, not per toolpath run.** A5's
  one-rule form ("removed deeper than authorised" / "authorised deeper than removed") makes gouge and
  uncut the same subtraction with two signs, so a test that proves one proves the arithmetic of the
  other. Per-toolpath attribution is reported alongside, by simulating each toolpath's own span of
  moves over the shared field.
- **D-M5-3 — Cell-centre coverage, with a published bound.** A cell is cut when its centre is inside the
  swept disc. That is the cheap, deterministic rule; it makes the volume error of a boundary at most
  half a cell wide, which is why A8's bound is `perimeter × cell / 2` and why A6 demands monotone
  refinement. Recorded alternatives rejected: exact analytic area coverage (a kernel of its own, and
  every boundary would need arc integral arithmetic) and supersampling (a hidden tolerance knob).
- **D-M5-4 — A rapid below the surface is refused, not simulated.** S11's linking retracts to `safe_z`
  precisely so no rapid crosses material, so a program that does has a defect that must surface. The
  simulation reports it and the command fails (A4); it does not "cut" the rapid and continue, because
  that would hide a crash inside a plausible volume number.
- **D-M5-5 — A profile's target is the material its tool sweeps, not the distance its centre travels.**
  The emitter offsets the tool *centre* by `radius + allowance`, so the material a clean profile
  removes spans from `allowance` to `2·radius + allowance` outside the vector (`Inside`: the same band
  inwards; `On`: `±radius`, because `lateral_offset` returns zero for `On` whatever the allowance says).
  Authorising only `0…radius + allowance` would report every clean profile as a gouge of the outer
  half of its own kerf, and S3 found exactly that: the narrow band was the spec's reading, not the
  machine's (L6). Material on the keep side has target depth zero, which is what turns "the profile
  gouged the part" into one number.
- **D-M5-10 — An allowance is material the target does not authorise.** The emitter insets a pocket's
  outermost ring by `radius + allowance`, so a pocket with an allowance leaves a wall of width
  `allowance` standing on purpose. The target for a pocket is therefore its region inset by the
  allowance (islands grown by it, which is the same parity operation), and with the default allowance
  of zero it is the region itself. Without this, a correctly cut allowance pocket reports `allowance ×
  perimeter × depth` of uncut material that the document asked to keep.
- **D-M5-11 — A convex corner over-authorises by the corner-rounding area, and that is recorded.** The
  band comes from `offset_contour`, whose joins are mitered (`miter_limit: 4.0`), while the removal is a
  union of discs of the tool radius along that path. Along a straight or smooth vector the two agree;
  at a convex corner the mitered band claims material the disc never touches, at most `4(r² − πr²/4)`
  per right-angled corner, so a perfect cut reports `≈7.73 mm² × depth` of uncut per corner at a 6 mm
  tool (measured: 24 mm³ over 4 cells on a 100 mm square profile at a 1 mm cell, converging to the
  analytic limit as the grid refines). The same arithmetic explains a pocket's four corners, and P1's
  end-to-end run measured exactly it: a mitered ring's corner stands `r√2` from the region's corner while
  the disc reaches only `r`, so a clean offset pocket also reports `24 mm³` over the same 4 cells at a
  1 mm cell. Every region with a right angle therefore reports it, in both strategies, and it is the
  miter that would have to become a round join to remove it. The alternative — a round-join offset — is a
  kernel feature this milestone does not add, and a tolerance tuned to corners would be exactly the
  resolution knob D-M5-3 refuses. Recorded as a limit, with the circular-contour oracle as the case where
  the statement is exact.
- **D-M5-12 — A raster lattice that stops short of the far edge is left as recorded, not fixed.** S6
  rescued the row that lands *on* a boundary; it did not make the lattice *reach* the boundary. On a
  100 mm square pocketed with a 6 mm tool at a 4 mm stepover the rows run 13, 17, … 85 while the offset
  region's far edge is 87, so the last row's disc reaches 88 and the 2 mm strip `88…90` is never cut:
  `simulate` reports `uncut: 984.000 mm3 over 164 cell(s)` on a program no user would call wrong. The fix
  is a boundary pass — one row at the offset region's far edge — and it is deliberately **not** taken
  here: M4's raster contract says "rows on the `min.y + k·stepover` grid" and pins "32 rows at y = 3, 6,
  … 96" (`odd/tasks/m4-toolpaths.md`), so adding a finishing row changes a documented contract and
  belongs to a decision the owner takes, not to a milestone that was asked for simulation and a preview.
  Recorded with its measurement so the report's `uncut` figure on a plain raster pocket is understood as
  the strategy's outline showing through, not as a broken simulation.
- **D-M5-6 — A simulation that finds defects exits `6`.** The CLI's existing codes are `0` ok, `2`
  invalid input, `3` unsupported schema, `4` post error, `5` vector issues; `5` sets the precedent that
  a *found defect* is its own code rather than "invalid input". Crash, gouge and uncut therefore exit
  `6` and the report names which. Bad arguments and unreadable documents stay `2`.
- **D-M5-7 — The 3D surface is painted with `egui::Mesh`, and `wgpu` keeps a trigger.** This is S2's
  decision, and it inherits S12's revisit trigger rather than contradicting it: `cam-view` emits the
  projected mesh (pure data), the shell paints it, and a wgpu backend would consume the same mesh when
  the trigger fires — measured, not assumed. The mesh carries a decimation stride so the triangle count
  is bounded independently of the cell count (S7's "resolution decoupled from job size", without
  `rayon`: the removal loop is single-threaded until a measurement says otherwise, D-M5-8).
- **D-M5-8 — No `rayon`, no tiling, in M5.** S7 mitigates *large* jobs; M5's job sizes are bounded by
  A6's cell cap, and a parallel removal loop would need its own determinism argument for the volume
  sums. Recorded as a decision with its trigger: parallel or tiled removal lands when a measured
  simulation of a default sheet at the default resolution exceeds one second.
- **D-M5-9 — The preview never owns the document.** The window simulates a program handed to it (A10,
  S3: "La ventana muestra el preview del programa ya autorado por `cam-cli`"); authoring toolpaths in
  the shell stays out of M5, per S14, so M5 adds no `cam-app` command and no document mutation path.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **S1** — A1, A3, A6 — inline — `feat(sim): a heightfield of the stock and its volumes`
- **S2** — A2, A4 — inline — `feat(sim): swept removal over a program, and the rapid that rams it`
- **S3** — A5, A13 — inline — `feat(sim): the authorised target surface, gouges and uncut material`
- **S4** — A7 — inline — `feat(cli): simulate, the report and its exit code`
- **S5** — A8 — inline — `test(sim): the oracle for every strategy M4 shipped`
- **S6** — A8 — inline — `fix(toolpath): a raster row on the region's far edge is material`
- **V1** — A9 — inline — `feat(view): a 3D camera and the stock surface as a mesh`
- **V2** — A9, A11, A12 — inline — `feat(ui): the 3D panel, orbit, zoom and a depth-shaded surface`
- **V3** — A10 — inline — `feat(ui): the program plays, and the stock disappears as it cuts`
- **D1** — docs — inline — `docs(m5): the simulation model, the CLI contract and the roadmap evidence`
- **P1** — A12, and the differential the shared CLI parser owes — inline —
  `test(cli): the pre-existing commands are unchanged, and M5 is closed`

Route notes: the two writer pairs this plan named (S2+S3 over the new crate, V1+V2 over `cam-view`
and `cam-ui`) could not be launched: the writer gate accepts a task of a feature document only in the
form `Spec: odd/tasks/<feature>.md, T<n>`, and this feature numbers its tasks `S`, `V`, `D` and `P`
(L4). The two launches were refused by the harness twice and, per its own instruction, were not
retried; every task is therefore inline, in the order below. The dependency reasoning that produced
the pairs still holds and is what orders the work: S1 fixes the field, S2 and S3 are independent
files over it, S4 and S5 consume both, V1 and V2 are independent crates, V3 consumes V1's projection.

### Task detail

**S1 — The field.** New crate `cam-sim` (`crates/cam-sim`, added to the workspace members), depending
on `cam-units`, `cam-move` and `cam-model` — never on `cam-toolpath`, `cam-post` or any UI crate, so the
simulation cannot be accused of consulting the emitter.

```rust
pub struct Grid { pub origin: Point2, pub cell: Length, pub columns: usize, pub rows: usize }
pub const DEFAULT_CELL: f64 = 1.0;            // millimetres
pub const MAX_CELLS: usize = 4_000_000;
pub enum GridError { TooFine { cell: f64, columns: usize, rows: usize, cap: usize },
                     Degenerate { reason: String } }
pub fn grid_for(job: &Job, cell: Length) -> Result<Grid, GridError>;

pub struct Heightfield { /* grid + Vec<f64> of surface z, row-major */ }
impl Heightfield {
  pub fn new(job: &Job, cell: Length) -> Result<Self, GridError>;  // surface = 0 everywhere
  pub fn grid(&self) -> Grid;
  pub fn surface(&self, column: usize, row: usize) -> f64;
  pub fn lower(&mut self, column: usize, row: usize, z: f64);      // min(current, z), clamped
  pub fn volumes(&self) -> Volumes;
  pub fn cell_centre(&self, column: usize, row: usize) -> Point2;
}
pub struct Volumes { pub removed: f64, pub remaining: f64, pub through_cells: usize }
```

Semantics to pin with tests: `grid_for` covers the job extent with `columns = ceil(width / cell)`,
`rows = ceil(height / cell)`, and a cell's centre is `origin + (i + 0.5) * cell`; the grid is whole
cells, so its area can exceed the job's by less than one cell in each direction, and every volume is
measured on the grid rather than on the nominal rectangle (the published coverage bound of D-M5-3).
`Heightfield::new` starts at the stock top (`0`), `lower` never raises a cell and clamps at
`-thickness`, and `volumes()` sums in row-major order (`removed = Σ -z * cell²` over cut cells,
`remaining = grid volume - removed`, `through_cells` counts cells at exactly `-thickness`).
`MAX_CELLS` is enforced before allocation, with the error carrying the cell, the grid it implies and
the cap (A6). Tests: a default sheet at the default cell size stays under the cap; a 0.1 mm request for the same
sheet is `TooFine` and names `cap`; a 10 mm square at 1 mm is 10 × 10 cells with centres at 0.5 … 9.5;
lowering a cell twice keeps the deeper value; `volumes` of an untouched field is zero removed /
`width*height*thickness` remaining; lowering a whole field to `-thickness` reports every cell through
and remaining zero; a property test asserts `removed + remaining` equals the stock volume for random
sequences of `lower` calls.

**S2 — Sweep.** In `cam-sim`, over a `MoveStream`:

```rust
pub struct Crash { pub toolpath: String, pub move_index: usize, pub at: Point3 }
pub struct Removal { pub moves: usize, pub crashes: Vec<Crash> }
pub fn apply(program: &MoveStream, field: &mut Heightfield) -> Removal;
pub fn apply_move(field: &mut Heightfield, movement: Move, tool_radius: f64) -> Option<Crash>;
```

Rules: `Feed` and `Arc` remove to their destination `z` along the swept footprint; a `Rapid` or
`Retract` at or above the current surface removes nothing and is never a crash; a `Rapid` that ends
below the current surface at its destination cell is a crash and does not cut. The tool radius is the
per-segment tool's `diameter / 2`, taken from the `Segment` — which needs the diameter on the segment:
`cam_move::Segment` carries `tool_number` but not `diameter`, so S2 adds `tool_diameter: Length` to
`Segment` (one field, every constructor updated, no behaviour change for the emitter, which ignores it)
and records that choice here rather than plumbing the library into the simulation. A `Feed` whose `z` is
above the surface cuts air. Sweep sampling: a line is walked in steps of `cell / 2` along its length and
a disc of the tool radius is applied at each sample (the union of discs whose centres are that dense
covers the swept rectangle within one cell, which is inside D-M5-3's bound); an `Arc` is walked in
angular steps of `cell / (2 * radius)` radians. Determinism: the sample count is a function of the
move's own numbers, so two runs give the same field. Tests: a straight feed 10 mm long with a 6 mm tool
on a 1 mm grid cuts a 10 × 6 mm rectangle of cells (counted, not eyeballed); the same cut as one arc of
a full circle leaves the centre of the circle uncut, proving the arc is swept along its circle, not its
chord; a rapid at `safe_z` over untouched stock removes nothing and reports no crash; a rapid to `-3`
reports one crash at that move index and leaves the field untouched; a feed above the surface cuts
nothing; two runs are field-identical, asserted through `volumes` and a checksum of the surface values.

**S3 — Target.** In `cam-sim`, `target.rs`:

```rust
pub struct Target { /* grid + Vec<f64> of authorised depth, row-major, >= 0 */ }
pub fn target_for(document: &Document) -> Result<Target, GridError>;
pub struct Deviation { pub gouge: f64, pub uncut: f64, pub gouge_cells: usize, pub uncut_cells: usize }
pub fn deviations(field: &Heightfield, target: &Target, tolerance: Length) -> Deviation;
```

Rules (A5, D-M5-2, D-M5-5): authorised depth is `max` over every toolpath that claims the cell — pocket
region (boundary minus islands) → final depth `start_depth + cut_depth`; drilling → the hole circle at
its final depth; profile → the kerf band at its final depth, less `tab.height` inside a tab's span and
zero on the keep side. Everywhere else, zero. `deviations` compares `removed_depth = -surface` with
`authorised`: gouge where `removed > authorised + tolerance`, uncut where `authorised > removed +
tolerance`. Per-toolpath attribution is a second call over that toolpath's own moves (D-M5-2), not a
second rule. Tests: a 100 mm square pocket with a centred r10 island at 6 mm deep authorises the square
minus the disc, with the island's cells at zero; a profile Outside on the same square authorises a 3 mm
band outside it and nothing inside; a profile with a tab of 4 mm height authorises `2` mm in the tab's
span at a 6 mm cut; a drilling toolpath authorises each hole's disc; two overlapping pockets authorise
the deeper of the two; a cell claimed by nothing is zero. Then the arithmetic: a field cut exactly to
the target deviates zero; cut 1 mm too deep reports the gouge volume of that region; cut 1 mm too
shallow reports the same volume as uncut; the tolerance moves the same difference from "gouge" to
"within tolerance" (one boundary test).

**S4 — The command.** `cam-cli simulate <document> [--library <sqlite>] [--resolution <mm>]`:

```
simulate: 1219 x 2438 x 18 mm, 1 x 1 mm cells (1219 x 2438), 3 toolpath(s)
program: 214 move(s), 412.500 mm cutting length
removed: 123456.789 mm3 of 53433816.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 812.400 mm3 over 204 cell(s)
crash: none
```

The tool library resolves exactly as `emit` does (`--library`, else `document.tools_ref` relative to the
document), the segments are built with `build_segment` unchanged, and the report is printed to stdout.
A crash line reads `crash: <toolpath> move <index> at (x, y, z)`, unsorted input order preserved. Exit
`0` when the report is clean, `6` when gouge, uncut or crash is non-zero (D-M5-6), `2` for a bad
argument, an unreadable document or `GridError::TooFine` rendered whole. Numbers use `cam-units`'
fixed-decimal formatting so the output is locale-free and byte-stable (S9). The `--help` text and
`docs/format.md`'s command table gain the row in the same commit (the repository's own rule). Tests
(binary, in `cam-cli/tests/cli.rs`'s style): a pocket document reports the analytic removed volume within
A8's bound and exits `0`; the same document at a finer resolution reduces the error (A6); a too-shallow
pocket reports uncut and exits `6`; a document with a rapid at depth exits `6` and names the toolpath and
move index; two runs print identical bytes; a `--resolution` that exceeds the cap exits `2` with the
three numbers in the message; the file on disk is not modified.

**S5 — The oracle.** In `cam-sim/tests` and asserted again through the binary: for a 100 mm square
pocket 6 mm deep with a 6 mm tool at 4 mm stepover, the removed volume is `area × depth` within
`perimeter × cell / 2`; the raster mode agrees with the offset mode to that same bound; drilled holes of
r3 in a 100 mm square remove `π r² depth` per hole within the bound; a profile Outside on a 100 mm
square removes the kerf band's area × depth within the bound; and the monotone-refinement test from A6
runs the same fixture at 2, 1 and 0.5 mm cells and asserts the error never grows. A too-deep variant of
each exits `6` with gouge, a too-shallow one exits `6` with uncut — the assertion S6 names explicitly,
stated in both directions so the test cannot pass by reporting the wrong sign.

**V1 — The camera and the mesh.** In `cam-view`:

```rust
pub struct Camera3d { pub yaw: f64, pub pitch: f64, pub distance: Length, pub target: Point3, pub viewport: Viewport }
impl Camera3d { pub fn project(&self, point: Point3) -> Option<Screen3>;  // Screen3 { x, y, depth }
                pub fn orbit(&mut self, delta_yaw: f64, delta_pitch: f64);
                pub fn zoom(&mut self, factor: f64); }
pub struct SurfaceMesh { pub vertices: Vec<SurfaceVertex>, pub triangles: Vec<[usize; 3]> }
pub struct SurfaceVertex { pub position: Point3, pub depth: f32 }   // depth for the colour ramp
pub fn build_surface(field: &Heightfield, camera: &Camera3d, stride: usize) -> SurfaceMesh;
pub fn build_stock_box(job: &Job) -> Vec<Line3>;
```

`pitch` is clamped just short of vertical (the projection has no up vector to invent), `project` returns
`None` behind the camera, and the mesh decimates by `stride` so its triangle count is bounded (D-M5-7).
The viewport is `cam_app::Viewport`, which is the type this crate already uses for the 2D camera; the
mesh takes no bounds argument because the grid carries its own extent, and the stock box returns lines
because four plan-view corners cannot express a thickness (L6).
Tests: a point at the camera's target projects to the viewport's centre; yaw rotates a point about the
target by that angle; a point behind the camera projects to `None`; pitch clamping is asserted at both
ends; a flat field at `stride = 1` gives `(columns-1) × (rows-1) × 2` triangles and at `stride = 4` the
count shrinks by the documented factor; the same field and camera give vertex-for-vertex identical
meshes; the depth ramp maps the stock bottom to one end of the palette and the top to the other, exactly.

**V2 — The panel.** In `cam-ui`: a `panel_3d` bound to a key or a button in the command bar, painting
`SurfaceMesh` through `egui::Mesh` (A11) with a depth ramp, the stock box edges, and the toolpaths as
projected polylines; orbit by drag, pitch clamped, wheel zoom about the target. The panel is additive:
`Shell::show` keeps its 2D path when the panel is closed (A12), and `DrawnState` gains the counts a test
can read (`painted_triangles`, `painted_3d_lines`). Tests, headless in `cam-ui/tests`: opening the panel
paints triangles (count > 0) and a closed panel paints none; a drag changes yaw and a wheel changes
distance, asserted through the shell's own state; a mesh that the camera rejects paints nothing instead
of panicking; the 2D canvas still paints its items while the 3D panel is open (A12's regression, asserted
as a count).

**V3 — Playback.** In `cam-ui` (state) with its arithmetic in `cam-view` or `cam-sim` where it can be
tested without egui:

```rust
pub struct Preview { pub moves: Vec<Move>, pub tool_radius: f64, pub cursor: usize,
                     pub fraction: f64, pub speed: f64, pub playing: bool }
impl Preview { pub fn advance(&mut self, dt: f64); pub fn step(&mut self); pub fn reset(&mut self); }
```

`advance(dt)` consumes `dt * speed` seconds of cutting time at the segment's feed rate, applies whole
moves to the field as it crosses them, and stops at the end; `dt` comes from the caller (`Shell` passes
`ctx.input(|i| i.stable_dt)` — the only place a clock is read, A10). Tests: `advance` with a fixed `dt`
sequence reaches the same `cursor` and the same field volumes as the closed form; a zero `dt` changes
nothing; pause leaves the cursor still and play resumes from it; `step` advances exactly one move;
`reset` restores the stock; the preview's field at the end equals the whole-program simulation from S2
(the animation and the report cannot disagree).

**D1 — Docs.** `docs/m5-simulation.md`: the heightfield model, the target surface and its two
deviations, the crash rule, the resolution/cap policy, the report's exact shape, the exit codes, and the
recorded limits (flat-cylinder tool, cell-centre coverage, no linking optimisation yet, no `rayon`).
`README.md`'s status line and "Next" move to M6; the crate table gains `cam-sim` and the dependency
sentence gains its edge. `docs/format.md`'s command table gains `simulate` with R (the offset documented
as addable). `odd/roadmap.md` gains §4.8 with M5's evidence table in the shape §4.6 and §4.7 use, marks
M5 delivered in §4, and **corrects the stale paragraph in §4.7 that still calls P5 open** (P5 landed in
`b7910f3`; the text contradicts the table directly above it).

**P1 — Close.** The differential the shared CLI parser owes: with the branch's `simulate` in place, run
`validate`, `inspect`, `import`, `check`, `postp validate`, `tools list` and `emit` against a copy of
`master` at `0d96b36` in a `mktemp -d` scratch directory and against the branch, comparing exit code,
stdout and stderr — any difference nobody asked for is a regression to fix and pin with a test. Then the
evidence table below, the task commits, and the final `./scripts/check.sh`.

## Delivery slices

| Slice | Tasks | What the slice demonstrates |
| --- | --- | --- |
| 1 | S1 | The field exists, is bounded and reports volumes |
| 2 | S2, S3 | The field follows a real program, refuses a ram, and knows what the document authorised |
| 3 | S4, S5 | The CLI is the contract, and every M4 strategy passes its oracle |
| 4 | V1, V2 | The stock is visible in 3D, headless-proven |
| 5 | V3 | The program plays and the stock disappears as it cuts |
| 6 | D1, P1 | Docs, roadmap evidence, differential, close |

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| S1 | `36bdf40` | 11 tests (`crates/cam-sim/tests/heightfield.rs`). The default sheet at the default cell is 1219 x 2438 = 2,971,922 cells, under the 4,000,000 cap; a 0.1 mm request names 12190 x 24380, the product and the cap in one message; a zero-sized job and a zero cell are `Degenerate`, not a panic; a 10 mm square at 1 mm has 100 cells whose centres run 0.5 … 9.5; lowering twice keeps the deeper value; a cell clamped at `-thickness` counts as through; a whole field to the bottom reports 500 mm³ removed, 0 remaining and 100 through cells; 25 cells at 2 mm deep report 50 mm³; a 10.2 mm wide job gets 11 columns whose last centre is outside the nominal width, which is the coverage bound stated rather than hidden; and a property test asserts `removed + remaining` equals the grid volume for random cut sequences |
| S2 | `93179d3` | 6 tests (`crates/cam-sim/tests/sweep.rs`), one of them lifted the whole capsule question: a straight feed's cut cells are compared against a brute-forced exact point-to-segment distance over every cell, so the sampled-disc sweep has to agree with the real capsule cell for cell, and the removed volume is that count times the depth. A full-circle arc leaves its centre cell standing (the centre is 0.5 mm from the cell at (20.5, 20.5), and the tool is 1 mm, so the assertion also proves the disc is applied where the arc is); a rapid at clearance cuts nothing and is not a crash; a rapid to -3 is one crash carrying `pocket 1` and move index 1, and the field stays untouched; a feed at +2 cuts air; two runs give identical volumes and identical surfaces cell by cell. `cam_move::Segment` gained `tool_diameter` (the emitter ignores it) and `Heightfield` gained `surface_under` |
| S3 | `cdc5ad1` | 15 tests (`crates/cam-sim/tests/target.rs`). A pocket region with a centred r10 island authorises the square minus the disc (island cells 0, volume `(100² − π10²) × 6` inside the published bound); an `Outside` profile authorises its band and leaves the part standing; an `Inside` profile the mirror; an `On` profile ignores the allowance the emitter ignores; a tab keeps its height back; a tab on a circular profile follows the arc length rather than a sampled point cloud; a drilling toolpath authorises each hole and nothing else; two pockets authorise the deeper, never the sum; a cell nobody claims is 0; a field cut exactly to the target deviates by zero; one difference read with two signs is a gouge and then an uncut of the same volume; the tolerance decides the side; a field and a target on different grids are refused naming both; `start_depth 2 + cut_depth 6` authorises 6, not 8; a profile without its tool diameter is refused by name |
| S3b | `b283e6c` | 20 tests after the amendment, 5 of them new. The band is `allowance … 2·radius + allowance`: `Outside` 112²−100² = 2544 cells (was 636), `Inside` 2256 (was 636), the tab block 96 (was 39), and a 1 mm allowance authorises nothing in the first millimetre while the full depth sits past it (15552 and 13248 mm³). A pocket's target is its region inset by the allowance through one whole-region `Region::offset`, so its wall is not authorised (1944 mm³ at 18² cells) and its island grows with it. The clean-cut test is the amendment's own evidence, and it was run against the old band first: a perfect 100 mm `Outside` cut reported `Deviation { gouge: 1632.0, uncut: 0.0, gouge_cells: 272, uncut_cells: 0 }` — the outer half of its own kerf — and reports zero on a circular contour now |
| S4 | `93e4788` | 9 binary tests (`crates/cam-cli/tests/cli.rs`), `cam-cli` at 53. A pocket that cuts what it claimed prints its six lines byte for byte, exits `0` and leaves the document untouched; a finer resolution reduces the coverage error (505.0 to 126.5 mm3 on a 120.4 x 90.4 pocket, A6); a pocket deeper than the stock pins the whole uncut report and exits `6`; a drill wider than its hole reports gouge and exits `6`; a rapid at depth names the toolpath and move index on the first crash line and lists the rest in program order; two runs print identical bytes; `--resolution 0.01` exits `2` naming the cell, the grid and the cap; a missing library exits `2`; `--help` lists the command, its flags and its exit code. The tolerance is `cell/8`, and `docs/format.md` says why: the sweep samples at half a cell, so its own error is `cell^2/(32r)` while the defects being caught are a millimetre deep. Two numbers were corrected against independent fact rather than fitted: `program:` counts `MoveStream` moves, two fewer than the G-code's move lines because the post's footer is not a stream move, and the refinement fixture needed a tool that sweeps the region at both resolutions |
| S5 | `240212c` | 6 kernel tests (`crates/cam-sim/tests/oracle.rs`, `cam-sim` at 43) plus 2 added and 1 strengthened through the binary (`cam-cli` at 55), workspace at 794. Offset pocket 59976 of 60000 mm3 analytic (the four corner cells a 6 mm disc cannot enter); raster agrees with offset to 600 mm3; three r3 holes 576 mm3 against 508.938; a circular `Outside` profile is exact -- `Deviation::default()` -- because a circle has no convex corner for D-M5-11 to over-authorise; refinement at 2/1/0.5 mm never grows the error (115.062, 67.062, 4.938 mm3); and both signs on one region, 1 mm shallow is uncut 96 mm3 and 1 mm deep is gouge 96 mm3, so the test cannot pass by reporting the wrong sign. Through the binary a raster pocket and a drilling document pin their whole reports and exit `0`, and the gouge direction states its cubic millimetres: a 2 mm hole drilled with a 6 mm tool reports `gouge: 120.000 mm3 over 20 cell(s)`. Two fixture numbers were corrected against arithmetic, and the first correction exposed a real defect in the raster strategy (S6) |
| S6 | `bd37bad` | 1 new kernel test (`crates/cam-toolpath/tests/pocket.rs`, `cam-toolpath` at 74) and 2 re-derived expectations (`crates/cam-cli/tests/cli.rs`, still 55), workspace at 795; no other file touched. The reproduction, run before the fix: a 100 mm square, 2 mm tool, 2 mm stepover, no allowance emits **49** rows instead of 50 — the row at `y = 99`, the inset region's far edge, is dropped — and covers **9800** of the 10000 cell centres of a 1 mm lattice, i.e. 58800 mm3 removed against the analytic 60000 and 200 cells / 1200 mm3 left standing, which is exactly the `removed: 58800.000 mm3`, `uncut: 1200.000 mm3 over 200 cell(s)`, exit `6` S5 measured. After the fix: 50 rows at `y = 1, 3, … 99`, the last within `ROW_TOLERANCE` of 99, and 10000 covered cells = 100 x 100 mm2 x 6 mm = 60000 mm3. Through the binary at a 2 mm stepover: `program: 200 move(s), 4900.000 mm cutting length` (50 rows x 4 moves, 50 x 98 mm), `removed: 60000.000 mm3`, `gouge: 0.000 mm3 over 0 cell(s)`, `uncut: 0.000 mm3 over 0 cell(s)`, `crash: none`, exit `0` — and S4's tuned 1.58 mm stepover is gone from that fixture, because 1.58 was chosen to make the row lattice miss the far edge (it read 252 moves / 6174 mm and reported the same 60000 mm3 by luck of overlap, not by cutting the edge). The island raster golden went from 45 to 49 lines: the `y = 57` row is cut (N390–N420), 9 spans to 10, 6 rows to 7 — and the old program's last row at `y = 48` reached only `y = 51` with a 3 mm tool, leaving the 9 mm strip from 51 to the square's 60 mm edge uncut, so that golden was pinning a program with a hole in it. Every other emit golden (offset pocket, island offset, circular island, boss, profile, drilling, schema-one) is byte-identical, so only raster programs moved. One property of the fixture is stated rather than implied: a 2 mm stepover under a 2 mm tool leaves adjacent rows exactly tangent, so the same program at `--resolution 0.5` reports `uncut: 300.000 mm3 over 200 cell(s)` — the centres between two tangent rows are past the radius. The fixture pins the row lattice's far edge at the 1 mm cell the oracle's bound is written for, not a resolution-independent claim |
| V1 | `6603f93` | 13 tests (`crates/cam-view/tests/camera3d.rs`, `tests/mesh.rs`); the crate stands at 36. The target projects to the viewport's centre at four pitches and three yaws; yawing the camera by δ equals rotating the point about the vertical by δ, and a quarter turn carries a point across the centre in the direction the sign says; points behind the eye or not finite are `None` while points in front are not; the pitch clamp holds at both ends and a field poked past it still projects; zoom halves, clamps at `MIN_DISTANCE`/`MAX_DISTANCE` and ignores a zero, a negative, a NaN and an infinity; a 10 x 8 field gives 80 triangles / 126 vertices at stride 1, 12 / 12 at stride 4 and 4 / 2 at stride 100, all indices in range, with a zero stride treated as one; the depth ramp is exactly 0.0 at the untouched top and exactly 1.0 at the stock bottom, 2/18 for a shallow cut; the stock box has 12 edges, 8 corners of degree 3, every edge axis-aligned, spanning `0..width`, `0..height`, `-thickness..0`; two builds are identical vertex for vertex, even across a different camera, and the triangles are ordered back to front by a stable sort. 2D suites untouched (13 + 10) and `cargo test -p cam-ui` still green (14 + 38), which is A12's crate-boundary half |
| V2 | `1c3d3bf` | 9 tests (`crates/cam-ui/tests/view3d.rs`), `cam-ui` at 61. An open panel paints triangles and exactly 16 3D lines (12 box edges + the program's 4); a drag orbits yaw and pitch with the target fixed and the wheel zooms about it; a camera the projection rejects paints nothing instead of panicking; a closed panel paints no 3D at all; the surface reaches the painter as one `egui::Mesh` of exactly three vertices per painted triangle, so A11's "no new crate" is pinned at the painter; the 2D canvas still paints its 4 items and its grid while the panel is open; and a closed panel leaves `canvas`, `painted_items` and `painted_grid_lines` equal to a second shell that holds no preview at all — the 52 pre-existing shell tests are untouched. Deviations: a top panel rather than a central split, `show_3d` as shell state rather than a `UiAction` (a view is not a document command, D-M5-9), and arcs flattened at half a pixel before projecting so the picture shows the cut the machine made |
| V3 | `2d479d4` | 10 tests (`crates/cam-ui/tests/view3d.rs`), `cam-ui` at 71, `cam-desktop` at 2. Playback is a pure function of the injected delta: six deltas at speed 1 walk `cursor` 0→1→2→2→2→4 with fractions 0.5/0.25/0.2/0.6/0.9/1.0 and `elapsed` equal to their sum; the field's volumes equal the closed form of the same prefix simulated independently; a partly consumed move cuts nothing; a zero delta and a paused frame change nothing; a step advances exactly one move and applies it; `reset` restores the untouched stock cell for cell; the final field equals `sweep::apply` over the whole stream twice (one 10 s call and 40 × 0.03 s frames) on a 2 mm grid; playback stops at the end and never loops; and `DrawnState::preview_progress` is 0.0 paused, strictly between 0 and 1 after one playing frame (which proves the shell feeds the real delta), and exactly 1.0 at the end. The window's own sample is pinned the same way: untouched before playing, cut after, cell-for-cell equal to `sweep::apply`. Contract change recorded: `set_preview` now takes the untouched stock, because `reset` needs a pristine field and nothing can un-cut one |
| D1 | _pending_ |  |
| P1 | the closing commit of this feature | The differential over every pre-existing subcommand, run against a copy of `master` at `0d96b36` in a scratch sandbox and against the branch in an identical one: `import`, `tools add`, `tools list`, `toolpath` for profile, offset and raster, `validate`, `inspect`, `check`, `postp validate`, and `emit` twice are byte-identical in exit code, stdout, stderr and the files they produce (the authored `.c2d`, the tool library and the emitted G-code). Exactly two differences, both additive and both asked for: `--help` gained the `simulate` block and exit code 6, and `simulate` itself exists. Running it on the mixed fixture was the milestone's first end-to-end report, and it attributed its `uncut: 48.000 mm3 over 8 cell(s)` to the two limits already recorded: the profile's four mitered corners (24 mm³ over 4 cells) and the pocket's four mitered corners (the same 24 mm³, measured by running each toolpath alone), with D-M5-12's raster strip found the same way (`uncut: 984.000 mm3 over 164 cell(s)` on a 4 mm stepover). The closing gate is `./scripts/check.sh` green at **795 tests** — `fmt --check`, `clippy -D warnings` and the whole suite |


## Log

- **L1 (user, verbatim):** "m5".
- **L2 (user decisions, verbatim option labels and descriptions, this session):** "egui mesh, cero
  dependencias nuevas" — "Proyección y sombreado propios sobre egui::Mesh. Testeable headless,
  determinista. wgpu queda como trigger de performance registrado."; "Sólo simulación + preview" — "M5 =
  cam-sim + reporte + vista 3D + animación + `cam-cli simulate`. La ventana muestra el preview del
  programa ya autorado por `cam-cli`."
- **L3 (plan):** this document. Base: `master` at `0d96b36`, branch `feat/m5-simulation`, suite at
  707 tests. Integration surface confirmed read-only before writing it: `ToolSpec` carries only
  `diameter` (S13/A13); `cam_move::Segment` carries `tool_number` but not the diameter, which is why S2
  adds `tool_diameter` to it; strategies emit cutting moves at negative Z (S13); nothing in the
  workspace imports `wgpu`, `glam`, `bytemuck` or `rayon`; `cam-cli`'s tests are one binary test file
  with inline golden program constants and `scratch(name)` directories; `cam-ui`'s headless harness is
  `Harness::new()` + `frame(events)`.
- **L4 (routing).** The plan named two writer pairs (S2+S3, V1+V2). The writer gate accepts a
  delegation only for a task of a feature document referenced as `Spec: odd/tasks/<feature>.md, T<n>`,
  and this document numbers its tasks `S1…S5`, `V1…V3`, `D1`, `P1`, so both launches were refused and,
  per the harness's own instruction, were not retried. Every task is inline. The route column and the
  route notes were corrected rather than left claiming a route that did not happen.
- **L5 (execution, S2).** The sweep landed with six tests, and one of them caught the spec's own
  arithmetic: the annulus assertion first named the cell at (22, 20), whose centre is 2.55 mm from the
  circle's centre — nowhere near the 4…6 mm annulus a radius-5 arc cut by a 1 mm tool sweeps. The cell
  at (24, 20) is 4.53 mm out and is the one the arc cuts; the test now also asserts that the cell at
  (21, 20), 1.58 mm out, is left standing, so the inner edge of the annulus is pinned rather than
  implied. Two changes outside the new crate were needed and are recorded: `cam_move::Segment` gained
  `tool_diameter`, because the stream carried the tool's number but not its size and the simulation
  needs the footprint a move sweeps (the emitter ignores the field, and its seven tests are unchanged);
  and `Heightfield` gained `surface_under`, the cell containing a point, which is what the crash rule
  asks.
- **L6 (execution, S3, and a decision reversed by evidence).** S3's worker found that D-M5-5 as first
  written could not be satisfied by a clean program: the emitter offsets the tool *centre* by
  `radius + allowance`, so the kerf it cuts is `allowance … 2·radius + allowance` wide, and a target of
  `0 … radius + allowance` reports the outer half of a perfect cut as gouge. The decision and A5 were
  amended rather than the test. Four further defects came out of the same task, each fixed at its root:
  `authorised_depth` read `start_depth + cut_depth` when the emitter's `pass_depths` walks *from*
  `start_depth` to `cut_depth` and lands on it (`crates/cam-toolpath/src/lib.rs:264`, `docs/m4-toolpaths.md:60`),
  so any toolpath with a non-zero start depth authorised material no program cuts; the tab spans were
  gated on a half-cell halo instead of the band they belong to; the tab parameterisation was read from
  `Contour::tessellate`, which samples only segment ends, so a cell beside the middle of a long edge
  mapped to the edge's start and a tab at 0.9 could never match; and the `On` band applied the
  allowance that the emitter ignores. `deviations` also compared only the grid's shape, so two
  different stocks with the same column and row counts compared silently — the cell size is compared
  too. The remaining limit is recorded, not fixed: a document whose `cut_depth` exceeds the stock's
  thickness reports the difference as uncut, which is the honest reading of "the program does not
  reach the declared depth".
- **L7 (execution, S3b, and a limit rather than a fix).** Applying the amended D-M5-5 turned its premise
  into a measurement: the clean-cut test, run once against the old band, reported `gouge: 1632.0` over
  272 cells on a perfect 100 mm `Outside` cut. The fixed band is `allowance … 2·radius + allowance`, built
  from two offsets through a parity region, and the pocket's allowance is applied as one whole-region
  `Region::offset` so the kernel reads each boundary's nesting depth and islands grow in the same call
  (D-M5-10) — the alternative, per-boundary sign arithmetic in the simulation, would be a second copy of
  the rule the kernel already owns. One disagreement is left standing and recorded as D-M5-11: offsets
  are mitered, removal is a union of discs, so a convex corner over-authorises by up to `4(r² − πr²/4)`
  and a perfect square profile reports ≈`7.73 mm² × depth` of uncut per corner (24 mm³ over 4 cells at a
  1 mm cell, converging to that limit as the grid refines). A round-join offset would remove the
  disagreement and is a kernel feature this milestone does not add; a corner tolerance would be the
  resolution knob D-M5-3 exists to refuse. A pocket with a non-zero allowance whose boundaries cross is
  now refused by name out of `Region::offset`, an edge that needs the allowance to be reachable.
- **L8 (execution, V3, and a contract corrected).** Playback's equality claim is what makes the preview
  honest: after the last move the field must equal `sweep::apply` over the whole stream, asserted cell
  for cell on a 2 mm grid both from one long call and from forty small frames. The task also corrected
  a contract V2 had documented the wrong way round: `set_preview` took a *finished* simulation, which
  makes `reset` impossible and lets the picture disagree with the program from the first frame, so it
  now takes the untouched stock and the preview cuts it. Two smaller decisions are recorded rather than
  hidden: a rapid and a retract are timed at the segment's feed rate because `Segment` carries no rapid
  rate, so the clock never invents a machine number; and playback advances only while the panel is
  open, because a closed panel is not a hidden clock.
- **L9 (execution, S6, the defect M5 found).** `Region::raster_spans` answers `None` for a row that
  lies along a boundary, and `row_spans` retried such a row at `y + ROW_RETRY_NUDGE` only. At a
  region's near edge that `+1e-5` lands inside the material and the row is cut; at the far edge it
  lands outside, clips nothing, and the row was dropped — so the row the far edge sits on, which is
  the row that clears the strip that edge ends, was the one row the lattice never cut. Measured on a
  100 mm square with a 2 mm tool at a 2 mm stepover: rows 1, 3, … 97, a last row whose 2 mm tool
  reaches `y = 98`, and the 2 mm strip from 98 to the edge left standing as 200 cells at 6 mm deep =
  1200 mm3 (S5: `removed: 58800.000 mm3`, `uncut: 1200.000 mm3 over 200 cell(s)`, exit `6`). The fix
  is in `row_spans`, not in the caller and not in the emitter: the retry tries the nominal height, then
  `y + nudge`, then `y - nudge`, and takes the first height that clips **any** material. Two nudge
  directions rather than a tolerance knob, and no "last row" special case; the candidate order is
  fixed, so the retried height is a function of the region and the row height alone and S9's
  determinism is unchanged (the same document still emits the same bytes, pinned by
  `two_simulations_of_one_document_print_identical_bytes` and `raster_pocket_with_an_island_is_deterministic`).
  `flattened` reporting, the retried-height convention and the drop of a row that clips nothing on any
  of the three heights are untouched: `a_row_the_island_covers_entirely_emits_nothing` still reports
  the same 24 rows over its island band, and `a_row_touching_a_narrow_island_strip_never_emits_an_unpaired_span`
  still resolves its collinear row into two paired spans. Two expectations moved, both re-derived
  rather than fitted: S4's raster `simulate` fixture now runs the honest 2 mm stepover (50 rows x 4
  moves = 200 moves, 50 x 98 = 4900 mm cutting length, 10000 cells x 6 = 60000 mm3, uncut 0, exit 0)
  instead of the 1.58 mm stepover that dodged the dropped row, and the island raster golden gains the
  `y = 57` row (45 to 49 lines, 9 spans to 10, `N390G00Y57.0000` through `N420G00Z5.0000`). The offset
  strategy, drilling, profiles and every non-raster golden are byte-identical, which is what says the
  defect was where S5 measured it and nowhere else.
- **L10 (execution, P1, the closing differential, and one decision left to the owner).** Every
  pre-existing subcommand was run against a copy of `master` at `0d96b36` and against this branch in an
  identical sandbox: twelve commands byte-identical in exit code, stdout, stderr and produced files
  (`import`, `tools add`, `tools list`, three `toolpath` runs, `validate`, `inspect`, `check`,
  `postp validate` and `emit` twice, including the authored `.c2d`, the SQLite library and the emitted
  G-code), and exactly two differences — `--help` gaining the `simulate` block and exit code 6, and
  `simulate` existing at all. The first attempt of that script measured nothing (a relative `mktemp`
  path put the base binary where the script did not look, so every base run exited 127); it was
  corrected and re-run rather than reported. The end-to-end report on the mixed fixture then paid for
  itself: `uncut: 48.000 mm3 over 8 cell(s)` decomposed into the profile's four mitered corners and the
  pocket's four, 24 mm³ each at a 1 mm cell, which is D-M5-11's arithmetic measured on a real document;
  and the same run exposed D-M5-12, a raster lattice that stops 2 mm short of the far edge and leaves
  `984 mm³` standing. That one is recorded and not fixed, because finishing the far edge changes M4's
  documented raster contract — a decision for the owner, not for a simulation milestone.
