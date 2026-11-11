# Moulding (M8)

Moulding is the eleventh and last strategy in the scope the roadmap fixed at M0: a **rail** in XY, a
**cross-section** of `(lateral, depth)` points swept along it, and a **stepover** that spaces the
overlapped lateral passes. It is the first operation in the tree whose cutting law is a cross-section
rather than one number, and `odd/roadmap/index.md` gave it a milestone of its own for exactly that reason —
"Needs the 3D-adjacent math from M7's distance field".

What it cost is worth stating against M6 and M7, because M8 is the cheap one. M6 discovered that no tool
geometry reached the toolpath layer and opened with two prerequisites. M7 spent a research unit on the
medial axis. M8 changed **no post contract** — a moulding's Z is constant *within* a pass, so
`FEED_MOVE`'s `[N]G01[X][Y][Z]` carries it as it stands — added **no** variant to `c2d/document@2`
beyond `Operation::Moulding` itself, and needed no new dependency. What it did need was geometry: a rail
walked by **arc length** with the contour's arcs kept as arcs, a cross-section that is a *function* of
lateral distance, and a stepover law whose coverage is a certificate rather than a hope.

```console
$ cam-cli toolpath part.c2d --kind moulding --entities E1 --tool 1 \
      --profile 0:-1 --profile 2:-3 --profile 4:-1 --stepover 2 --library tools.sqlite
added TP1 moulding (3 points, 2 mm stepover), tool T1, 1 entity(ies)
$ cam-cli simulate part.c2d --library tools.sqlite
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 12 move(s), 180.000 mm cutting length
removed: 300.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
```

The document holds one 60 mm rail at `y = 50.5` and the library's T1 is a 0.5 mm bit, so the three
passes are rows 50, 48 and 46 at 1, 3 and 1 mm deep: 60 columns of 1 mm cells × 5 mm of depth is the
`300.000 mm³` above, and both deviation signs are zero because the claim and the program read the same
rail, the same profile and the same stepover.

