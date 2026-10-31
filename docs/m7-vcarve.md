# V-Carve (M7)

V-Carve is the one strategy the roadmap kept out of the mechanical milestones, and the reason is a
sentence rather than a feature: a V-bit's depth is a function of how far the tip is from the boundary, so
the toolpath is not a pass sequence at all — it is a **depth field**, and the field's skeleton is the
medial axis of a region that may contain arcs. `odd/roadmap.md` §4.1 put it in its own milestone for
precisely that: "the algorithm is not recoverable from static analysis — this is a genuine research
task".

M6's shape was the opposite, and the difference is worth stating because it is what the milestone cost.
M6 discovered, before writing any spec, that no tool geometry reached the toolpath layer and that the
move vocabulary could not express a helix, so it opened with two prerequisites. M7 opened with the same
question and got the other answer (L4): `FEED_MOVE` is `"[N]G01[X][Y][Z]"` and the sweep already
interpolates Z along a cutting move (D-M6-12), so a per-vertex Z feed is expressible **today**, and the
milestone changes no post contract at all. What M7 spent instead was geometry — an exact distance field,
a medial axis whose every vertex carries a certificate — and one defect nobody had fixed: the simulation
built its field in document coordinates while programs are emitted in machine coordinates, which is why
M7's first slice is a coordinate-frame fix and not a strategy.

```console
$ cam-cli toolpath part.c2d --kind vcarve --entities E1 --tool 1 --library tools.sqlite
added TP1 vcarve, tool T1, 1 entity(ies)
$ cam-cli toolpath part.c2d --kind vcarve --entities E2 --tool 1 --tolerance 0.1 --library tools.sqlite
added TP2 vcarve, tool T1, 1 entity(ies)
$ cam-cli simulate part.c2d --library tools.sqlite
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 2 toolpath(s)
program: 407 move(s), 55.437 mm cutting length
removed: 4874.366 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
```

The document holds a 20 x 20 mm square at the origin and a disc of radius 15 mm at (120, 50), and the
library's T1 is a 90° V-bit with a sharp point. Two commands appended two toolpaths and the report is
what the shipped code prints: `1340.000 mm³` of the square's analytic `1333.333` and `3534.366` of the
disc's analytic `3534.292` (the difference is the 1 mm grid's quadrature of `∫ d dA`, not lost material).

| Piece | Where |
| --- | --- |
| The exact distance from a point to a region's boundary, arcs native | `cam-geom::region::Region::distance_to_boundary` (`region_distance.rs`) |
| Which boundary point a segment is nearest at | `cam-geom::segment::Segment::nearest_point` |
| The verified medial axis: branches, vertices, clearances, drops | `cam-geom::medial::{medial_axis, MedialAxis, MedialBranch, MedialVertex}` |
| The parameter record and the depth law, once | `cam-model::VCarveParams`, `VCarveParams::depth` |
| The walk | `cam-toolpath::vcarve` |
| The frame that relates the document to the machine | `cam-model::Frame` |
| The authorised depth for a V-carve cell | `cam-sim::target::claim_v_carve` |
| `--kind vcarve`, `--tolerance` | `cam-cli` |
| The contract row for the `v_carve` kind | [format.md](format.md#document-c2d) |

The decisions are D-M7-1 … D-M7-8 and the log is L1 … L12 in the
[feature document](../odd/tasks/m7-vcarve.md); the milestone's evidence is §4.10 of the
[roadmap](../odd/roadmap.md), and the project-level summary is in the [README](../README.md).

## The claim, and why the medial axis is the toolpath

A V-bit cutting with its tip on `p` at depth `z_p` occupies the closed cone above it: at lateral distance
`s` from `p` its flank is at `z_p + s / tan(θ/2)`. The **tool body is above that cone**, so the material
left standing after sweeping a set of tips is the **lower** envelope of their flanks, not the upper one.
Put the tip at

```text
z_p = −d(p) / tan(θ/2)
```

with `d(p)` the exact distance from `p` to the boundary of the region `R`. Then:

1. For any `p` and any `q` in `R`, `|q − p| − d(p) ≥ −d(q)`, because `d` is 1-Lipschitz. So the swept
   surface never rises above `−d(q)/tan(θ/2)`: **the carve can only leave material standing, it can never
   cut deeper than the distance field.**
2. Equality is attained at the tip whose nearest boundary point is the one radially aligned with `q`, so
   the surface is exactly `−d(q)/tan(θ/2)` **provided the tips reach every point of `R`**.
3. `R` is exactly the union of the balls `B(p, d(p))` over the medial axis of `R`, and no ball `B(p, d(p))`
   leaves `R`. That is the definition of the medial axis, and it is why the axis **is** the toolpath: it
   is the smallest set of tips whose cones cover the region.

Two consequences are asserted rather than assumed, and they are what makes this operation's oracle
possible at all. **Gouge is exactly zero for any tip set inside `R`** (point 1) — not "within a
tolerance", exactly zero — and **coverage is complete if and only if the tips' balls cover `R`** (points 2
and 3). Error is therefore **one-signed by construction**: a skeleton piece dropped or misplaced costs
coverage and shows up as `uncut`, never as gouge. That is the same discipline the region booleans
recorded as D-RB-9, and it is why a coarse tolerance is a coverage knob and never a depth knob.

