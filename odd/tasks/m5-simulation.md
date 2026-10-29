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
| S1 | `THIS_COMMIT` | 11 tests (`crates/cam-sim/tests/heightfield.rs`). The default sheet at the default cell is 1219 x 2438 = 2,971,922 cells, under the 4,000,000 cap; a 0.1 mm request names 12190 x 24380, the product and the cap in one message; a zero-sized job and a zero cell are `Degenerate`, not a panic; a 10 mm square at 1 mm has 100 cells whose centres run 0.5 … 9.5; lowering twice keeps the deeper value; a cell clamped at `-thickness` counts as through; a whole field to the bottom reports 500 mm³ removed, 0 remaining and 100 through cells; 25 cells at 2 mm deep report 50 mm³; a 10.2 mm wide job gets 11 columns whose last centre is outside the nominal width, which is the coverage bound stated rather than hidden; and a property test asserts `removed + remaining` equals the grid volume for random cut sequences |
| S2 | _pending_ | |
| S2 | `THIS_COMMIT` | 6 tests (`crates/cam-sim/tests/sweep.rs`), one of them lifted the whole capsule question: a straight feed's cut cells are compared against a brute-forced exact point-to-segment distance over every cell, so the sampled-disc sweep has to agree with the real capsule cell for cell, and the removed volume is that count times the depth. A full-circle arc leaves its centre cell standing (the centre is 0.5 mm from the cell at (20.5, 20.5), and the tool is 1 mm, so the assertion also proves the disc is applied where the arc is); a rapid at clearance cuts nothing and is not a crash; a rapid to -3 is one crash carrying `pocket 1` and move index 1, and the field stays untouched; a feed at +2 cuts air; two runs give identical volumes and identical surfaces cell by cell. `cam_move::Segment` gained `tool_diameter` (the emitter ignores it) and `Heightfield` gained `surface_under` |
| S3 | _pending_ | |
| S4 | _pending_ | |
| S5 | _pending_ | |
| V1 | `THIS_COMMIT` | 13 tests (`crates/cam-view/tests/camera3d.rs`, `tests/mesh.rs`); the crate stands at 36. The target projects to the viewport's centre at four pitches and three yaws; yawing the camera by δ equals rotating the point about the vertical by δ, and a quarter turn carries a point across the centre in the direction the sign says; points behind the eye or not finite are `None` while points in front are not; the pitch clamp holds at both ends and a field poked past it still projects; zoom halves, clamps at `MIN_DISTANCE`/`MAX_DISTANCE` and ignores a zero, a negative, a NaN and an infinity; a 10 x 8 field gives 80 triangles / 126 vertices at stride 1, 12 / 12 at stride 4 and 4 / 2 at stride 100, all indices in range, with a zero stride treated as one; the depth ramp is exactly 0.0 at the untouched top and exactly 1.0 at the stock bottom, 2/18 for a shallow cut; the stock box has 12 edges, 8 corners of degree 3, every edge axis-aligned, spanning `0..width`, `0..height`, `-thickness..0`; two builds are identical vertex for vertex, even across a different camera, and the triangles are ordered back to front by a stable sort. 2D suites untouched (13 + 10) and `cargo test -p cam-ui` still green (14 + 38), which is A12's crate-boundary half |
| V2 | _pending_ | |
| V3 | _pending_ | |
| D1 | _pending_ | |
| P1 | _pending_ | |

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