| Piece | Where |
| --- | --- |
| The rail: arc-length parametrisation, exact normals, arcs native | `cam-geom::moulding::{Rail, RailSample}` |
| The cross-section as one depth per lateral distance | `cam-geom::moulding::CrossSection` |
| The swept band and its containment certificate | `cam-geom::moulding::SweptBand` |
| The pass family and its coverage radius | `cam-geom::moulding::StepoverLaw` |
| The parameter record (`profile`, `stepover`) and its validation | `cam-model::MouldingParams`, `check_moulding` |
| The walk | `cam-toolpath::moulding` |
| The authorised depth for a moulding cell | `cam-sim::target::claim_moulding` |
| `--kind moulding`, `--profile`, `--stepover`, its refusals, `inspect` | `cam-cli` |
| The contract row for the `moulding` kind | [format.md](format.md#document-c2d) |

The owner's scope decision (S3, verbatim: "Sección Z barrida por el rail" — "the Z section swept by the
rail", with "el rail va en XY, el perfil dice (distancia lateral, profundidad Z), stepover son pasadas
laterales solapadas") is the whole specification M8 had, and the five decisions below are its reading.
The feature document is `odd/tasks/019-m8-moulding.md`; the milestone's evidence is §4.11 of the
[roadmap](../../odd/roadmap/index.md), and the project-level summary is in the [README](../../README.md).

## What S3 decided, in five decisions

| Decision | What it means | Where it is pinned |
| --- | --- | --- |
| Lateral is measured **to the right of travel** | `Rail::normal_at` is the tangent turned clockwise, so a rail along `+X` has its positive lateral side towards `-Y`, and a **counter-clockwise closed rail has its positive side outside the loop** | `cam-geom`'s moulding tests and the closed-rail oracle: a profile of `-3` at laterals `-2` and `+2` and `-5` on a circle's rail claims material inside **and** outside the loop and nothing in the middle |
| The rail is **open or closed** | An open rail sweeps a strip; a closed rail sweeps a frame. The document refuses neither | `check_moulding` and the closed-rail oracles in `cam-sim` |
| The profile's laterals **strictly increase** | A cross-section is one depth per lateral distance, so a profile that doubles back would be two depths for one place; refused by name at load, in the strategy and in the claim | `cam-model::check_moulding`, `cam-toolpath`'s `MouldingProfileLateral`, the CLI's `--profile` refusal |
| A stepover **equal to** the profile's width is the **two edge passes** | The law's start-and-step produces them and does not collapse them into one | `StepoverLaw`'s own docs and its tests |
| A stepover **wider than** the profile is **one centred pass** | With that spacing no second distinct pass exists, and a pass at one edge would cut one authored edge and leave the other; the centre is the honest single answer, and its coverage radius is still half the *profile's* width, which is less than half the requested stepover | `StepoverLaw`'s tests, and the V-bit oracle below (a 0.5 mm profile at a 10 mm stepover is one pass on `y = 50.25`) |

The stepover law itself is: start at the cross-section's minimum lateral distance, step by the stepover
while the next pass stays inside the span, and always make the maximum lateral distance a pass. That
makes every gap at most one stepover, both authored edge depths cut, and the **coverage radius** — the
furthest any lateral position in the span can be from a pass — at most **half a stepover**;
`StepoverLaw::covers` is that sentence as one predicate, so the strategy and the claim state the
guarantee they rely on instead of restating the arithmetic. The law lives in `cam-geom` and not in
`cam-toolpath` because it is lateral-offset geometry: a strategy that repeated it would be a second
implementation of the same certificate, and the two could disagree.

## The rail, and why arcs stay arcs

`Rail` stores the contour's **own segments** and maps an arc-length distance to the point, tangent and
normal there with the segment's exact equation, so a point asked for on an arc is on the arc and not on
a chord. Tessellation happens in exactly one place — `Rail::samples`, where a caller asks for a walk —
and that walk chords an arc to `DEFAULT_CHORD_TOLERANCE` (`0.001` mm). The walk keeps **one sample per
segment end**, which means a corner appears **twice**, once arriving and once leaving: a corner really
does have two normals, and averaging them would invent a direction the rail never has and cut across
the corner. Both the strategy and the claim use that same walk (`Rail::samples(DEFAULT_CHORD_TOLERANCE)`),
and that shared walk is what makes the containment argument below a *statement* rather than a
coincidence.

## What the walk emits

```text
cam-cli toolpath <document> --kind moulding --entities <rail ids> --tool <n>
                 --profile <lateral>:<depth> [--profile …] [--stepover <mm>]
                 [--library <path>] [--out <path>] [--id <id>] [--name <name>]
```

Per pass, in the stepover law's order: a **rapid** to the pass's first point at the job's `safe_z`, a
**plunge** there to the depth the cross-section authors at that lateral offset, **one `Move::Feed` per
point of the pass's offset walk** ([`Rail::offset_walk`]) at that same Z, and a **retract** to `safe_z`
(D-M4-6's rule between passes, unchanged: a pass that merely *looks* adjacent can be on the far side of
a wall). The walk is one function in the kernel and not a convention each boundary restates, because
the claim authorises against the same polyline: `offset_walk` is the samples offset by the pass's
lateral, plus the **closing edge** a closed rail needs when its seam sits on a corner — where the
walk's last sample and its first are the same *point* carrying two different normals — and nothing at
all when the seam is in the middle of an edge, where the two tangents are collinear and the two offset
points are one point up to `SEAM_EPSILON`.

The Z is a property of the **lateral offset**, not of the rail position: every pass sits at the depth
its own cross-section point demands, so the program is a family of Z heights rather than one depth. No
arc move is emitted, and none is needed: `Move::Arc` is constant-Z by contract, and the corner argument
above forces the walk to carry both normals as points and bridge them with a straight feed — inventing an
arc the rail never had would be a different cut. On a corner whose two offset samples coincide (the
pass on the rail itself) the repeated sample emits a **bare `G01` block**: a zero-length feed, visible
in the frame golden below, and the price of not averaging two real normals into one invented direction.

Two whole programs are pinned byte for byte, each asserted **three ways** — as built, after a `c2d` save
and load round trip, and with the toolpath authored through `cam-cli toolpath`:

| Fixture | Program | What reading it establishes |
| --- | --- | --- |
| a 60 mm open rail, profile `-1, -3, -1` at laterals `0, 2, 4`, 2 mm stepover | **21 lines**, 3 passes, 3 plunges | the passes are rows 50, 48 and 46 at `Z-1.0000`, `Z-3.0000`, `Z-1.0000` — the law's laterals and the profile's depths, one rapid, one plunge, one feed and one retract each |
| a 40 x 20 mm closed frame, flat profile 2 mm wide at `-2`, 2 mm stepover (the two-edge-pass case) | **33 lines**, 2 passes, 4 bare `G01` blocks | the inner pass is the frame itself with a repeated corner sample per corner, and the outer pass is the frame 2 mm out with a 45 degree diagonal across each corner (`N180G01X72.5000Y40.5000`) — the offset family's corner, not a chamfer the rail does not have |

## Why `gouge` is zero by construction

The argument is **coverage plus a monotone tool law**, and it is different from the V-carve's (M7 rests
on a 1-Lipschitz distance field):