`VCarveParams::depth` is the law's only implementation. `cam-toolpath::vcarve` calls it for each tip and
`cam-sim::target::claim_v_carve` calls it for each cell, so a stored derived depth cannot drift from a
recomputed one — M6's D-M6-2 argument for a chamfer's bevel, applied to a field. On a 90° bit
`tan(45°) = 1`, so the depth **is** the clearance, which is what the oracle's fixtures use: an arithmetic
error in the tangent cannot hide behind `tan(θ/2) = 1`.

## The axis is verified, never guessed

`cam_geom::medial::medial_axis(region, tolerance)` returns branches of `MedialVertex { point, clearance }`
and a count of candidates it dropped. A point is on the axis **because an exact certificate says so**, and
the certificate is the empty-ball test:

- the nearest **boundary points** — deduped by point, so a vertex shared by two segments is one point —
  number at least two; or
- the nearest set is an interval and every point of it is at the same exact distance (an arc asked about
  its own centre).

`clearance` is `Region::distance_to_boundary`'s answer, which is exact and arc-native: the centre of a
circle reports the radius, not the apothem of an inscribed chord. A candidate that fails is dropped
**whole**, never nudged towards the axis — a nudged tip is a lie about its own clearance — and
`MedialAxis::candidates_dropped` counts every refusal. `every_retained_vertex_of_every_fixture_is_a_
certified_medial_point` asserts the certificate on seven fixtures, and the fixtures' expected branches are
asserted by name: a square's four diagonals meeting at the centre, a disc's centre alone, a rectangle's
single midline that ends where the corners take over, an L's branch through its reflex corner, an
annulus's closed loop at the mid radius, an ellipse's axis on its major axis.

Candidates come from a **nearest-feature lattice** refined by **bisecting the scalar** `dist(p, i) −
dist(p, j)` along a lattice edge (D-M7-2, D-M7-4). This is the milestone's one amended decision: the
specification first said candidates would come from "the bisectors of boundary element pairs", and every
bisector involving an arc is a parabola or a hyperbola, so the plan assumed arcs would be tessellated for
the skeleton. The implementation never builds a bisector, only roots a scalar, so **arcs stay arcs and
nothing is tessellated**: the `arcs_tessellated` field the specification asked for was structurally `0`
and was removed. D-M7-4 is recorded as superseded in its premise rather than deleted, because its
reasoning is exactly why the alternative looks necessary.

