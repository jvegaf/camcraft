# Simulation and the 3D preview (M5)

M4 shipped three strategies whose proof was a green suite and a byte-for-byte golden program. That says
the toolpath satisfies its rules; it does not say the part comes out right. M5 adds the missing half — the
stock itself. A new `cam-sim` crate sweeps the emitted program over a heightfield of the material, derives
from the document what each cell was **authorised** to lose, and reports the two ways a program can
disagree with it. `cam-cli simulate` is that report, and the shell can show the same program in 3D and
play it.

```console
$ cam-cli simulate part.c2d --library tools.sqlite
simulate: 200.000 x 100.000 x 18.000 mm, 1 x 1 mm cells (200 x 100), 1 toolpath(s)
program: 200 move(s), 4900.000 mm cutting length
removed: 60000.000 mm3 of 360000.000 mm3, 0 through cell(s)
gouge: 0.000 mm3 over 0 cell(s)
uncut: 0.000 mm3 over 0 cell(s)
crash: none
```

The document above is a 100 mm square cleared by a raster pocket — a 2 mm tool, a 2 mm stepover, 6 mm
deep — on the fixture's 200 x 100 x 18 mm stock: 50 rows of 4 moves, 50 x 98 mm of cutting, and
100 x 100 x 6 mm³ of material gone. The report is the contract, so it is printed in one order with one
decimal count and pinned byte for byte.

| Piece | Where |
| --- | --- |
| The grid, its cell size and its hard cap | `cam-sim::grid` |
| The stock as remaining surface heights, and its volumes | `cam-sim::heightfield` |
| Swept removal over a `MoveStream`, and the rapid that rams it | `cam-sim::sweep` |
| The authorised depth per cell, and the two deviations | `cam-sim::target` |
| `simulate`, its report and its exit code | `cam-cli` |
| The orbit camera and the projected surface mesh | `cam-view::camera3d`, `cam-view::mesh` |
| The 3D panel, its gestures and playback | `cam-ui::view3d` |

The contract rows for `simulate` and exit `6` are in [format.md](format.md); the decisions are
D-M5-1 … D-M5-11 and the log is L1 … L9 in the
[feature document](../odd/tasks/m5-simulation.md); the milestone's evidence is §4.8 of the
[roadmap](../odd/roadmap.md), and the project-level summary is in the [README](../README.md). The
simulation consumes the same `MoveStream` `cam-cli emit` hands the post-processor, never a strategy's
intentions (D-M5-1): an oracle that re-reads the intent it is meant to check is not one. A defect the
strategies share with the emitter is therefore simulated faithfully, which is the point.

## The heightfield

The stock is one surface height per cell, in machine Z. The stock's top is `0` and its bottom is
`-thickness`, because the strategies emit their cutting moves at negative Z and a cutting depth is
written as a positive magnitude. A cell is never raised and never pushed below the bottom, so every value
in the field is a height a real tool could leave behind.

The grid covers the job in **whole cells**: `columns = ceil(width / cell)` and `rows = ceil(height / cell)`,
with cell `(i, j)` centred at `origin + (i + 0.5) · cell`. Its area can therefore exceed the job's by less
than one cell in each direction, and every volume is measured on the grid rather than on the nominal
rectangle — which is what makes `removed + remaining` exactly the grid's volume instead of approximately
the stock's.

| Number | Value | Why |
| --- | --- | --- |
| Default cell | `1` mm | Keeps the default 1219 x 2438 mm sheet at 2,971,922 cells, under the cap, and is the resolution the oracle's error bound is written for |
| Cell cap | `4,000,000` cells | 32 MB of `f64` surface heights: enough for the default sheet at a millimetre, small enough that a mistyped resolution is refused instead of attempted |
| Coverage bound | `perimeter × cell / 2` | A cell is cut when its **centre** is inside the swept disc, so a boundary is wrong by at most half a cell; times the perimeter is the area the report can be off by, and times the depth the volume |

A request over the cap fails **before any allocation**, with exit `2`, and the error carries all three
numbers a reader needs — the cell asked for, the grid it implies and the cap. On the default
1219 x 2438 mm sheet a `0.1` mm request is refused as `12190 x 24380 = 297192200 cells, over the
4000000 cell cap`, followed by the one thing to do about it: `use a coarser --resolution`.

The cap is a documented cell count, not a time limit, and the coverage bound is published rather than
tuned: refining the resolution never grows the error on a fixed fixture, which is the assertion A6 asks
for. The alternatives D-M5-3 rejected are exact analytic area coverage (a kernel of its own, and arc
integral arithmetic at every boundary) and supersampling (a hidden tolerance knob).

## The sweep

A cutting move — `Feed` or `Arc` — removes material to its destination `z` for every cell whose centre
lies inside the disc the tool sweeps along it:

- **Arc moves are swept as arcs.** The circle comes from the move's own centre and radius, so a full circle
  leaves the middle of the circle standing. A chord sweep would cut the hole solid, and the test that
  proves it is a full-circle arc whose centre cell is still at `0`.
- **A `Rapid` never cuts.** A rapid or a retract at or above the surface removes nothing and is not a
  crash. A rapid that ends **below** the surface at its destination cell is a crash: it is reported with
  its toolpath, its move index and its position, and it does not cut. M4's conservative linking retracts
  to `safe_z` precisely so that no rapid crosses material (D-M4-6), so a program that does has a defect,
  and D-M5-4 refuses to hide that defect inside a plausible volume number.
- **A feed above the surface cuts air.** Lowering a cell to a height above its current one is not a
  raise, so an approach that clears the stock leaves it untouched.
- **The vertical ramp is not modelled.** A move removes to its destination `z` across its whole footprint
  rather than interpolating from the previous height, so a ramping move cuts early rather than late. The
  error is in the one direction that keeps the report honest: no material is claimed to be standing that
  the tool did not leave standing.

Sampling is deterministic and a function of the move's own numbers:

| Move | Sampling |
| --- | --- |
| Line | A step of `cell / 2` along its length, a disc of the tool radius applied at each sample |
| Arc | An angular step of `cell / (2 · r)` radians, `r` being the arc's own radius, a disc at each sample |

Sampling at half a cell is what makes the union of discs cover the swept capsule within one cell, which is
inside the coverage bound above. The stream carries no explicit start position, so a segment's first move
starts where it ends: a plunge is a single disc rather than an approach the program never declared. The
tool radius comes from the `Segment`'s own `tool_diameter`, which M5 added to the move vocabulary because
the stream carried the tool's number but not its size — the emitter ignores the field, and its tests are
unchanged.

## The target surface

One rule, derived from the document alone: **the depth each cell is authorised to lose**, and zero
everywhere the document claims nothing. Each operation's form of it:

| Operation | Cells it claims | Authorised depth |
| --- | --- | --- |
| `pocket` | Its region — boundaries minus islands — inset by `allowance` | `cut_depth` |
| `drilling` | Each selected hole's circle | `cut_depth` |
| `profile` | The band the tool **sweeps**: `allowance … 2·radius + allowance` outside the vector, the same band inwards for `inside`, and `±radius` for `on` | `cut_depth`, less a tab's `height` inside the tab's span |
| Anything else | — | `0` |

Three of those rows are decisions rather than arithmetic:

- **The band is the material the tool sweeps, not the distance its centre travels** (D-M5-5, amended by
  evidence). The emitter offsets the tool *centre* by `radius + allowance`, so a clean profile removes
  from `allowance` to `2·radius + allowance` outside the vector. Authorising only `0 … radius + allowance`
  reported the outer half of every perfect kerf as gouge — measured as `gouge: 1632.0 mm3 over 272
  cell(s)` on a clean 100 mm `Outside` cut before the amendment. An allowance is material the emitter
  keeps, so the target does not authorise it (D-M5-10): a pocket's region is inset by it in one
  whole-region `Region::offset`, which grows the islands in the same call. With the default allowance of
  zero the target is the region itself, cell for cell.
- **`start_depth` is where the passes begin, not what they add.** The emitter's `pass_depths` walks from
  `start_depth` to `cut_depth` and lands on it, so the deepest cut is `cut_depth` itself. Adding the two
  authorised material no program cuts.
- **A cell claimed twice is authorised the deeper claim**, never the sum: two toolpaths that both cut a
  cell remove it once.

Gouge and uncut are then one subtraction with two signs (D-M5-2): with `removed = -surface`,

- **gouge** where `removed > authorised + tolerance` — material removed deeper than the document asked;
- **uncut** where `authorised > removed + tolerance` — material standing above what a toolpath claimed.

Each is reported as a volume and a cell count, from the same cell set in one deterministic order, and a
test that proves one sign proves the arithmetic of the other. Per-toolpath attribution is that same
comparison over a toolpath's own moves rather than a second rule; the CLI report prints the whole-stock
totals. A field and a target on different lattices are refused by name, naming both grids, because a cell
comparison across two resolutions is meaningless.

## `cam-cli simulate`

```text
cam-cli simulate <document> [--library <path>] [--resolution <mm>]
```

The tool library resolves exactly as `emit`'s does — `--library`, else the document's `tools_ref` relative
to the document — and the segments are built by the same function the emitter uses. `--resolution` is the
cell size in millimetres and defaults to `1`.

The report is six lines, in this order: the job, the grid and the toolpath count; the program's move count
and cutting length; removed volume against the grid's volume with the through-cut cell count; gouge; uncut;
and the crashes, one line per crash in program order, or `crash: none`.