1. **The claim's footprint contains the program's.** Both walk the rail with the same
   `Rail::samples(DEFAULT_CHORD_TOLERANCE)`, so the pass *polyline* the claim measures is the polyline
   the program cuts. The sweep then samples that polyline more finely (half a cell per move), and every
   one of its samples lies **on** it, so a cell's distance to the program's nearest sample is never
   smaller than its distance to the polyline. With `REACH_SLACK` (`1e-9` mm), every cell the program
   can reach is claimed.
2. **The claim's depth there is never shallower.** The tool's `rise` grows with the radius
   (`ToolShape::rise_at_radius`), and the program's sample is at least as far away, so
   `cut − rise(claimed distance) ≥ cut − rise(the program's distance)`.

So a correct program can never remove more than was authorised, for a cylinder and a cone alike, and
whatever the rail's curvature: material removed can never exceed material authorised, which is why
`gouge` is **exactly zero** for a correct moulding and why a dropped pass or a too-shallow cross-section
costs `uncut` and nothing else. The claim reads the cross-section, the stepover and the rail **from the
document** — never the emitted moves — so a claim built from the program would agree with it by
construction, including where both are wrong (M6 A5, D-M6-6). Claims are never summed: the deepest
claim wins (D-M5-2), which a test pins by sweeping two mouldings along one rail.

**One corner of that argument was false as M8 first shipped it, and the fix is the walk's own law.** A
**closed** rail whose vertex list **starts at a corner** has a seam where the walk's last sample and its
first are the same *point* carrying two different normals: the strategy bridges them with one closing
feed across the corner, and while the claim measured a walked path with `windows(2)` alone, that
diagonal was an edge of the program that nothing authorised. On the 40 x 20 mm frame below it was
**`gouge 2.000 mm³` over one cell** — cell `(29, 39)`, the seam diagonal's own centre — on a program
that was otherwise exact. The trigger is the drawing's own start vertex, arbitrary in a DXF and
wherever the user began the rectangle in the shell, so it was a real case and not a corner case in the
colloquial sense. The fix is not a special case on the claim's side: the pass's polyline is
[`Rail::offset_walk`] in `cam-geom`, the strategy emits it and the claim authorises against it, so the
two cannot disagree about where a pass cuts. The corner frame is now exact in both signs — `496.000`
mm³ removed against `496.000` claimed, over the seam-smooth frame's own **248** cell centres — and
`cam-sim/tests/moulding.rs` pins it as
`a_closed_rail_whose_start_vertex_is_a_corner_claims_its_seam_diagonal`. A seam **in the middle of an
edge** has collinear tangents, `offset_walk` appends nothing, and its numbers are unchanged, which is
what makes this a fix to one case rather than a renumbering of the frame.