Topology is an answer, so it is repaired rather than inherited. Two measurements made it so. An
**unverified contact** — two arcs that meet at a joint disagree about that joint's coordinates by up to
`1.3e-5` mm, so a point just off the joint's normal is nearest one arc's interior and the other's endpoint
at the same distance — grew a stub of "axis" at every joint of the 5 x 2 ellipse's sixteen-arc boundary and
reported **13 branches at a 0.1 mm tolerance against 1 at 0.5 mm**; the contact dedup is now read at a
tenth of the tolerance, which can only ever reject a candidate. And a **refused candidate is a lattice
crossing that disappears**, so the one-cell-at-a-time walk ends on both sides of it; a **stitch pass**
joins two branch ends within two lattice steps when each points into the seam and the seam crosses nothing.
No vertex is invented and none is recomputed: the square's four branches stayed four, the L's ten became
nine, and the ellipse's thirteen became one. The property that guards all of it is
`a_finer_tolerance_never_returns_more_branches_than_a_coarser_one`: a finer tolerance that bought a better
measurement by fragmenting the toolpath into thirteen walks would retract the tool thirteen times, and the
test says so.

Determinism is a design constraint, not a hope (S10): the lattice is fixed by the region's bounds and the
step, the labels are broken by segment index, cells are visited row-major, the vertex index and the branch
sort are ordered containers, and seams are joined in order of gap and then branch index. Two runs return
the same branches in the same order, vertex for vertex
(`two_runs_return_the_same_branches_in_the_same_order_vertex_for_vertex`), and no hash iteration reaches
the output.

## What the walk emits

```text
cam-cli toolpath <document> --kind vcarve --entities <ids> --tool <n>
                 [--tolerance <mm>] [--library <path>] [--out <path>] [--id <id>] [--name <name>]
```

Per branch, in the axis's own order: a **rapid to `safe_z`** over the branch's first vertex, a **plunge**
there to that vertex's own depth, **one `Move::Feed` per further vertex carrying that vertex's own Z**, and
a **retract to `safe_z`** (A3). A branch that is a loop returns to its first vertex, so the loop's closing
edge is cut too; a branch of one vertex — a disc's whole axis — is a plunge and a retract, which is exactly
what a disc's V-carve is. Branches are separate walks and do not link (D-M4-6, unchanged): a branch that
merely *looks* adjacent can be on the far side of a wall.

**No arc move is emitted, and none is needed** (D-M7-7). `Move::Arc` is constant-Z by contract and a
V-carve's Z is a function of *position*, so the walk is linear with a per-vertex Z. The alternative —
chording the level sets into arc blocks — would trade the law's exactness for a shorter file, and M7 had
no evidence that file length matters at this scale. The existing `FEED_MOVE` block carries `[Z]`, so the
post contract is untouched, and the sweep measures the move honestly (S12, S13).

Two whole programs are pinned byte for byte, each asserted **three ways** — as built, after a `c2d` save
and load round trip, and with the toolpath authored through `cam-cli toolpath`
(`a_square_v_carve_emits_the_golden_program`, `…_survives_a_document_save_and_load_round_trip`,
`…_authored_through_the_cli_…`, and the same three for `a_circle_v_carve_…`). Both are derived and then
read against the geometry, never captured and blessed:

| Fixture | Program | What reading it establishes |
| --- | --- | --- |
| 20 x 20 mm square, 90° bit, 0.1 mm axis | **413 lines**, 4 branches, 396 feed tips, 10 rapids | each branch starts 0.2 mm inside a corner at `Z-0.2000` and walks the diagonal to `(10, 10)` at `Z-10.0000`, the apex; consecutive tips are `0.1·√2 = 0.1414` mm apart; **every** Z equals the tip's own exact distance to the boundary (worst deviation `1.4e-15` mm), which is the field, line by line |
| disc of radius 15 mm, 90° bit, 0.1 mm axis | **12 lines**, 1 tip, 0 lateral moves | one plunge at `(25, 15)` to `Z-15.0000`: the axis of a disc is its centre, so the whole carve is one cone |

```console
$ cam-cli emit part.c2d --post post_mm.pp --library tools.sqlite --out square.nc
$ sed -n '1,8p' square.nc
( V carve 1 )
( CamPilot fixture Mach3 Arcs (mm) (*.nc) )
N0G00G17G90G21
N10T1M06
N20S18000M03
N30G00X0.2000Y0.2000
N40G01Z-0.2000F100.0
N50G01X0.3000Y0.3000Z-0.3000
$ tail -4 square.nc
N4070G00Z5.0000
N4080G00X0.0000Y0.0000
N4090M05
N4100M30
```