```text
simulate: <w> x <h> x <thickness> mm, <cell> x <cell> mm cells (<columns> x <rows>), <n> toolpath(s)
program: <n> move(s), <mm> cutting length
removed: <mm3> of <mm3>, <n> through cell(s)
gouge: <mm3> over <n> cell(s)
uncut: <mm3> over <n> cell(s)
crash: <toolpath> move <index> at (<x>, <y>, <z>)
```

Volumes and coordinates carry three decimals through `cam-units`' locale-free formatting, and the cell size
is trimmed (`1` and `0.5`, not `1.000` and `0.500`), so two runs of the same document print identical
bytes. `program:` counts the stream's moves, which is two fewer than the G-code's move lines because the
post's footer is not a stream move.

**The deviation tolerance is `cell / 8`.** The sweep samples at half a cell, so its own boundary error is
`cell² / (32 · radius)` — a hundredth of a millimetre for a 1 mm cell and a 3 mm tool — while the defects
the tolerance exists to catch are a whole millimetre deep. An eighth of a cell is above the sampling noise
and below the defect, at the resolution the bound is written for.

| Exit | Meaning |
| --- | --- |
| `0` | The program cut what the document authorised: no gouge, no uncut material, no crash |
| `6` | The simulation found defects, and stderr names which: gouge, uncut volume, or a crash count |
| `2` | Invalid input: an unknown option or a bad argument, an unreadable document or library, missing tool data, a grid over the cell cap, a target the geometry refuses |
| `3` | An unsupported document schema, through the same document load every command uses |

**Nothing is written.** A simulation is a question about the document, not a change to it: no document, no
program, no cache, and the file on disk is compared before and after by the tests that pin the report.

## Known limits, each a decision

- **The tool is a flat cylinder** (A13). `ToolSpec` carries a number, a diameter, a feed rate, a plunge
  rate and a spindle speed and no shape, so the sweep is a disc of the tool's diameter and nothing else.
  V-bit and tip-geometry simulation is a recorded limit rather than an omission: the target model is
  depth-based, so it does not need one, and the strategy that would need a tip geometry is M7's.
- **A convex corner over-authorises, by a bound that is written down** (D-M5-11). The profile band comes
  from `offset_contour`, whose joins are mitered (`miter_limit: 4.0`), while the removal is a union of
  discs of the tool radius along that path. Along a straight or smooth vector the two agree. At a convex
  corner the mitered band claims material the disc never touches, at most `4(r² − πr²/4)` per right-angled
  corner — about `7.73 mm² × depth` at a 6 mm tool — so a perfect square profile reports 24 mm³ over 4
  cells at a 1 mm cell, converging to that limit as the grid refines. The case where the statement is
  exact is the **circular contour**, which has no convex corner for the miter to over-claim: the oracle's
  circular `Outside` profile reports `Deviation::default()`, zero on both signs. A round-join offset would
  remove the disagreement and is a kernel feature this milestone does not add; a tolerance tuned to
  corners would be exactly the resolution knob D-M5-3 refuses.
- **A stepover of two radii is tangent, and tangency is a resolution question.** A 2 mm stepover under a
  2 mm tool leaves adjacent rows exactly tangent, so the same raster program at `--resolution 0.5` reports
  `uncut: 300.000 mm3 over 200 cell(s)`: the cell centres between two tangent rows are past the radius.
  The raster fixture pins the row lattice at 1 mm cells, which is the resolution the coverage bound is
  written for. It is not a resolution-independent claim, and it is stated rather than implied.
- **A cut deeper than the stock reports as uncut** (L6). A document whose `cut_depth` exceeds the stock's
  thickness authorises more than the field can remove, so the difference is reported as uncut — the honest
  reading of "the program does not reach the declared depth". The field itself stops at `-thickness` and
  never goes through it.
- **No parallel or tiled removal** (D-M5-8). The removal loop is single-threaded and untiled, and `rayon`
  is not a dependency. M5's job sizes are bounded by the cell cap, and a parallel loop would need its own
  determinism argument for the volume sums. Recorded trigger: parallel or tiled removal lands when a
  measured simulation of a default sheet at the default resolution exceeds one second.
- **Linking stays conservative** (D-M4-6, unchanged). M4 retracts to `safe_z` between rings, rows, spans
  and holes because staying down needs a gouge-free proof. M5 supplies the proof — a stay-down link can
  now be simulated instead of argued about — and adds no linking optimisation of its own, so the cost M4
  recorded (more rapids than a hand-written program) still stands.

## The 3D preview

The picture is data first and paint second: the projection and the surface live in `cam-view`, are pure
functions of their inputs, and are tested headless. The shell only turns them into `egui::Mesh` triangles
and lines on the existing painter, which is why the 3D view needs no new dependency (D-M5-7). A `wgpu`
backend would consume the same mesh when D-M1-2's trigger fires; nothing below `cam-ui` would change.