## What the simulation authorises, in both signs

For a moulding cell the authorised depth is the deepest pass's `cut − rise(distance)`, capped at zero,
computed from the document alone; the deepest claim wins and nothing is summed. The oracles run on a
**1 mm** grid with a **0.5 mm** bit, and the fixtures' coordinates are on half-millimetre lines so that a
pass at a whole lateral distance lands on a **row of cell centres** and the arithmetic is an equality
rather than a bound:

| Fixture | Claimed | Removed | `uncut` | `gouge` | Deepest |
| --- | --- | --- | --- | --- | --- |
| straight rail, 5 passes at `1, 2.5, 4, 2.5, 1` mm | `660.000` | `660.000` | **`0.0`** | **`0.0`** | −4 |
| … the same, 1 mm shallow at every pass | `660.000` | `360.000` | `300.000` (300 cells) | **`0.0`** | −3 |
| … the same, 1 mm deep at every pass | `660.000` | `960.000` | `0.0` | **`300.000`** (300 cells) | −5 |
| closed frame, 2 passes at `-2` mm | `496.000` | `496.000` | **`0.0`** | **`0.0`** | −2 |
| … the frame, 1 mm shallow at both passes | `496.000` | `248.000` | `248.000` (248 cells) | **`0.0`** | −1 |
| … the frame, 1 mm deep at both passes | `496.000` | `744.000` | `0.0` | **`248.000`** (248 cells) | −3 |
| V-bit cone (90°), one pass at `-4` mm | `1027.127` | `1027.127` | **`0.0`** | **`0.0`** | −3.75 |
| … the cone, 1 mm deeper | `1027.127` | `1630.974` | `0.0` | **`603.671`** (676 cells) | −4.75 |
| the frame **whose seam is a corner**, correct program | `496.000` (248 cells) | `496.000` | **`0.0`** | **`0.0`** | −2 |

The straight rail's numbers are exact for a reason worth keeping: the rail is at `y = 50.5`, so the pass
at lateral `L` lies on the centre line of row `50 − L`, and the profile `-1` at lateral 0, `-4` at
lateral 4 and `-1` at lateral 8 with a 2 mm stepover gives the passes at laterals `0, 2, 4, 6, 8` and the
depths `1, 2.5, 4, 2.5, 1` — all exact in binary, because each is a midpoint of whole numbers. 60 columns
× 11 mm of depth is `660 mm³`, and neither sign can hide in a tolerance. The frame's `496` is the same
kind of arithmetic: the pass on the rail claims the 120 cell centres of a 40 × 20 mm rectangle's
perimeter and the pass one lateral out claims 128 (the four corner diagonals add one cell each), so 248
cells at 2 mm.

The V-bit rows are the cone's own field: a 90° bit at 4 mm reaches `4 − d` at a cell `d` millimetres from
the pass, so the swept solid is the triangle's prism (`2 × ½ × 4 × 4 × 60 = 960 mm³`) **plus** the two
half-disc caps a cone swept along a *segment* leaves at its ends — measured `1027.127 mm³`, and equal to
the claim to the last bit. The two zeros in that row are the point of it: a claim that assumed a
cylinder's flat band would report a correct bevel as `uncut`, and a program that reached one cell
further out as `gouge`.

End to end through the binary, three reports say the same thing on the CLI's own surface:

- a correct straight-rail moulding prints the `300.000 mm³` report above and exits `0`
  (`simulate_reports_a_correct_moulding_with_no_gouge_and_no_uncut_material`);
- a frame authoring 2 mm out of a **1 mm stock** leaves the deepest millimetre of every claimed cell
  standing — `uncut: 248.000 mm3 over 248 cell(s)`, exit **6**, the file untouched
  (`a_moulding_deeper_than_the_stock_reports_uncut_and_exits_six`);