The plunge enters at the first tip's own depth and the feed that follows descends with it. No rapid in
either program is taken below `Z5.0000`, the job's `safe_z` — replayed line by line, the tool is at safe
height on every `G00` — and neither program contains a single `G02`/`G03`: the walk is linear with a
per-vertex Z, asserted rather than assumed.

## What it refuses

A V-carve has two inputs of its own — the region and the bit — and no depth, no side, no stepover and no
pass law. It refuses by name everything whose meaning it does not have (A5, S16's precedent from M6):

| Refusal | Raised by | Message |
| --- | --- | --- |
| `OpenContour` | the strategy | `cutting toolpaths need closed geometry` |
| `InvalidIncludedAngle` | the strategy | `chamfer included angle 0 degrees must be strictly between 0 and 180` — the sentence names a chamfer because the refusal is shared, see below |
| `VCarveNeedsAngledTool` | the strategy, and the CLI before it writes | `tool T3 has no included angle in the library, so a V-carve's depth cannot be derived: a V-carve needs a V-bit` |
| `VCarveNeedsSharpBit` | the strategy, and the CLI before it writes | `tool T3 has a 0.4 mm flat tip, so it cannot carve to a sharp edge: a V-carve's depth law is the cone of a sharp point, so a V-carve needs a sharp V-bit` |
| `MedialAxis` | `cam_geom::medial`, surfaced by the strategy | `the V-carve's medial axis could not be computed: a tolerance that would need an unbounded lattice …` |
| `VCarveNothingToCarve` | the strategy, and the CLI before it writes | `the V-carve's 50 mm tolerance leaves nothing to carve in the 20 x 20 mm region: every point of the region is within that distance of its boundary, so the medial axis is empty and there is no tip to plunge` |
| 17 flags its law does not have | `cam-cli toolpath` | one `--kind vcarve takes no <flag>: …` per flag |

The angle's sentence still says "chamfer", because `InvalidIncludedAngle` is the chamfer's refusal and M7
reuses it rather than reword a message M6 pinned: the value, the range and the exit code are the same fact
for both operations, and a second variant would only duplicate it. Recorded so the wording is a known wart
rather than a surprise.

The angle is checked twice on purpose, exactly as the chamfer checks it: the document refuses an angle
outside `(0, 180)` first, and the strategy refuses it again, because `tan(0)` is zero and a caller that
reached `vcarve()` without passing through a validated document must not receive an infinite Z. The
**flat tip** is refused rather than absorbed: with a flat of diameter `f` the surface its tip leaves is
`f/2 · cot(θ/2)` wide at the bottom, so the law of a sharp point is the law of a bit that is not in the
machine — the chamfer's D-M6-10 in the other direction. The CLI refuses it before anything is written, and
the strategy refuses it again, because the library can change under a document written against a sharp bit.

**An empty axis is not a carve.** A tolerance at least as large as the region's own clearance puts every
point of the region within that step of its boundary: no point qualifies as material, no candidate is
certified, and the axis comes back with no branch. Before T7 the command wrote that document anyway, and
the only symptom was a later `simulate` reporting the whole field as uncut (exit 6). Both numbers are in
the command's hand, so the refusal names them and exits `2` with the file byte-identical;
`emit` and `simulate` refuse the same document in the same words, because they compute the same walk.

The 17 refused flags are `--cut-depth`, `--start-depth`, `--pass-depth`, `--stepover`, `--allowance`,
`--side`, `--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--pitch`,
`--chamfer-width`, `--inlay-allowance`, `--clear` and `--strategy`. Each names itself and the reason it has
no meaning here rather than being accepted and ignored — a `--stepover 2` nobody applied still looks
applied. `--start-depth` is one of them, and that is D-M7-3 rather than an omission: with a start depth `s`
the cone's top reaches `s · tan(θ/2)` **outside** the vectors, so the authorised surface is no longer
`−d(q)/tan(θ/2)` inside the region alone, and the milestone's cleanest assertion would be false for a
legitimate program.

## The tolerance, and its four declared costs

`--tolerance` is the axis's **lattice step**. It is the same *kind* of knob the product calls `VCarve
Toolpath Tolerance` — "tolerance to apply to VCarve toolpaths after calculating to reduce file size" — but
not the same knob: the product simplifies a computed path afterwards, while M7 refines the axis to a step.
It is stored in the document (default `0.1` mm) because `emit` recomputes the axis from the document alone
(S9, S10); a flag that was accepted and then dropped would make the program depend on a constant nobody
named. The four costs are declared in `cam_geom::medial`'s module doc and measured, not asserted:

| Cost | What it is | Measured |
| --- | --- | --- |
| The chording between certified vertices | the polyline between two tips is straight while the axis between them is not; bounded by the step | at a 0.1 mm step, consecutive tips are `0.1·√2` mm apart on a square's diagonal |
| A piece shorter than the step | a spur of the axis shorter than the lattice step is missed entirely | the axis's balls still cover `0.99670`–`1.00000` of every fixture at 0.1 mm |
| The convex-corner band | a tip whose clearance is at or below the step is not material, so the carve stops one step short of each convex corner | **0.20 mm** of missed depth at 0.5 mm, **0.00** at 0.1; on the oracle's 20 x 20 square on its 0.5 mm grid that is `uncut 0.207 mm³ over 4 cell(s)` at 0.5 and `0.000` at 0.1 |
| A curvature extremum truncates a branch | at the ends of an ellipse's major axis the two nearest contacts converge on one region of boundary, so the coarse lattice stops offering a candidate | the 5 x 2 ellipse's axis reaches `|x| = 3.5` against its analytic `4.2` at 0.5 mm, and `4.8` at 0.1 mm |

Coverage — the fraction of the region the axis's own balls reach — measured on six fixtures at both
tolerances (`the_axis_balls_cover_the_region_at_both_tolerances`):

| Fixture | 0.5 mm: branches, coverage, largest miss | 0.1 mm: branches, coverage, largest miss |
| --- | --- | --- |
| square 10 x 10 | 4, `0.98894`, `0.20000` mm | 4, `0.99696`, `0.00000` mm |
| disc r10 | 1, `0.99997`, `0.00000` | 1, `0.99997`, `0.00000` |
| rectangle 20 x 6 | 5, `0.98791`, `0.20000` | 5, `0.99670`, `0.00000` |
| L 10 x 10 | 9, `0.98585`, `0.20000` | 9, `0.99855`, `0.00000` |
| annulus 4/10 | 1, `0.99890`, `0.00750` | 1, `0.99996`, `0.00000` |
| ellipse 5 x 2 | 1, `0.97694`, `0.19964` | 1, `1.00000`, `0.00000` |

Every one of the six is **one-signed**: the missing material is a band of at most the step along a convex
corner, never a deeper cut. Stabilising the topology moved no coverage figure at all — it changed how many
walks the same certified points form, not what they cover. `a_finer_tolerance_never_covers_less` pins the
direction, so the tolerance cannot be "tuned" into a wrong answer.

## What the simulation authorises, in both signs

For a V-carve cell the authorised depth is `d(centre) / tan(θ/2)`, clamped to the stock, computed from the
document alone — `Operation::VCarve(params)` names the region and the angle, and the region is the
toolpath's own parity union, so the claim and the walk read the same geometry. `cam-sim`'s fixtures run on
a **0.5 mm** grid with a **0.1 mm** axis, four times finer than the simulation's default, so the volume
arithmetic is measured on a lattice fine enough to see the field rather than its cell corners:

| Fixture | Analytic | Removed | `uncut` | `gouge` | Deepest |
| --- | --- | --- | --- | --- | --- |
| 20 x 20 mm square, 90° | `w³/6 = 1333.333` | 1332.5 | `0.0` | **`0.0`** | −10 |
| r = 15 mm disc, 90° | `πr³/3 = 3534.292` | 3534.152 | `0.0` | **`0.0`** | −15 |
| the square's axis halved | 1333.333 | 1189.802 | `142.043` (642 cells) | **`0.0`** | −10 |
| the square at 20% too deep | 1333.333 | 1962.155 | `0.0` | **`629.557`** (1937 cells) | −12 |
| the square's axis at 0.5 mm | 1333.333 | 1332.5 | `0.207` (4 cells) | **`0.0`** | −10 |

The two correct carves are short of the analytic volume by `0.833` and `0.140 mm³` — the cell-centre
quadrature of `∫ d dA`, not lost material: `uncut` is exactly zero because every cell's own centre is
reached to its own field value. The rows below them are the two signs the criterion demands, and neither
can pass as the whole truth: a **deliberately thinned skeleton** (half the square's branches dropped)
leaves material standing and gouges **nothing**, and a program at **20% more depth** than its clearance
allows is reported as gouge in the same cells the correct one is exact in. The 629.557 mm³ is more than
the 266.667 a fifth of the carve would be, and the excess is the shape of the same error: a 20% taller cone
is 20% wider, so it reaches cells the correct carve never touched.

End to end through the binary, `simulate_reports_a_correct_vcarve_with_no_gouge_and_no_uncut_material`
asserts the same claim on a 40 x 30 rectangle: `gouge: 0.000`, `uncut: 0.000`, `crash: none`, exit `0`, and
the emitted program's deepest line is `Z-15.0000` — half the short side, which is the region's largest
clearance.

## The frame, and the three defects T0 found outside its scope

M6 recorded a defect and did not fix it: `cam_sim::grid::grid_for` built the field at `(0, 0)` from
`job.width/height`, i.e. in document coordinates, while programs are emitted in machine coordinates, so at
any origin corner other than `bottom_left` the report and the preview disagreed with the document about
*where* the cut is (S17). M7's acceptance test is a simulation report, so the fix had to come first
(D-M7-6), and it is the roadmap's own recorded next step rather than new scope.

`cam_model::Frame` is now the one definition of the mapping — a translation and, at `top_left`, a Y
reflection — and it is honoured at every boundary: the field's grid and the sweep's cell mapping, the
program's own coordinates, and the handedness of every arc. The implementation found a cheaper and safer
crossing point than the roadmap's sketch, and the reason is a tab (D-M7-8): M5 places a tab by **arc length
from the contour's start**, and reflecting a contour reverses its traversal, so mapping the *target's*
geometry into the machine frame would move every tab to the other side at `top_left`. The field, the target
and the preview therefore stay in the document frame, and the program is read into it once, in
`sweep::apply_move`. What the sketch got right is the invariant — exactly one mapping, at exactly one
boundary — and that is what the code does.

Measured on M6's own fixture (an 80 x 30 mm band pocketed at `y = 60 … 90` in a 200 x 100 mm sheet, a 6 mm
tool, 1 mm cells), before and after:

| Origin | Before (`9ff0b8c`) | After (`786c3d8` + `228ecc7`) |
| --- | --- | --- |
| `bottom_left` | removed 14376.000, gouge 0.000, uncut 24.000 over 4 cells | unchanged, byte for byte |
| `top_left` | removed 14376.000, **gouge 14376.000 over 2396 cells**, **uncut 14400.000 over 2400 cells** | removed 14376.000, gouge 0.000, uncut 24.000 over 4 cells |
| `center` | M6's record, on a 100 x 80 mm square: `removed` collapsed from 47976.000 to 2394.000 | the bottom-left numbers, measured on the same band fixture at every corner |

Two further defects came out of T0's own work and neither was anticipated by the feature document, which
is why A7 was widened to cover arc **handedness** and T0 was widened with it:

1. **A reflection reverses orientation.** An arc emitted at `top_left` cut nothing where the document's own
   arc cut `-4 mm`; the sweep's mapping now flips `ccw` when the frame reflects.
2. **The emitted program itself was wrong, since M4.** `cam-toolpath` passed `ccw: arc.is_ccw()` through
   unchanged while mapping only the points, so at `top_left` the **program** stated the complementary arc —
   a `G02` where the machine needs `G03`. That is not a reporting error: it is a wrong cut on real
   hardware, and no fixture noticed because every fixture is bottom-left.
3. **The thread's helix had the same defect in a second half only the first fix revealed.** `thread.rs`
   argued its helix was already in machine coordinates, and only the orbit's *start* was: the strategy
   mapped the centre but offset the start along **+x**, which a Y reflection leaves alone, and computed the
   fractional turn's end with `+sin` around that machine centre. `top_left` emitted the opposite orbit and
   the opposite hand — a left-hand thread from a document that describes a right-hand one.

All three construction sites in the workspace that build a `Move::Arc` or `Move::Helix` from document
geometry now state their direction in the machine's numbering, and the tests state the rule that matters:
a program read back into the document frame must be the program `bottom_left` emits
(`every_origin_corner_emits_the_program_bottom_left_emits_once_read_back`). Bottom-left programs are
byte-identical after all of it: five `emit` fixtures compared with `cmp` and the thread's own move dump, 22
`G02`/`G03` lines among them, no difference.

## Determinism and the old bytes

Two runs of every new subcommand print and emit identical bytes:
`a_vcarve_tolerance_is_byte_identical_across_runs` runs `toolpath --kind vcarve` twice on two copies of the
same document and compares stdout, the documents and the emitted program byte for byte, and each golden
test emits its program twice. No map iteration reaches an output path: the axis is ordered by first vertex
and then by vertex sequence, the lattice walks in one direction, and the ids a command invents are the
first free `TPn`.

The pre-existing command surface was then run against `9ff0b8c` in an identical sandbox — same document,
same library, same posts, same geometry, relative paths — **35 invocations**, each from a fresh copy of the
fixture set, comparing exit code, stdout, stderr and **every produced file** (documents as their
`document.json` payloads, programs byte for byte, the imported document, the library): `validate` (a good
document, an unreadable one, an unsupported schema, a profile carrying islands, a missing path),
`inspect`, `check` (an open contour and a clean document), `postp validate` (a good post and a malformed
one), `tools list`, `tools add` (and its missing-argument path), `toolpath` for **profile**, **pocket
offset**, **pocket raster with an island**, **drilling with a bank** and **thread**, `tools add`'s refusals,
`toolpath`'s `--islands`-on-a-profile refusal, a missing entity and an unknown kind, `emit` to a file and to
stdout, `emit` with a post that has no arc block and with one that has arcs but no helical block, `emit`
without a library, `simulate` at **1, 0.5 and 2 mm** and at a resolution the cell cap refuses, `import` as a
report and with `--out`, `import` of a missing file, an unknown command, and `--help`. **844 file snapshots**
were hashed.

**Two differ, and both are supposed to:**

| Case | Before | After |
| --- | --- | --- |
| `--help` | lists the nine kinds and no `--tolerance` for a toolpath | lists the ten kinds, `--tolerance`, and what a V-carve's step costs |
| unknown `--kind` | `…; expected profile, pocket, drilling, chamfer, engrave, fluting, texture, inlay or thread` | `…; expected profile, pocket, drilling, chamfer, vcarve, engrave, fluting, texture, inlay or thread` |

Everything else — the `--islands` refusals, the arc and helix post refusals, the five-toolpath `simulate`
report at three resolutions, the emitted program, the imported document and the library — is identical,
byte for byte, and so is `validate`'s sentence for a profile toolpath carrying islands ("takes islands only
for a pocket or an inlay"), which the CLI cannot produce but a hand-edited archive can. `Cargo.lock` is
unchanged between `9ff0b8c` and this branch: the same **470** `[[package]]` entries.

## Review

**This milestone's diff has had no native outside view.** The candidate was submitted twice and both times
was refused in preflight with `lens_context_budget_exceeded` (`mutation_performed: false`, no authority
created, nothing abandoned, `retry_safe: true`): **4 425 inserted lines across 7 commits**, with `--base-ref`
set to the branch's cut point `9ff0b8c` and `committed-only` on. The second attempt asked for **one** lens
(`risk`) instead of three and was refused identically, which is the fact worth keeping: the budget is
bounded by the candidate's size, not by the lens count, so tuning the lens selection cannot make an
oversized candidate fit — only smaller commits can. The owner's disposition was to disable the review switch
for this clone and deliver M7 on its own evidence, the path M6 took: `gentle-ai review mode disable --scope
clone` reads `receipt-driven development: off (decided by clone_local)`. The consequence is recorded rather
than softened: the milestone's delivery rests on its tests, its simulation oracle in both signs, its two
golden programs and its differential — not on an outside review that never ran.

## Known limits, each a decision

- **No `start_depth`** (D-M7-3). The product's dialog has `_vcpdStartDepth`; M7 refuses `--start-depth` by
  name rather than storing a field that defaults to zero. With a start depth the cone's top reaches outside
  the vectors and the authorised surface is a claim of its own, and shipping it would make "gouge is
  exactly zero" false for a legitimate program.
- **No flat tip** (`VCarveNeedsSharpBit`). The law is the cone of a sharp point; a flat of diameter `f`
  would leave a floor `f/2 · cot(θ/2)` wide and the operation would no longer meet the vectors at the
  edge. Refused at all three boundaries that can see the tool.
- **No corner relief** (D-M7-1, the owner's "M7 es V-carve a punta pura"). No extra pass, no extended walk.
  The law reaches every boundary point — at a boundary `d = 0`, so the surface tapers to zero depth exactly
  at the edge, sharp corners included — and what a real machine leaves in a corner is the tip's own finite
  radius, which the simulation reports as `uncut`.
- **No flat bottom** (`_vcpdFlatDepth`, `_vcpdDoFlatBottom`, `_vcpdUseAreClearTool`, the owner's "Nada
  extra: `simulate` lo reporta como uncut"). Recorded out of scope rather than dropped: it changes what the
  operation *is*, and it is a milestone of its own.
- **No `PreserveArcs`** (`_vcpdPreserveArcs`). It cannot apply: **arc output is not preserved because the
  walk is linear with a per-vertex Z.** A constant-`d` contour of the distance field is not a constant-Z
  curve, so `Move::Arc` — constant-Z by contract — cannot carry it (D-M7-7). The cost is file length, and
  the benefit is that the surface is the field vertex for vertex.
- **The tolerance is coverage, never depth.** All four of its costs above are material left standing. A
  finer step is a longer program; a coarser one leaves up to the step standing at a convex corner.
- **A region thinner than the step is refused, not approximated.** An empty axis means there is no tip to
  plunge, so `toolpath`, `emit` and `simulate` all refuse it by name with the tolerance and the region in
  the message.
- **The CLI still ignores a flag its kind does not use, milestone-wide** (D-M6-11, still open). Closing it
  changes the contract for callers that pass a superset today. `vcarve` refuses the 17 flags its own law
  forbids, which keeps its contract honest where M7 touched it.
- **No 3D, no laser, no projection, no toolpath form in the window** (D2, S15, S18,
  `_vcpdProjectToolpath`). The V-carve is authored by `cam-cli toolpath` like every other strategy.

## Where the record lives

| Question | Where |
| --- | --- |
| What M7 had to be, and every amendment to it | `odd/tasks/m7-vcarve.md` — S1 … S18, A1 … A8, D-M7-1 … 8, L1 … L12 |
| What the `v_carve` kind stores | [format.md](format.md#document-c2d), one row per `kind` |
| What the simulation authorises | [m5-simulation.md](m5-simulation.md#the-target-surface) |
| How a profile, a pocket and a drill behave | [m4-toolpaths.md](m4-toolpaths.md) |
| How the depth-variant strategies behave | [m6-toolpaths.md](m6-toolpaths.md) |
| How islands, parity and the `@2` tag work | [region-booleans.md](region-booleans.md) |
| What M7 delivered, and what it cost | [odd/roadmap.md](../odd/roadmap.md) §4.10 |
| The project-level summary | [README.md](../README.md) |
