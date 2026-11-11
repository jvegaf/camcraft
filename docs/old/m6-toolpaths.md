# Depth-variant strategies (M6)

M4 shipped three strategies whose proof was a golden program and a green suite. M5 added the stock, so a
strategy could be asked whether the part comes out right. M6 spends that: six strategies whose value is
a **Z law** rather than a new pattern — a chamfer's derived depth, an engrave's one pass, a flute's ramp,
a texture's lattice, an inlay's pair of clearings, a thread's helix. Five of them are new laws over
machinery M4 already has (`pass_depths`, `Region::offset`, `Region::raster_spans`, `walk_contour`) and
one of them is not: a thread descends around a circle, so nothing the move vocabulary could express
would carry it. All six are authored the same way, through `cam-cli toolpath`, and all six are proved
the same way, by the simulation M5 built.

```console
$ cam-cli toolpath part.c2d --kind chamfer --entities E1 --tool 3 --chamfer-width 3 \
    --library tools.sqlite
added TP1 chamfer, tool T3, 1 entity(ies)
$ cam-cli toolpath part.c2d --kind engrave --entities E3 --tool 3 --cut-depth 2 --library tools.sqlite
added TP2 engrave, tool T3, 1 entity(ies)
$ cam-cli toolpath part.c2d --kind fluting --entities E3 --tool 1 --cut-depth 1.5 --ramp 10 \
    --library tools.sqlite
added TP3 fluting, tool T1, 1 entity(ies)
$ cam-cli toolpath part.c2d --kind texture --entities E1 --tool 1 --cut-depth 0.8 --spacing 2 \
    --angle 45 --library tools.sqlite
added TP4 texture (2 mm at 45 degrees), tool T1, 1 entity(ies)
$ cam-cli toolpath part.c2d --kind inlay --entities E2 --tool 1 --cut-depth 3 \
    --inlay-allowance 0.5 --stepover 2 --library tools.sqlite
added TP5 inlay female, tool T1, 1 entity(ies)
added TP6 inlay male, tool T1, 1 island(s)
$ cam-cli toolpath part.c2d --kind thread --entities E2 --tool 2 --cut-depth 3 --pitch 1.5 \
    --library tools.sqlite
added TP7 thread (1.5 mm pitch, internal), tool T2, 1 entity(ies)
$ cam-cli simulate part.c2d --library tools.sqlite
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 7 toolpath(s)
program: 542 move(s), 13791.440 mm cutting length
removed: 60944.257 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 12.000 mm3 over 4 cell(s)
crash: none
```

The document above holds one 100 x 80 mm square, a 20 mm hole and an open line, and a library with a
6 mm end mill, a 3 mm end mill and a 60° V-bit. Six commands appended seven toolpaths — an inlay is a
pair — and the report is what the shipped code prints: the 12 mm³ of uncut is inside M5's published
coverage bound and is the pocket's own residue under the inlay's male half (see the inlay section).