- **two rails** in one toolpath cut and claim both — `removed: 600.000 mm3`, `gouge: 0.000`,
  `uncut: 0.000`, exit `0` (`a_moulding_over_two_rails_cuts_and_claims_both_of_them`), which is the
  multi-rail case proven end to end rather than assumed;
- a **corner seam** through the binary — a 40 x 20 mm frame imported from SVG, whose start vertex the
  importer normalises to a corner — prints `gouge: 0.000 mm3 over 0 cell(s)` and exits `0` at 0.25, 0.5
  and 1 mm resolutions with a 2 mm bit, where the same document reported `gouge: 3.000 mm3 over 24
  cell(s)` at 0.25 mm, `3.000` over 6 at 0.5 mm and `2.000` over 1 at 1 mm and exited **6** before the
  walk was shared. The program is byte-identical either way: 21 moves, `removed` unchanged at every
  resolution.

**The gouge sign cannot be reached through the CLI by a document that validates**, and that is the
argument above rather than a gap in the tests: the claim's footprint contains the program's, both read
the same authored cross-section and the same tool, so no valid document emits a program that cuts
deeper than its own claim. It is proved where it *can* be produced — a program written 1 mm too deep at
every pass, in the oracle. The `uncut` sign needs no such help, which is why the exit-6 test states it as
a defect of the document.

## What it refuses