### The camera

Four numbers: `yaw` around the vertical, `pitch` above the horizontal plane, `distance` from the target,
and the `target` itself — plus the viewport it projects into. The eye sits on a sphere around the target
and a perspective projection is built from the resulting basis; screen space has its origin at the top
left with Y growing downwards, as the 2D camera does.

| Number | Value | Why |
| --- | --- | --- |
| Pitch clamp | `±(π/2 − 1e-3)` rad, about 0.06° short of vertical | The basis comes from world +Z, so there is no up vector to invent at the pole; the clamp keeps the right vector from collapsing |
| Distance range | 1 mm … 100 m | The near end is a camera inside the stock, the far end covers the largest sheet in millimetres |
| Vertical field of view | 45° | A standard perspective: wide enough to frame a stock, narrow enough not to warp a straight edge at the border |
| Defaults | yaw 0.75 rad (≈43°), pitch 0.55 rad (≈31°), distance 600 mm | The three-quarter view an operator expects, at a starting distance the caller reframes |

`orbit` holds the pitch inside the clamp whatever delta it is given, and `zoom` holds the distance inside
its range and ignores a factor that is zero, negative or not finite, so a drag or a wheel event cannot
invert or destroy the camera. `project` returns `None` behind the eye, at the eye's own plane, for input
that is not finite, and for a viewport with no area — a screen coordinate computed there would appear in
front of the camera and be wrong.

### The mesh, the ramp and the paint order

`build_surface` samples every `stride`-th grid line and always the last one, so a stock is never cut short
of its far edge. The sampled lattice is `ceil((columns − 1) / stride) + 1` by the same in rows — exactly
the field when `stride` is `1`, which is what makes the mesh the field's own geometry rather than an
approximation of it — with two triangles per sampled cell pair. A stride of `0` is read as `1`, never as a
divisor. The triangle count is therefore bounded by the stride rather than by the job's cell count
(D-M5-7), and the panel picks a stride that keeps both sides within 200 sampled lines.

Each vertex carries a **depth fraction**, `-height / thickness` clamped to `0.0 … 1.0`: `0.0` at the
untouched top and `1.0` at the stock's bottom, exactly at both ends. It is the colour ramp's coordinate,
not a distance in millimetres, so the same palette says the same thing about an 18 mm sheet and a 6 mm
one.

Triangles are handed to the painter **back to front**, sorted by their mean projected depth with a stable
sort so ties stay in lattice order. That is not a style choice: `egui::Mesh` has no depth buffer, so the
order the triangles arrive in is the only depth ordering it can be given. The stock's twelve edges are
painted next and the program's moves last, on top of a picture that is already correct. A triangle with a
vertex the camera cannot project is skipped rather than clamped; drawing it would mean inventing a screen
position. Arcs are flattened at half a pixel, and only by the painter — every earlier stage keeps the arc
as an arc.

### Playback

The time model is the program's own. A move takes its length over the segment's rate: `plunge_rate` for a
move that travels in Z alone, `feed_rate` otherwise, and an arc measured along its own circle rather than
its chord. A rapid and a retract are timed at the segment's feed rate, because `Segment` carries no rapid
rate and the clock does not invent a machine number. `advance(dt)` consumes `dt × speed` seconds of that
program time, the panel's default speed being 60x, and a move is applied to the field only when the cursor
crosses it **whole**: a half-played move has removed nothing, so the cut is always a state the machine
really reaches. `step` advances exactly one move and pauses; `reset` puts the untouched stock back;
playback stops at the end and never loops.

Time enters as an injected delta. `Shell::show_in` is the one place in the shell that reads a clock — one
`stable_dt` per frame — and `Preview3d::advance` reads none, so a test drives a frame sequence and asserts
the same cursor, the same volumes and the same field a window would show. The field at the end of a
program is cell-for-cell the field a whole-program sweep produces, which is what keeps the animation and
the report from disagreeing.

`set_preview` takes the stock **before** the first move, not a finished simulation: playback cuts it, and
`reset` needs a pristine field because nothing can un-cut one. Handing in an already-simulated field would
leave the picture and the program disagreeing about where the first move starts.

The panel shows a program; it never authors one and never touches the document (D-M5-9). It adds no
application command and no path from the picture back into the geometry, and the 2D canvas keeps painting
its items and its grid while the panel is open. There is still no toolpath form in the window: a preview
is a `MoveStream` a caller hands in, authored through `cam-cli` — the native window's sample is built the
way `cam-cli toolpath` builds one, a strategy and its `ToolSpec` turned into a stream — and the authoring
surface stays where M4 left it (D-M4-8).