| Piece | Where |
| --- | --- |
| The six parameter records, `ChamferParams::depth` and `TextureParams::grooves` | `cam-model` |
| The six strategies | `cam-toolpath::{chamfer, engrave, fluting, texture, inlay, thread}` |
| The tool's shape on a segment, `ToolShape::{Cylinder, VBit}` | `cam-move`, `cam-sim::sweep` |
| The helical move and its four post block kinds | `cam-move::Move::Helix`, `cam-post` |
| The authorised depth for each strategy | `cam-sim::target` (`claim_cone_region`, `claim_along_path`, `claim_flute`, `claim_helix`, `claim_clearing`) |
| `--kind chamfer\|engrave\|fluting\|texture\|inlay\|thread` | `cam-cli` |
| The contract rows for each `kind` | [format.md](format.md#document-c2d) |

The decisions are D-M6-1 … D-M6-14 and the log is L1 … L17 in the
[feature document](../../odd/tasks/008-m6-depth-variants.md); the milestone's evidence is §4.9 of the
[roadmap](../../odd/roadmap/index.md), and the project-level summary is in the [README](../../README.md).

## The six strategies, one contract

`cam-cli toolpath` accepts six more `--kind` values and each appends one `Operation` variant to the
document — `inlay` appends two. The schema stays `c2d/document@2`: the variants are additive under the
existing `kind` tag, an unknown `kind` is still a hard load error, and every document written before M6
still loads and re-saves byte-identically. That is not decoration — it is the property the closing
differential measured against `8a953ba` (see [Determinism](#determinism-and-the-old-bytes)).

| Kind | In the document | Passes | The Z law | Cuts from |
| --- | --- | --- | --- | --- |
| `chamfer` | `chamfer_width`, `included_angle` (radians), `start_depth` | one | `start_depth + width / tan(angle/2)`, derived and never stored | the vector, tip **on** it |
| `engrave` | `start_depth`, `cut_depth` | one | `cut_depth`, authored | the vector, centre **on** it |
| `fluting` | `start_depth`, `cut_depth`, `ramp` (default 0) | one | ramps 0 → depth over the first `ramp` mm, plateau, depth → 0 over the last | the vector, centre **on** it |
| `texture` | `start_depth`, `cut_depth`, `spacing` (default 3), `angle` (radians, default 0) | one per groove | `cut_depth` | each groove of a derived pattern, centre on it |
| `inlay` | `allowance`, `stepover`, `pass_depth`, `start_depth`, `cut_depth` | the pocket's | the pocket's depth chunking | **two** toolpaths: the grown region, then the job rectangle around the region |
| `thread` | `pitch`, `direction`, `start_depth`, `cut_depth` | one descent | `pitch` per revolution to `cut_depth` | a helix around the selected circle |

Every one of them inherits M4's conservative linking: between grooves, spans, rings and passes the tool
retracts to `job.safe_z` and rapids (D-M4-6, S14). Staying down needs a gouge-free proof, and a texture
that linked along its own surface would drag the bit across the material the pattern exists to leave
standing.

There is no toolpath form in the window for any of them (D-M4-8, S13): `cam-cli toolpath` is the
authoring surface, and the shell gained nothing.

## Chamfer

A chamfer is a V-bit bevel, and its depth is a **derivation** rather than a number the user types.

```text
cam-cli toolpath <document> --kind chamfer --entities <ids> --tool <n> --chamfer-width <mm>
                 [--start-depth <mm>] [--library <path>] [--out <path>] [--id <id>] [--name <name>]
```

### The Z law

The tip travels **on** the vector and the bevel is symmetric about it, so there is no side to choose and
no lateral offset to apply. The flank is a cone, so the width and the bit's full included angle fix how
deep the tip has to go for the flank to meet the surface exactly `chamfer_width` from the vector:

```text
depth = start_depth + chamfer_width / tan(included_angle / 2)
```

| Bit | `chamfer_width` | Derived depth | What it looks like |
| --- | --- | --- | --- |
| 90° | 3 mm | `3 / tan(45°) = 3.000` | a 3 mm chamfer at 45° — the case a user checks by eye |
| 60° | 6 mm | `6 / tan(30°) = 10.392` | the same 6 mm of surface width, 10.392 mm deep |
| 60° | 3 mm | `3 / tan(30°) = 5.196` | the worked fixture below |

`depth()` lives once, in `cam-model`, and both `cam-toolpath` and `cam-sim` call it: a stored derived
depth and a recomputed one would eventually disagree in a hand-edited document, and a bevel that a
program cuts and a simulation authorises at two different depths is exactly the disagreement the oracle
exists to catch.

One pass, whatever the depth. A second pass at a deeper Z would cut a second bevel into the first, so
the profile's pass law, tabs, leads and side machinery are not reused at all — the walk is `walk_contour`
at one depth, arcs kept as arcs.

### What it refuses, and why

| Refusal | Raised by | Message |
| --- | --- | --- |
| `ChamferWidthNotPositive` | the document | `toolpath "TP1" chamfer width must be a positive number of millimetres, got 0 mm` |
| `ChamferAngleOutOfRange` | the document | `toolpath "TP1" chamfer included angle must be between 0 and 180 degrees, got 0 degrees` |
| `InvalidIncludedAngle` | `cam-toolpath::chamfer` | `chamfer included angle 0 degrees must be strictly between 0 and 180` |
| no angle in the library | `cam-cli toolpath` | `tool T3 has no included angle in the library, so a chamfer's depth cannot be derived: a chamfer needs a V-bit` |
| a flat tip | `cam-cli toolpath` | `tool T3 has a 2 mm flat tip, so a chamfer's bevel would reach the surface wider than --chamfer-width 3 mm: a chamfer needs a sharp V-bit` |
| `--side`, `--cut-depth`, `--pass-depth` | `cam-cli toolpath` | `--kind chamfer takes no --side: the tip runs on the vector and its depth is derived from --chamfer-width and the tool's included angle` |
| `ChamferNeedsVBit`, `ChamferNeedsSharpBit` | `cam-sim::target` | a segment whose tool is a cylinder or a flat-tipped bit cannot be the bevel the document describes |

The angle is checked twice on purpose. The document sees it before a toolpath exists, and the strategy
checks it again because the tangent of zero is zero: a caller that reaches `chamfer()` without passing
through a validated document must not receive an infinite Z (D-M6-10). A **flat tip** is refused rather
than absorbed: with a flat of diameter `f` the bevel reaches the surface `f/2` wider than
`chamfer_width`, so honouring the number the user typed would need a derivation that reads the tool,
which `ChamferParams` deliberately does not do. The gate exists at all three boundaries that can see the
flat, because they see different things (L11): the library **before** authoring, the library **after**
the document exists — the case that matters, a document authored against a sharp bit and emitted
against a flat one — and the `ToolShape` the segment carries.

### What the simulation authorises

The band is the contour offset out and in by the cone's own reach at its derived depth, and inside that
band the authorised depth is the cone's floor, cell by cell: `depth − rise(distance)`, capped at `depth`,
zero past the reach. For a sharp bit the reach *is* the chamfer width, so the band and the bevel are the
same number. The claim is graded rather than flat, which is why a square chamfer does not show the
corner over-authorisation a cylindrical profile does: on the worked fixture (100 x 80 mm square, 3 mm
bevel, 60° bit, 1 mm cells) `simulate` reports `removed 5595.074 mm³`, `gouge 0.000`, `uncut 0.000` —
the cone's grade accounts for the mitered corner out to the reach and claims nothing past it.

The oracle asserts the volume of **both** flanks: on a 40 mm radius ring, a 6 mm bevel from a 60° bit
removes `L · w · d` where one flank alone would be `L · w · d / 2`, against the analytic `2πR · w · d`
within the coverage bound `4πR · cell/2 · d`, and a clean chamfer deviates by nothing at the tolerance
the graded cut is judged at, `rise(cell/4)`. A second oracle covers the one-sided reading (A9 as amended
in L9): a closed rectangle whose cut edge lies on the stock's low edge and whose other three edges lie
outside the sheet, so the grid clips exactly the flank that would fall on air.

## Quick engrave

```text
cam-cli toolpath <document> --kind engrave --entities <ids> --tool <n> --cut-depth <mm>
                 [--start-depth <mm>] [--library <path>] [--out <path>] [--id <id>] [--name <name>]
```

One pass at one authored depth with the tool's centre on the vector (D-M6-4). It is the profile's `On`
side with the pass law removed, and that removal is the point: a second pass at a deeper Z would cut a
second groove into the first. There is no side, no stepdown, no lead and no tab, and the CLI refuses each
of those flags by name rather than accepting and ignoring it:

```text
--kind engrave takes no --pass-depth: an engrave is one pass at a constant depth, with the tool centre
on the vector, no stepdown, no tabs and no lead
```

It cuts **open** vectors too, as fluting does and nothing else here does: engraving a single line is
the strategy's whole point, and it stops at the vector's last vertex. `cut_depth` must be greater than `start_depth`
(`cut depth 2 mm must be greater than start depth 2 mm`), so the refusal is reachable from the CLI —
which is why `--start-depth` is accepted even though A3 names only `--cut-depth` (L12).

The claim is the tool's footprint band **along** the vector, `cut_depth − rise(distance)` clamped at
zero, plus the reach test a cone self-clamps and a cylinder does not (L12's first finding). A V-bit
engrave therefore removes the cone it sweeps — `L · d² · tan(θ/2)`, `3325.6 mm³` for a 6 mm pass over the
oracle's 160 mm vector — and an end mill removes a uniform band `2r · d`. A flat tip is *not* refused
here: an engrave's depth is authored rather than derived, so a flat widens the groove by `f/2` and
`rise_at_radius` accounts for it in both the sweep and the claim. D-M6-10 stays chamfer-only.

## Fluting

```text
cam-cli toolpath <document> --kind fluting --entities <ids> --tool <n> --cut-depth <mm>
                 [--ramp <mm>] [--start-depth <mm>] [--library <path>] [--out <path>] ...
```

A flute is one groove per vector (D-M6-5), open or closed, with the tool's centre on it. What it has
instead of a pass law is a **ramp**, and the ramp is a length rather than a depth: with `ramp > 0` the
tool enters over the first `ramp` millimetres of the vector and leaves over the last, so a round-nose
bit never plunges at full depth into the end of a visible flute. `ramp = 0` is the plain groove — a
plunge in place at the start and a retract in place at the end.

```text
      surface ─┐                              ┌─ surface
               ╲                            ╱
                ╲__________________________╱
      −depth     ╲    plateau at −depth   ╱
                 0 … ramp            L−ramp … L
```

The entry and the exit are **real moves that follow the vector**, because a real controller interpolates
them and so does `cam-sim` (D-M6-12). That has one consequence worth stating: a ramp along a contour that
contains an arc is emitted as one **helical** move per arc (`Move::Helix`), never as a straight chord —
a chorded ramp is `ramp² / (8r)` inside the vector, half a millimetre for a 10 mm ramp on a 25 mm circle,
and T4 refused that case by name until T7's helix existed. `ramp = 0` on the same contour is untouched,
because a constant-depth pass along an arc is exactly what `Move::Arc` is for.

| Refusal | Message |
| --- | --- |
| the document, negative ramp | `toolpath "TP1" fluting ramp must not be negative, got -1 mm` |
| `InvalidRamp` | `fluting ramp -1 mm must not be negative` |
| `RampTooLong` | `fluting ramp 51 mm is longer than half of the vector's 100 mm length` |
| `InvalidDepthRange` | `cut depth 6 mm must be greater than start depth 6 mm` |

A ramp longer than half the vector is an input error naming both numbers rather than a silently
shortened ramp, which would cut both ends at a depth nobody authored; a closed contour measures the ramp
against its perimeter, because that is the groove it actually cuts.

The claim is a depth along the **path**, not at a distance: `min(depth, depth·s/ramp, depth·(L − s)/ramp)`
less the tool's rise, taken as the maximum anywhere the tool could have removed the cell. The envelope is
not decoration — a tip-only claim reports a perfectly clean flute as `gouge 149.2 mm³ over 120 cells`,
because a 6 mm end mill 3 mm further up the ramp is deeper there and its flat bottom really does remove
the cell (L13, L14). The oracle's arithmetic is the closed form the ramp gives:

```text
∫ depth ds = d·ramp/2 + d·(L − 2·ramp) + d·ramp/2 = d·(L − ramp)
removed = 2·R·d·(L − ramp) = 2 · 3 · 6 · 150 = 5400 mm³   (R = 3, d = 6, L = 160, ramp = 10)
```

Measured on that field: **5548.5 mm³**. The 148.5 mm³ between the two is mostly the *toes* — `π·R²·d =
169.6 mm³` of material a tool's footprint adds where each ramp meets the surface, because the bottom of a
6 mm end mill is still in the cut 3 mm past its tip — and the rest is the grid's account of the same
toes. Four samplings of one geometry say so: the rejected staircase measured 5549.2, the half-cell
sampling 5539.2, the first derived sampling 5545.5 and the shipped one 5548.5, all inside the published
bound of `2·L·cell/2·d = 960 mm³`. The documented closed form has not moved through any of it.

## Texture

```text
cam-cli toolpath <document> --kind texture --entities <ids> --tool <n> --cut-depth <mm>
                 [--spacing <mm>] [--angle <degrees>] [--start-depth <mm>] ...
```

Parallel grooves across a **closed** region, `spacing` millimetres apart at `angle` counter-clockwise from
the X axis (D-M6-6). **It does not clear.** The material between the grooves is meant to stand, so a
texture is a pattern and never a pocket with a wide stepover, the command's `--kind` help says so, and it
refuses every flag a clearing operation would take — `--stepover` included, because a texture's distance
is `--spacing` and accepting both would leave two numbers naming one thing.

The pattern is **derived, never stored**, in one place: `TextureParams::grooves` rotates the region by
`−angle`, asks the kernel's `Region::raster_spans` for each row of a lattice `spacing` apart in that
frame, and rotates the resulting spans back. Both `cam-toolpath::texture` and `cam-sim::target` call it,
so the program and the authorised grooves cannot be made to agree by construction (D-M6-2's argument for
a chamfer's depth, applied to something much easier to get subtly wrong twice). Arcs and islands come
free: the region's parity rule makes an appended boundary a hole, and the clipping is the kernel's own
row question rather than a second intersector.

The lattice's phase is the one thing the specification did not decide: the first groove sits **half a
spacing inside** the region's lower extent in the groove frame, so it starts off the region's own edge
instead of on it, where the kernel reports a tangency. The pattern is therefore centred only when the
extent is a whole number of spacings (L15).

| Refusal | Raised by | Message |
| --- | --- | --- |
| `TextureSpacingNotPositive` | the document | `toolpath "TP1" texture spacing must be a positive number of millimetres, got 0 mm` |
| `InvalidSpacing` | `cam-toolpath::texture` | `texture spacing 0 mm must be greater than zero` |
| `NoGroove` | `cam-toolpath::texture` | `texture spacing 500 mm at 0 degrees leaves no groove across the region: …` |
| no spacing anywhere | `cam-cli toolpath` | `a texture needs --spacing: the tool library has no stepover for that tool` |

The oracle: a 40 mm square at an 8 mm spacing is five rows at `y = 4, 12, 20, 28, 36`, `Σ length = 200 mm`,
and a 6 mm end mill at a 6 mm depth removes `2·R·d·ΣL = 7200 mm³` — measured **exactly** 7200, because
the grooves end on the sheet's edges and every band's edges fall on cell boundaries.

**The 45° case does not pass `simulate`'s zero-defect gate at 1 mm cells.** T5 recorded
`uncut 1.000 mm³ over 2 cell(s)` on its own fixture (L15); a 60 mm region at 30° with a 12 mm spacing
reproduces the same two-cell rind at `uncut 12.000 mm³ over 2 cell(s)`. That is not a defect in the
pattern: it is M5's published coverage bound, `perimeter × cell/2`, measured on a rind the grid does not
sample — the same one-sided residue every graded claim here carries. The 45° test therefore asserts the
emitted diagonals and the claim geometry directly, rather than widening a tolerance over a shape the
kernel describes exactly (which is the trade L14 rejected).

## The inlay pair

```text
cam-cli toolpath <document> --kind inlay --entities <ids> --tool <n> --cut-depth <mm>
                 [--inlay-allowance <mm>] [--stepover <mm>] [--pass-depth <mm>] [--clear] ...
added TP5 inlay female, tool T1, 1 entity(ies)
added TP6 inlay male, tool T1, 1 island(s)
```

One command, **two** toolpaths, in a fixed order, sharing one copy of five parameters (D-M6-7):

- the **female** clears the selected region **grown by the allowance** — the cavity — with the pocket's
  offset rings, which follow the wall a plug has to fit;
- the **male** clears the job rectangle with the region standing as an **island** — the plug — with the
  pocket's raster rows.

The allowance *is* the clearance: the cavity is `allowance` bigger than the plug on every side, so there
is one number and no second "gap" parameter. The female's growth is the pocket's own `allowance`
**negated**, because that parameter is how far the tool centre keeps back from the boundary; at
`allowance = 0` the female is the plain pocket of the region, cell for cell.

The male's boundary is `Job::outline()` — the one place the job rectangle is built, so the program and
the simulation cannot disagree about where the stock ends.

### How the two halves are told apart

A6 says both halves share one `InlayParams`, so the half cannot live in the parameters without adding a
field the specification does not have. It lives where the document already says such things (D-M6-13):
the **female** names the region as its `entity_ids` and stands nothing; the **male** has no boundary
entity at all — its boundary is the job rectangle, which is not an entity — and names the region as its
`island_ids`. A record that names both is refused rather than guessed at (`toolpath "TP5" is an inlay
male with entity_ids: …`), and the toolpath's display name is never consulted because it is a
user-editable string.

### Why the male cuts rows and not rings

The material around a region is a **frame**, and a ring lattice converges on the frame's medial axis and
stops. On a 40 mm region in a 100 mm stock with a 6 mm tool at a 2 mm stepover, the ring male leaves
**2300 mm²** of the frame's 8556 mm² never cut — the four diagonal corners where the plug would still be
attached to the stock — while the row male leaves 80 mm². At a 4 mm stepover the same comparison reads
2808 mm² of 8400 mm² against the rows' residue. The row lattice has no such blind spot: every row is
clipped to the frame by the kernel's own spans, so it reaches both walls.

| Refusal | Raised by | Message |
| --- | --- | --- |
| `InlayStepoverNotPositive` | the document | `toolpath "TP5" inlay stepover must be a positive number of millimetres, got 0 mm` |
| `InlayDepthRange` | the document | `toolpath "TP5" inlay cut depth 6 mm must be greater than its start depth 6 mm` |
| `InlayMaleTakesNoEntities` | the document | `toolpath "TP5" is an inlay male with entity_ids: …` |
| `InlayAllowanceCollapsed` | `cam-toolpath::inlay` | `inlay allowance -25 mm leaves no region to cut: …` |
| `InlayRegionOutsideJob` | `cam-toolpath::inlay` | `inlay region is not strictly inside the job's 100 x 100 mm rectangle: …` |
| `--id`, `--name`, `--side`, `--tab`, `--lead-*`, `--ramp`, `--spacing`, `--angle`, `--chamfer-width`, `--strategy` | `cam-cli toolpath` | `--kind inlay names its two toolpaths itself: …` and one refusal per flag |

A **negative** allowance is an interference fit, not an error: the cavity comes out smaller than the
plug. It is refused only when it shrinks the region away, and then by name with the kernel's reason.

### What the simulation authorises, and what stands

The female's claim is the region grown by the allowance; the male's is the job rectangle with the region
standing; a cell both claim keeps the deeper and never the sum (D-M5-2). On the oracle's fixture
(a 38 mm region at (21, 21) in a 100 mm stock, a 2 mm allowance, a 6 mm tool, a 2 mm stepover, 6 mm deep):

- the female's residue is the four cells a 6 mm disc cannot reach into the grown square's mitered
  corners: **`uncut 24.000 mm³ over 4 cell(s)`**, the pocket's own sliver;
- the male's residue is **`480.000 mm³ over 80 cells`** — the stock's four corner cells plus a rind of up
  to one stepover along the two walls of the region that run parallel to the rows, whose nearest row is
  at `wall − radius − 1` rather than `wall − radius`. That is the **pocket's published coverage bound**,
  pinned in the oracle with its arithmetic rather than absorbed into a tolerance (L16). What the pair
  needs is that the plug be *separated* from the stock, which the rows do everywhere except those corner
  cells, and that is the honest statement of its finish.

## Thread milling

```text
cam-cli toolpath <document> --kind thread --entities <ids> --tool <n> --cut-depth <mm> --pitch <mm>
                 [--thread-direction internal|external] [--start-depth <mm>] ...
```

One helical descent around each selected **circle**, one `pitch` per revolution, from the surface to
`-cut_depth`, then a retract (A7). The tool orbits at a fixed radius that the circle and the tool fix
between them:

| Direction | Orbit radius | What it cuts |
| --- | --- | --- |
| `internal` (default) | `(D − t)/2` | the hole's wall, from inside: the disc reaches `D/2` and stops `t` short of the axis |
| `external` | `(D + t)/2` | a boss's wall, from outside |

`D` is the selected circle's diameter and `t` the tool's, and neither is stored in the document: the
circle is the entity the toolpath names and the tool is the library's, so a document cannot disagree with
the geometry it was written against.

### The Z law, and the spiral it leaves

`cut_depth / pitch` revolutions, emitted as one **full** helical move per whole revolution and one move
carrying the fraction that reaches the depth — a full turn being the move whose destination is its own
start point, exactly as a full-circle arc is. The depth at unwrapped angle `Φ` from the orbit's `+x`
point is `pitch · Φ / 2π`, so the floor the descent leaves is a **spiral**, not a disc: the material
between two revolutions stands, and that material *is* the thread.

```text
$ cam-cli emit part.c2d --post post-helix.pp --library tools.sqlite
N5470M05
N5480T2M06
N5490G00
N5500G00X68.5000Y50.0000
N5510G01Z0.0000
N5520G03Z-1.5000I-8.5000J0.0000
N5530G03Z-3.0000I-8.5000J0.0000
N5540G00Z5.0000
```

That is a 3 mm thread at a 1.5 mm pitch on a 20 mm hole with a 3 mm tool: orbit `(20 − 3)/2 = 8.5` about
the circle's centre at `x = 60`, so the descent starts at `x = 68.5`; two revolutions, each one `G03`
carrying `[Z]`, landing on the surface at `Z0` and reaching `Z-3`. `Move::Helix` is a move kind of its own and the post has four
blocks for it (D-M6-8):

```text
$ cam-cli emit part.c2d --post post_mm.pp --library tools.sqlite
error: post-processor "…" defines no helical move block, so it cannot express a helical arc
(Move::Helix): a program that descends around a circle needs CW_HELIX_MOVE, CCW_HELIX_MOVE,
FIRST_CW_HELIX_MOVE or FIRST_CCW_HELIX_MOVE; refusing to emit an approximation
$ echo $?
4
```

The refusal is the whole point of the block kinds: an arc block's template carries `[X][Y][I][J]` and no
`[Z]`, so a helix emitted through one would cut a flat circle at the wrong depth. Absence is a refusal,
exit `4`, and nothing is written — M4's arc refusal, in the same words and for the same reason.

| Refusal | Raised by | Message |
| --- | --- | --- |
| `ThreadPitchNotPositive` | the document | `toolpath "TP7" thread pitch must be a positive number of millimetres, got 0 mm` |
| `ThreadDepthRange` | the document | `toolpath "TP7" thread cut depth 6 mm must be greater than its start depth 6 mm` |
| `InvalidPitch` | `cam-toolpath::thread` | `thread pitch 0 mm must be greater than zero` |
| `NotACircle` | `cam-toolpath::thread` | `entity "E1" is not a circle, so it has no centre to work from` |
| `ThreadToolTooWide` | `cam-toolpath::thread` | `tool diameter 14 mm does not fit a 12 mm hole: …` |
| 11 flags its law does not have | `cam-cli toolpath` | `--kind thread takes no --stepover: a thread is one helical descent around each circle, one --pitch per revolution, …` |

### The claim is the spiral, not the annulus

The annulus the tool sweeps is `D/2 − t … D/2` inside a hole and `D/2 … D/2 + t` around a boss, but
authorising the whole annulus at `cut_depth` is an **upper bound** short by up to one pitch: the last
revolution passes each point at that point's own angle. That first claim failed the acceptance criterion
— a 6 mm deep, 1.5 mm pitch thread reported `uncut 106.452 mm³ over 151 cells` — which is what a gate that
fails every real thread looks like (D-M6-14). The claim is now the floor itself, cell by cell, taking the
maximum over the tool's angular window `α(r)` rather than at the cell's own angle, because a cell is
reached from angles up to `α` later than its own and the later pass is the deeper one — the same one-sided
error `claim_flute` was amended for.

Four numbers state a thread on the oracle's fixture (12 mm hole, 4 mm tool, 6 mm deep, 1.5 mm pitch):

| Number | Value | What it is |
| --- | --- | --- |
| Analytic | `π·t·(D − t)·d = 603.186 mm³` | the full annulus at one depth — what a flat-bottomed cut would remove |
| Authorised | `542.288 mm³` | the spiral, deepened by the footprint's angular window and quantised onto 1 mm cells |
| Removed | `541.000 mm³` | the same spiral, sampled; the 1.288 between this and the claim is the sampling |
| Tip-only claim | `527.788 mm³` | what the law at the cell's own angle would authorise — less than the program removes, so it would report a correct thread as a gouge |

The gap to the analytic — 62.186 mm³ — is the **crest**, which is the thread, not a defect. Both the
coarse 1.5 mm fixture and a 0.05 mm pitch are clean on both signs, with no deviation: the claim is exact
for every pitch, which is what replaced the sub-tolerance fixture the upper-bound claim needed.

## The tool's shape (M6's first prerequisite)

The roadmap calls the six "profile/pocket variants with a different Z law — cheap once M4 and M5 exist".
Two facts found before the first write said otherwise, and they are the reason T1 and T7 exist (L3).

The first: **no tool geometry reached the toolpath layer.** `ToolSpec` carried a number, a diameter, a
feed rate, a plunge rate and a spindle speed and nothing else, while `included_angle`, `flat_diameter` and
`tip_radius` had been sitting unused in the tool database since migration 1. More to the point,
`cam-sim` swept a constant-radius cylinder per segment, so a cone had no carrier at all — and a chamfer
that was swept as a cylinder would report a **correct** bevel as a gouge, which is exactly the failure
D-M5-5 was amended to fix.

So `cam_move::ToolShape` replaces the bare `tool_diameter` on `Segment` (D-M6-3):

| Shape | Numbers | `radius_at(depth)` | `rise_at_radius(radius)` |
| --- | --- | --- | --- |
| `Cylinder` | `diameter` | `diameter/2`, whatever the depth | `0` |
| `VBit` | `included_angle`, `flat_diameter` | `flat/2 + depth·tan(angle/2)` | how far the cone's surface sits above its tip at that radius |

The two methods exist because a cone's footprint grows with depth, and because the first form of the law
was written as a **depth** rather than a rise: `(x − flat/2)/tan(α)` is how far the surface sits *above*
the tip, not how deep it cuts. The wrong reading produced a groove that widens with depth — the exact
inverse of a V-groove — and it had a test written to agree with it before it was caught (L5). The
methods' names are the fix: a name cannot be read as a depth again.

The emitter ignores the shape and its tests are unchanged. The library stores `included_angle` in
**degrees**; the document stores it in **radians**; the conversion happens once, in the library reader.

One property of a cone cut is worth stating because it is what makes the coverage bound safe: a V-bit's
sweep can be reported **shallow** by the sampling and never deep, since its sample is always at least as
far from the path as the path is. The sampling can therefore never hide a gouge.

## The helical move (M6's second prerequisite)

The second fact: `Move::Arc` is documented constant-Z and the post fixture's arc blocks carry `[X][Y][I][J]`
and no `[Z]`, so thread milling could not ride on the existing move or block. M6 adds `Move::Helix` and
the four helical block kinds, and the emitter requires a block for every move kind it meets (D-M6-8).
Additive by construction: no existing block kind changed meaning, no document stops loading, and no post
that works today stops working.

`cam-sim` sweeps a helix with the arc code, because it *is* an arc that descends: the footprint is the
arc's, and only the Z differs, interpolated by angle. That is what makes a thread's floor the spiral the
descent leaves instead of a step, and a full turn's destination is its own start point, so one revolution
is one move to the sweep, to the preview and to `cut_length` alike.

## Determinism and the old bytes

Every new subcommand run twice prints and emits identical bytes: two runs of an inlay pair, of a thread
and of every other new kind are asserted byte-for-byte, from the document on disk through the program.
No map iteration reaches an output path: the pattern is a `Vec`, the lattice is walked in one direction,
and the ids a command invents are the first free `TPn`.

The pre-existing command surface was then run against `master`'s tip `8a953ba` in an identical
sandbox — same document, same library, same post, same geometry, relative paths, 29 invocations covering
`validate`, `inspect`, `toolpath` (profile, pocket offset, pocket raster with an island, drilling with a
bank), `emit`, `check`, `postp validate`, `tools list`, `tools add`, `import` (both forms, one writing a
document), `simulate` at three resolutions, and the error paths. Exit code, stdout, stderr and every file
produced (documents as their JSON payloads, programs byte for byte, the imported document, the library)
were compared. **Two differ, and both are supposed to:**

| Case | Before | After |
| --- | --- | --- |
| `--help` | lists `profile, pocket or drilling` and their flags | lists the nine kinds, `--spacing`, `--angle`, `--inlay-allowance`, `--chamfer-width`, `--ramp`, `--pitch`, `--thread-direction` |
| unknown `--kind` | `unknown toolpath kind "bogus"; expected profile, pocket or drilling` | `…; expected profile, pocket, drilling, chamfer, engrave, fluting, texture, inlay or thread` |

Everything else — including the `--islands` refusal, the `simulate` report of a document holding four M4
toolpaths, and the `emit` program that document produces — is identical, byte for byte. The new commands
themselves are, of course, new.

One further difference is deliberate and is **not** in that sandbox, because it takes a document the CLI
cannot write: a document that carries `island_ids` on a profile toolpath (the CLI refuses `--islands` for
any kind but a pocket before the document is ever validated). `cam-cli validate` on such a document says
`toolpath "TP1" takes islands only for a pocket` before M6 and `toolpath "TP1" takes islands only for a
pocket or an inlay` after it — the same exit code, the sentence reworded because an inlay male takes
islands and the old one had stopped being true (see [region-booleans.md](region-booleans.md)).

## Known defects

### The origin corner disagrees between the field and the program

**Severity: high. Present since M5, for every strategy, and not a defect any M6 strategy introduced.**
Found while T7 was proving the helix (L17), recorded there and here rather than fixed, because a fix
changes the coordinate contract of the whole simulation and needs its own slice.

`cam_sim::grid::grid_for` builds the field at origin `(0, 0)` spanning `job.width × job.height`, i.e. in
**document** coordinates, and `cam_sim::target` derives its authorisation from the document — also
document coordinates. The program, however, is emitted in **machine** coordinates:
`cam_toolpath::machine_point` maps document coordinates onto the machine's for `top_left` (`y → height − y`)
and `center` (`x → x − width/2`, `y → y − height/2`), and `cam_sim::sweep` cuts the field with the move
coordinates as they are. For any origin corner other than `bottom_left`, the field and the program
therefore disagree about **where** everything is.

Measured through `cam-cli simulate`, on a single pocket over an 80 x 30 mm region at `y = 60 … 90` in a
200 x 100 mm sheet, 1 mm cells:

| Origin | Removed | Gouge | Uncut |
| --- | --- | --- | --- |
| `bottom_left` | 14376.000 mm³ | 0.000 | 24.000 mm³ over 4 cells |
| `top_left` | 14376.000 mm³ | **14376.000 mm³ over 2396 cells** | **14400.000 mm³ over 2400 cells** |

The same program, the same region: at `top_left` the entire removal is reported as gouge and the entire
region as uncut, because the cut is a mirror image of the authorisation about `y = 50`. With
`origin: center` the removal collapses outright — on a 100 x 80 mm square pocket, `removed` falls from
47976.000 to 2394.000 mm³ and the report turns into `gouge 2394.000` and `uncut 48000.000`.

Two things follow, and both matter for what this milestone can claim. The **emitted G-code is not
affected**: it is a function of the document and the origin, and it is what the machine runs. What is
affected is the **simulation report** and the **3D preview**, which draws the same field, so at a
`top_left` or `center` origin a perfectly correct program is reported as gouging and a picture of the cut
lands in the wrong place. And a `bottom_left`-symmetric fixture **hides** it: the same 100 x 80 mm square
is a mirror of itself about `y = 50`, so its numbers come out identical at `bottom_left` and `top_left`,
which is why nothing caught it — every fixture and the default sheet are bottom-left.

A fix touches, in one change: `cam_sim::grid::grid_for` (the field's origin is a job-frame question, and
a grid that is not at `(0, 0)` needs `heightfield`, `mesh` and the sweep's indexing to carry it), the
point where the sweep maps a move into a cell (it must invert the same mapping `machine_point` applies,
which is the only place the two frames meet), and every fixture that assumes bottom-left. The minimal
honest form is one `machine_point`-style transform applied exactly once, at the field's boundary, with a
test per corner asserting that a correct program reports `Deviation::default()`; anything that
"fixes" it by moving the target instead moves the same disagreement into `cam-toolpath`.

## Known limits, each a decision

- **The mitered-corner over-authorisation still applies, and M6 inherits it** (D-M5-11). A profile's
  band comes from `offset_contour`, whose joins are mitered (`miter_limit: 4.0`), while the removal is a
  union of discs along that path. At a convex corner the mitered band claims material the disc never
  touches, at most `4(r² − πr²/4)` per right-angled corner — about `7.73 mm² × depth` at a 6 mm tool — so
  a perfect 100 mm square profile reports `24 mm³ over 4 cells` at a 1 mm cell, converging to that limit
  as the grid refines. The circular contour is the case where the statement is exact, which is why the
  chamfer's oracle is a circle. M6's cone-graded bands are built from the same mitered offset but graded
  by the cone's own law, so a corner cell is claimed only as deep as the cone could reach it: a square
  chamfer measures `gouge 0.000` and `uncut 0.000` on the worked fixture, and the V-bit profile test
  asserts zero gouge at **zero** tolerance on a square path. What M6 does not add is a round-join offset;
  a tolerance tuned to corners is exactly the resolution knob D-M5-3 refuses.
- **A texture leaves a rind the grid does not sample.** At 45° and 1 mm cells, `simulate` reports
  `uncut` on two cells (L15), because the groove's boundary falls between cell centres. The bound is
  M5's `perimeter × cell/2 × depth`, and it is one-sided: refining the resolution shrinks it, and no
  defect hides behind it. The pattern itself is asserted directly instead.
- **The inlay male leaves the pocket's residue** — 480 mm³ over 80 cells on the oracle fixture. It is
  the published coverage bound of the raster pocket, not an inlay-specific defect, and the pair's
  guarantee is separation, not a mirror finish.
- **A thread's floor is a helix, and the claim is a continuous maximum sampled onto cells.** A correct
  thread therefore carries the same one-sided residue every graded claim here has, a fraction of a
  cell's eighth. A crest is not a defect: it is the thread.
- **The CLI ignores a flag its kind does not use, and M6 does not close that** (D-M6-11). Every option is
  parsed and only the strategies that consume one validate it, so `--allowance` still reaches a drilling
  toolpath and is dropped. Chamfer, engrave, fluting, texture, inlay and thread each refuse by name the
  flags their own law forbids, which keeps their contract honest where M6 touched it; closing it for
  every kind changes the CLI's contract for callers that pass a superset today, and is its own slice.
  `--spacing` and `--angle` on a kind that does not use them now parse and are ignored rather than
  refused as unknown, which is a real change for those two names.
- **A ramped flute on an arc is a strategy-level rule, not a document one** (L14). `Document::validate`
  refuses a negative ramp, a non-positive spacing, a degenerate inlay stepover, a thread with no pitch
  and a chamfer with no angle or an impossible one; whether a ramp *fits* its vector needs the geometry's
  length, so it is refused where the vector is walked.
- **A cone's interior candidate is exact while the nearest point is a perpendicular foot** (L14). It is
  measured clean on every fixture; a ternary search would remove the assumption at a real per-cell cost.
- **No 3D and no laser** (D2, S12), no `Moulding`/`Swept Profile` (M8), no `V-Carve` (M7), and no toolpath
  form in the window for any of the six (S13). `BevelCarving` is in S4's vocabulary but not in
  the roadmap's M6 row: recorded as named-but-unscheduled rather than silently dropped.

## `cam-cli toolpath`, for the six kinds

```text
cam-cli toolpath <document> --kind chamfer|engrave|fluting|texture|inlay|thread
                 --entities <ids> --tool <number>
                 [--out <path>] [--library <path>] [--id <id>] [--name <name>]
                 [--start-depth <mm>] [--cut-depth <mm>]
                 [--chamfer-width <mm>] [--ramp <mm>]
                 [--spacing <mm>] [--angle <degrees>]
                 [--inlay-allowance <mm>] [--stepover <mm>] [--pass-depth <mm>] [--clear]
                 [--pitch <mm>] [--thread-direction internal|external]
```

| Option | Applies to | Default, and where it comes from |
| --- | --- | --- |
| `--kind` | — | Required: one of the nine |
| `--entities` | — | Required: each entity is its own vector, region, hole or thread |
| `--tool` | — | Required: the tool number in the library |
| `--library` | — | The document's `tools_ref`, resolved relative to the document |
| `--id`, `--name` | all but inlay | `TP<n>` (first free) and `<Kind> <n>`; an inlay names its pair itself |
| `--chamfer-width` | chamfer | Required; the depth is derived from it and the bit's angle |
| `--cut-depth` | engrave, fluting, texture, inlay, thread | `18` mm, the model's default; refused for chamfer |
| `--start-depth` | every kind | `0` |
| `--ramp` | fluting | `0`, the in-place plunge |
| `--spacing` | texture | The tool library's `stepover`; with neither, the command refuses |
| `--angle` | texture | `0` degrees |
| `--inlay-allowance` | inlay | `0`, the clearance between plug and cavity |
| `--stepover`, `--clear` | inlay | The tool library's `stepover`, or `clear_stepover` with `--clear` |
| `--pass-depth` | inlay | The tool library's `stepdown`; without it the command refuses |
| `--pitch` | thread | Required, in millimetres |
| `--thread-direction` | thread | `internal` |

The document is written only after the whole command succeeded, so every refusal exits `2` with the file
byte-identical. An inlay appends both halves or neither.

## Where the record lives

| Question | Where |
| --- | --- |
| What M6 had to be, and every amendment to it | `odd/tasks/008-m6-depth-variants.md` — S1 … S14, A1 … A11, D-M6-1 … 14, L1 … L17 |
| What each kind does in the document | [format.md](format.md#document-c2d), one row per `kind` |
| What the simulation authorises for each | [m5-simulation.md](m5-simulation.md#the-target-surface), one row per operation |
| How a pocket, a profile and a drill behave | [m4-toolpaths.md](m4-toolpaths.md) |
| How islands, parity and the `@2` tag work | [region-booleans.md](region-booleans.md) |
| What M6 delivered, and what it cost | [odd/roadmap/index.md](../../odd/roadmap/index.md) §4.9 |
| The project-level summary | [README.md](../../README.md) |