A moulding has three inputs — the rail, the cross-section and the stepover — and no depth, no side, no
pass law, no allowance, no tab and no tolerance. It refuses by name everything whose meaning it does not
have (S16's precedent from M6):

| Refusal | Raised by | Message |
| --- | --- | --- |
| `MouldingProfileTooShort` | the strategy, and the claim | `a moulding profile needs at least two points, got 1` |
| `MouldingProfileLateral` (the strategy's) / `MouldingProfileLateralOrder` (the kernel's) | the loader, the strategy and the claim | `moulding profile point 1 at lateral 2 mm must be greater than the previous 2 mm: a cross-section is one depth per lateral distance` — one sentence, three boundaries |
| `InvalidStepover` (the strategy's) / `MouldingStepoverNotPositive` (the kernel's) | the loader and the strategy restate it; the claim keeps the kernel's words | `stepover 0 mm must be greater than zero` from the strategy, `a moulding stepover must be a positive number of millimetres, got 0 mm` from the kernel |
| `MouldingProfilePointNotFinite` | the kernel, surfaced at all three boundaries | `a moulding profile point must use finite millimetres, got lateral NaN mm and depth -1 mm` |
| `InvalidToolDiameter` | the strategy | `tool diameter must be greater than zero` — a tool with no diameter is not a tool |
| `MouldingGeometry` | the strategy, and the claim as `TargetError::Geometry` | `the moulding's geometry could not be built: …`, with the kernel's own sentence after it — `a moulding rail needs a length to sweep along, got 0 mm` for a rail with no length |
| no rail at all | the claim, before it authorises anything | `it names no rail, and a moulding sweeps a cross-section along one` — authorising nothing would report every cell the program cuts as gouge |
| a named entity that is not a contour | the claim | `entity "gone" is not a contour in this document` |
| 22 flags its law does not have | `cam-cli toolpath` | one `--kind moulding takes no <flag>: …` per flag, exit `2`, the document byte-identical |

The 22 refused flags are `--cut-depth`, `--start-depth`, `--pass-depth`, `--allowance`, `--side`,
`--tab`, `--lead-in`, `--lead-out`, `--ramp`, `--spacing`, `--angle`, `--pitch`, `--thread-direction`,
`--chamfer-width`, `--inlay-allowance`, `--strategy`, `--tolerance`, `--clear`, `--peck-depth`,
`--retract`, `--retract-amount` and `--bank`. Each names itself and the reason it has no meaning here
rather than being accepted and ignored — a `--cut-depth 5` nobody applied still looks applied. The
**stepover** falls back to the tool library's `stepover` exactly the way a pocket's does, and its absence
is an input error naming the flag rather than a defaulted spacing.

## Known limits, each a decision

- **No tolerance flag, by design.** The rail's walk chords an arc at `DEFAULT_CHORD_TOLERANCE`
  (`0.001` mm), a constant `cam-geom` declares. `--tolerance` is refused by name: a moulding's error is
  the walk's chord error and nothing else, and a flag that moved it would be a flag about the file's
  length, not about the cut.
- **That chord constant is shared and unpinned.** `cam-geom`, `cam-toolpath` and `cam-sim` all read
  `DEFAULT_CHORD_TOLERANCE`, which is what makes the claim's polyline the program's — and **no test pins
  its value**. Changing it moves the programs of every curved rail and moves nothing in a straight one.
  A silent coupling, recorded rather than papered over.
- **A closed rail whose start vertex is a corner is exact, and that is the walk's law rather than a
  special case** (closed after M8's delivery; measured before as `2.000 mm³` over 1 cell on the 40 × 20
  mm frame). A pass's polyline is [`Rail::offset_walk`] — the samples offset by the pass's lateral plus
  the closing edge a corner seam needs, and nothing when the seam is mid-edge — and the strategy and the
  claim both read it, so the program's corner diagonal is authorised like every other edge it cuts. The
  trigger, the drawing's own start vertex, stayed the reason it mattered: a DXF may start a frame
  anywhere, and the shell starts it wherever the user did.
- **The stepover certifies coverage of the span, not the removal of it.** The law guarantees every
  lateral position is within half a stepover of a pass; the material a pass actually removes is the
  **bit's** footprint, so a bit narrower than the stepover leaves ridges between passes. On the frame's
  flat 2 mm profile with a 0.5 mm bit and a 2 mm stepover, the 1 mm ridge between the two passes stands —
  that is the authored stepover's own gap, not a lost claim, and the claim and the program agree about it
  cell for cell.
- **A document that names several rails is several sweeps in one toolpath.** The rail rides on
  `entity_ids` (T1's schema decision), and both the program and the claim walk every named entity in
  order; two rails are two mouldings the way two selected vectors are two flutes. Covered end to end by
  `a_moulding_over_two_rails_cuts_and_claims_both_of_them` on two rails 20 mm apart; rails whose bands
  overlapped would be an overlap of claims (deepest wins) and are not separately measured.
- **No shell drafting form** (S7, assumption carried from M4/M7): the operation is authored through
  `cam-cli toolpath` like every other strategy, and the shell's strategy list still says so. If the owner
  orders a form, S7 is rewritten and a task is added rather than the assumption silently absorbed.
- **The CLI still ignores a flag its kind does not use, milestone-wide** (D-M6-11, still open). Closing
  it changes the contract for callers that pass a superset today; `moulding` refuses the 22 flags its own
  law forbids, which keeps its contract honest where M8 touched it.
- **No 3D machining, laser, projection or rotary wrapping** (scope D2, carried from M7). A moulding is a
  2.5D operation: the rail is XY and the profile is (lateral, Z).

## Where the record lives

| Question | Where |
| --- | --- |
| What M8 had to be, and every decision behind it | `odd/tasks/019-m8-moulding.md` — S1 … S7, T1 … T7, L1 … L12 |
| What the `moulding` kind stores | [format.md](format.md#document-c2d), one row per `kind` |
| What the simulation authorises | [m5-simulation.md](m5-simulation.md#the-target-surface) |
| How the depth-variant strategies behave | [m6-toolpaths.md](m6-toolpaths.md) |
| How a V-carve's depth field behaves | [m7-vcarve.md](m7-vcarve.md) |
| How a profile, a pocket and a drill behave | [m4-toolpaths.md](m4-toolpaths.md) |
| What M8 delivered, and what it cost | [odd/roadmap/index.md](../../odd/roadmap/index.md) §4.11 |
| The project-level summary | [README.md](../../README.md) |
