# Pocket and drilling toolpaths (M4)

M0 cut a profile. M4 adds the two strategies that turn a sheet into a part: a **pocket** clears the
area inside a vector, and **drilling** puts a hole at the centre of every selected circle. Both are
authored headlessly — `cam-cli toolpath` appends one toolpath to an existing document — and both reach
G-code through `cam-cli emit` the same way a profile does.

```console
$ cam-cli toolpath part.c2d --kind pocket --entities E1 --tool 1 \
    --strategy raster --stepover 3 --pass-depth 6 --cut-depth 12
added TP1 pocket (raster), tool T1, 1 entity(ies)

$ cam-cli toolpath part.c2d --kind drilling --entities E2,E3 --tool 1 \
    --peck-depth 3 --cut-depth 12 --bank 3:32 --library tools.sqlite
added TP2 drilling (bank), tool T1, 2 entity(ies)
bank of 3 drill(s) at 32 mm pitch: D2 cuts E2, D2 cuts E3; head travel 108.135 mm

$ cam-cli inspect part.c2d
toolpaths: 2
  TP1 "Pocket 1": pocket (raster), tool T1, 1 entity(ies)
  TP2 "Drilling 2": drilling (bank), tool T1, 2 entity(ies)
```

The document above holds one 100 mm square and two circles. The pocket toolpath names the square, the
drilling toolpath names the two circles, and the bank report is the strategy's own answer, not a
second implementation of it.

## The three operations

| Operation | `operation` in the document | What it produces | Since |
| --- | --- | --- | --- |
| `profile` | `{"kind":"profile","side":"outside",…}` | One closed path per selected contour, offset by the tool radius and `allowance` to the chosen side, in depth passes, with lead-in/out and tabs | M0 |
| `pocket`, offset | `{"kind":"pocket","strategy":"offset",…}` | Concentric closed rings inside the contour, outermost first, one per `stepover`, until the inward offset collapses | M4 |
| `pocket`, raster | `{"kind":"pocket","strategy":"raster",…}` | Parallel rows along X, `stepover` apart, each clipped to the inset region | M4 |
| `drilling` | `{"kind":"drilling",…}` | A plunge at the centre of each selected circle, in a nearest-neighbour tour, in pecks | M4 |
| `drilling`, banked | the same, plus `"bank":{"count":3,"pitch":32.0}` | The same holes; each one names the bank drill that cuts it, and the emitted XY is the head's compensated position | M4 (selection only) |

A toolpath cites one or more entities, and each cited contour is machined on its own: a pocket clears
each selected contour as its own region, and a drilling toolpath treats each selected contour as one
hole. An unknown `kind` is still a hard load error rather than a skipped toolpath, which is why adding
these two variants kept the schema tag `c2d/document@1`; the contract rows are in
[format.md](format.md#document-c2d).

Every strategy produces the same move vocabulary, so every one of them reaches G-code:

| Unit of work | Moves emitted |
| --- | --- |
| A ring | `Rapid` to the ring's first vertex at `safe_z`, `Feed` down to the pass depth, the ring's segments in order (arcs stay arcs), `Retract` to `safe_z` |
| A raster span | `Rapid` at `safe_z`, `Feed` down, one straight `Feed` to the span's other end, `Retract` to `safe_z` |
| A hole | `Rapid` to the hole at `safe_z`, then one `Feed` down and one `Retract` per peck; the last retract of a hole is always at `safe_z` |

## Parameters

The parameter names are the vocabulary a toolpath form uses for these knobs, and every program spells
them its own way. This is where each one is carried:

| Form name | Where M4 carries it | Applies to | What it does |
| --- | --- | --- | --- |
| `ProfileSide`: `Profile Inside` / `Profile Outside` / `Profile On` | `side` | profile | Which side of the vector the tool centre rides |
| `Pass Depth`, `ProfilePass` | `pass_depth` | profile, pocket | Depth removed per pass; the last pass lands exactly on `cut_depth` |
| `PocketAllowance` | `allowance` | profile, pocket | Material left on the vector; the tool centre rides `diameter/2 + allowance` inside the boundary |
| `PocketDepth`, `Cut Depth` | `cut_depth` | pocket, drilling (and profile) | Total depth below the stock surface |
| `Start Depth` | `start_depth` | profile, pocket, drilling | Depth at which cutting begins, for a part already partly machined |
| `Pocket Pass` | `strategy` + `pass_depth` | pocket | The form has one name here; M4 carries the ring/row pattern and the pass count as two fields |
| `stepover` | `stepover` | pocket | Ring or row spacing, as a toolpath parameter |
| `stepover` (as a tool rate) | `tool_cutting_data.stepover` | pocket | The default for `--stepover` when the flag is omitted |
| `clear_stepover` | `tool_cutting_data.clear_stepover` | pocket with `--clear` | The default stepover for a clearing pocket |
| `Safe Z` | `job.safe_z` | every operation | The Z every retract and every rapid between rings, rows, spans and holes goes to |
| `Cut Rate` / `Plunge Rate` | `tool_cutting_data.feed_rate` / `plunge_rate` | every operation | The `F` values in the program |

`stepover` is a toolpath field, not a tool field: the same tool pockets MDF and hardwood differently,
and the spacing belongs to the cut rather than to the tool. The library's `stepover`,
`clear_stepover` and `stepdown` are the **defaults** the CLI fills in when a flag is omitted (D-M4-3),
which is also why no field was added to `ToolSpec` and no existing profile test had to move.

## Rules

### The tool centre rides `diameter/2 + allowance` inside the boundary

Both modes inset the boundary by `inset = diameter/2 + allowance`, so an allowance of `a` leaves
exactly `a` millimetres of wall standing and the tool's flank never touches the vector. Rings are
offset from the **original** contour rather than from the previous ring: chaining offsets accumulates
the kernel's join error and lets the stepover drift.

### Offset rings, each proving its distance

Ring `k` is the boundary offset inward by `inset + k·stepover`, outermost first. The ringing stops at
the first ring the offset kernel refuses, **or** at the first ring that fails a distance proof
(D-M4-9): every vertex must sit at least `distance − 0.002 mm` from the original boundary, the slack
being one chord sagitta of the kernel's 0.001 mm flattening tolerance. The proof is not paranoia. A
shrink wider than a feature can come back as a well-formed smaller ring with its winding unchanged — a
3 mm square inset by 2 mm gives a 1 mm ring — and the next ring then grows, so trusting the kernel
would emit a ring that cuts the wall it was meant to leave and loop forever. If the *first* ring fails,
the tool never fitted and the request is refused as `OffsetCollapsed` (`the tool does not fit: …`),
never emitted as a degenerate path. Rings keep their arcs as arcs.

### Raster rows

The boundary is inset once by `inset` to give the region; a refused or unproven inset is
`OffsetCollapsed`. Rows sit on the grid `region.min.y + k·stepover` while the line fits inside
`region.max.y`, and the lattice is then **finished**: when that grid stops short of `region.max.y` —
the stepover does not divide the region's height — exactly one more row is cut on `region.max.y`
itself. When a grid row already lands on `region.max.y` no extra row is emitted, because that row is
already cut and already retried onto the material, and appending would plunge into the same strip a
second time; the gap is compared against the kernel tolerance, so a row a millionth of a millimetre
from the edge counts as on it. The finishing row is clipped, zigzagged and retracted exactly like a
grid row, because it is one.

Each row is clipped by intersecting a horizontal line with every segment of the region: the x of every
intersection is sorted, deduped within the kernel tolerance (two segments meeting at a vertex both
report that vertex) and paired even-odd into spans. Span ends are therefore true line/arc
intersections, not tessellated approximations. A row whose clipping is ambiguous — a leftover odd
point, which is a tangency, or a `Coincident` result, which is a row lying along a straight boundary —
is retried once nudged `1e-5 mm` inside the region, ten times the kernel tolerance, and dropped if
that is ambiguous too: half a row is worse than none, because it leaves the tool in material it cannot
reach from the other side. Consecutive rows alternate direction (zigzag), and a row with several spans
retracts between its spans, not only between rows (D-M4-10). Rows are straight, so an arc-bearing
region produces no arc moves in raster mode.

### Depth passes, shared with profile

Both modes chunk depth with the rule the profile already uses: equal steps of `pass_depth` from
`start_depth`, the last pass exactly at `cut_depth`. A 12 mm cut at a 5 mm pass depth is
`[5, 10, 12]`. The ring or row geometry is computed once and cut at every depth, shallow to deep, so a
12-ring pocket at three passes is 36 units of work and every ring appears once per pass at that pass's
Z. Drilling reuses the same rule for its pecks.

### Retracts

Every retract — between rings, between rows, between the spans of one row, and between holes — goes to
`job.safe_z`, and every rapid between them happens at `safe_z` (D-M4-6, D-M4-10). Staying down across
already-cleared material is a linking optimisation that needs a gouge-free proof, and that belongs with
the simulation milestone. The recorded cost is more rapids than a hand-written program.

## Drilling

### Which contours can be drilled

A hole is the centre of a closed contour whose segments are all arcs about one common centre sweeping
a full turn — exactly what the circle primitive builds (D-M4-5). Any other closed contour is refused
by name rather than drilled at a guessed "geometric centre", because a hole in the wrong place is a
ruined part:

```console
$ cam-cli toolpath part.c2d --kind drilling --entities E1 --tool 1 --peck-depth 3 \
    --cut-depth 12 --library tools.sqlite
added TP3 drilling, tool T1, 1 entity(ies)
$ cam-cli emit part.c2d --post post_mm.pp --library tools.sqlite
error: toolpath "TP3": entity "E1" is not a circle, so it has no drill centre
$ echo $?
2
```

The check runs where a contour is resolved into a hole — `cam-cli emit`, and the bank report
`cam-cli toolpath` prints — so appending a drilling toolpath over a rectangle succeeds and the program
is what gets refused. Nothing is written to the document by the refusal.

### The tour

Holes are drilled in a nearest-neighbour tour seeded at the lowest-Y-then-lowest-X hole, ties broken by
entity id, and each step takes the nearest unvisited hole with ties broken by entity id. Ordering is
decided on the document centre the user drew, so "lowest Y" means what the drawing shows whatever the
job's origin corner is; distances are measured in machine space, which an origin transform translates
or flips without changing any of them. The scan is a `Vec` walk, never a map, so the same document
always produces the same program. The strategy reports the tour in cutting order, and the entity id
travels with each hole so that a refusal and the bank report can name it.

### Pecks and retracts

`peck_depth > 0` plunges in steps of at most `peck_depth` using the depth rule above, so the last peck
lands exactly on `cut_depth`; `peck_depth = 0` is one plunge. Between pecks `RetractMode::Amount(d)`
lifts `d` millimetres above the bottom of the peck just finished, and `RetractMode::Full` goes to
`safe_z`. The retract that ends a hole is always at `safe_z` whatever the mode says, and the tool
rapids to the next hole at `safe_z`.

### The drill bank is a selection, not a machine command

A `bank` of `count` drills at `pitch` millimetres is a gang mounted along X on one head. The machine
cannot swap drills, so the head moves: drill `k` is over a hole at `x` when the head is at
`x − k·pitch`, and the emitted XY is that head position with Y unchanged. For each hole in the tour the
strategy picks the `k` in `0..count` whose head is nearest the previous head — the first hole's
previous head is the machine origin — by rounding `hole.x − previous_head.x` to the nearest whole
number of pitches and clamping to the drills the bank actually has; a tie keeps the lower index. The
choice is reported per hole, and the tour order is unchanged: a bank selects, it does not re-plan.

Two travel figures, because a bank makes them different numbers:

| Figure | What it measures |
| --- | --- |
| `rapid_length` | The XY distance between consecutive holes in tour order, counted from machine `(0,0)`: how far apart the holes are |
| `bank_travel` | The XY distance the head actually travels, counted from machine `(0,0)`: what the program costs |

Without a bank the head visits the holes themselves, so the two are equal. With one they are not: the
worked example below is 120 mm of hole spacing against 60 mm of head travel.

**No bank command is emitted.** Nothing of the bank reaches the program: no `G` code, no `M` code, no
toolchange, no tool number per drill. Drill-bank hardware integration is deferred by the plan, and the
move vocabulary has no bank command to emit, so what a bank gives the operator is the per-hole
selection and the head travel — which `cam-cli toolpath` prints after appending the toolpath — and
what it gives the program is the compensated head position. `cam-cli inspect` calls such a toolpath
`drilling (bank)`.

## Known limits, each a decision

- **Islands** (D-M4-2, since retired). M4 shipped a pocket as one region per selected contour, so a
  nested selection pocketed each contour separately. That limit is gone: the kernel gained region
  subtraction and both pocket modes cut around explicit islands — see
  [region-booleans.md](region-booleans.md), where the model, the distance proof and the `@2` document
  contract are documented and evidenced.
- **Circles only for holes** (D-M4-5). Any other closed contour is refused by name. A non-circular
  closed contour has no single centre to drill at, so refusing it is a rule rather than a fallback.
- **Conservative linking** (D-M4-6, D-M4-10). The tool retracts to `safe_z` between rings, rows, spans
  and holes; there is no stay-down linking and no gouge-free optimisation. Cost: more rapids than a
  hand-written program.
- **No raster angle** (D-M4-7). Raster rows always run along X. A raster angle is an additive parameter
  for a later milestone.
- **No toolpath UI** (D-M4-8). There is no toolpath panel in the shell; `cam-cli toolpath` is how a
  toolpath is created in this milestone. The panel rides on application commands that do not exist for
  toolpaths yet, so the missing panel is a decision rather than a gap.
- **One closed error set.** Every refusal these strategies can produce is listed here, and none of them
  is a placeholder — the temporary stub the two strategies were built behind is gone, so a stub cannot
  survive as a contract:

| Error | Raised by | Message |
| --- | --- | --- |
| `OpenContour` | profile, pocket, drilling | `cutting toolpaths need closed geometry` |
| `InvalidToolDiameter` | pocket, drilling | `tool diameter must be greater than zero` |
| `InvalidPassDepth` | pocket (and profile) | `pass depth must be greater than zero` |
| `InvalidStepover` | pocket | `stepover 0 mm must be greater than zero` |
| `OffsetCollapsed` | pocket | `the tool does not fit: …` |
| `InvalidPeckDepth` | drilling | `peck depth -1 mm must not be negative` |
| `NoHoles` | drilling | `drilling needs at least one hole` |
| `InvalidBank` | drilling | `drill bank needs at least one drill and a positive pitch` |
| `InvalidDepthRange` | pocket, drilling (and profile) | `cut depth 6 mm must be greater than start depth 6 mm` |
| `NotACircle` | `cam-cli`, where a hole is resolved | `entity "E1" is not a circle, so it has no drill centre` |

## `cam-cli toolpath`

```text
cam-cli toolpath <document> --kind profile|pocket|drilling --entities <ids> --tool <number>
                 [--out <path>] [--library <path>] [--id <id>] [--name <name>]
                 [--side inside|outside|on] [--allowance <mm>]
                 [--strategy offset|raster] [--stepover <mm>] [--clear]
                 [--pass-depth <mm>] [--start-depth <mm>] [--cut-depth <mm>]
                 [--lead-in <mm>] [--lead-out <mm>] [--tab <pos:width:height>]
                 [--peck-depth <mm>] [--retract full|amount] [--retract-amount <mm>]
                 [--bank <count:pitch>]
```

| Option | Applies to | Default, and where it comes from |
| --- | --- | --- |
| `--kind` | — | Required: `profile`, `pocket` or `drilling` |
| `--entities` | — | Required: comma-separated ids; each one is its own region or hole |
| `--tool` | — | Required: the tool number in the library |
| `--out` | — | The document itself, written back after the command succeeds |
| `--library` | — | The document's `tools_ref`, resolved relative to the document; an explicit path that does not exist is an error |
| `--id` | — | `TP<n>`, the first free number at or above `toolpaths.len() + 1` |
| `--name` | — | `Profile <n>` / `Pocket <n>` / `Drilling <n>`, with `n = toolpaths.len() + 1` |
| `--side` | profile | `outside` |
| `--allowance` | profile, pocket | `0` |
| `--strategy` | pocket | `offset` |
| `--stepover` | pocket | The tool's `stepover` from `tool_cutting_data`, or its `clear_stepover` with `--clear`; with neither, the command refuses: `a pocket needs --stepover: the tool library has no stepover for that tool` |
| `--clear` | pocket | Off; selects `clear_stepover` as the default instead of `stepover` (D-M4-4) |
| `--pass-depth` | profile, pocket | The tool's `stepdown` from `tool_cutting_data`; without it the command refuses |
| `--start-depth` | profile, pocket, drilling | `0` |
| `--cut-depth` | profile, pocket, drilling | `18` mm, the model's default |
| `--lead-in`, `--lead-out`, `--tab` | profile | `0`, and no tabs |
| `--peck-depth` | drilling | `0`, one plunge |
| `--retract` | drilling | `full`; `amount` requires `--retract-amount` |
| `--bank` | drilling | None; `count:pitch`, both read as drills and millimetres |

The document is written only after the whole command succeeded: the result is validated (which is what
refuses a toolpath over open geometry) and any refusal exits `2` leaving the file byte-identical. The
command prints one `added` line, and a banked drilling toolpath prints its selection line as well.

## Worked examples

### Offset pocket: a 100 mm square, 6 mm tool, 4 mm stepover

`inset = 6/2 + 0 = 3 mm`, so ring `k` is the square offset inward by `3 + 4k` and is itself a square
of side `100 − 2·inset`:

| Ring | Inset | Ring size |
| --- | --- | --- |
| 1 | 3 mm | 94 mm square |
| 2 | 7 mm | 86 mm square |
| … | … | … |
| 12 | 47 mm | 6 mm square |
| — | 51 mm | does not fit: the ringing stops |

Twelve rings, outermost first, and the 51 mm inset is where the region collapses.

### Raster pocket: the same square at a 3 mm stepover

The inset region spans `y = 3 … 97`, 94 mm tall, so the rows are the grid `y = 3 + 3k` that fits,
finished with one row on the far edge:

| Row | y |
| --- | --- |
| 1 | 3 mm |
| 2 | 6 mm |
| … | … |
| 32 | 96 mm |
| 33 | 97 mm — the finishing row, on the far edge |

`96 + 3 = 99` is past 97, so the grid stops at 32 rows and ends `1 mm` short of the region's far edge;
the lattice is therefore finished with one row on `region.max.y`, giving **thirty-three rows**, each
spanning `x = 3 … 97`. The first row sits exactly on the region's lower edge, which makes its clipping
ambiguous, so it is the one the `1e-5 mm` retry exists for. A far edge the grid already reaches gains
nothing: the same square with a 2 mm tool at a 2 mm stepover keeps its `50` rows at `y = 1, 3, … 99`,
because `99` *is* a grid row and cutting it twice would plunge into the strip it already clears. A
circular region shows the consequence rather than a special case: a 40 mm circle under a 6 mm tool
leaves the inset radius 17, whose region is `y = 33 … 67`; the grid ends at `65`, and the finishing row
at `67` is tangent, so the retry cuts the chord `2·√(17² − (17 − 1e-5)²) ≈ 0.037 mm` — which is why
`circle_raster_spans_match_the_analytic_chord` expects 10 rows and not 9.

### Drill bank: holes at x = 0, 10, 40, 100 with 3 drills at 32 mm

All four holes are at `y = 20`, and the machine origin is `(0, 0)`:

| Hole x | Previous head x | `dx` | Pitches `dx/32` | Drill `k` | Head x = `x − 32k` |
| --- | --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 0 | 0 |
| 10 | 0 | 10 | 0.31 | 0 | 10 |
| 40 | 10 | 30 | 0.94 | 1 | 8 |
| 100 | 8 | 92 | 2.88, clamped to the last drill | 2 | 36 |

That is `drill_index = [Some(0), Some(0), Some(1), Some(2)]`, with the head travelling
`20 + 10 + 2 + 28 = 60 mm` — against `rapid_length = 20 + 10 + 30 + 60 = 120 mm` for the holes
themselves. The last hole is 64 mm from the drill that cuts it, which is why what a bank bounds is the
head's step, not its distance from the hole.

All three examples are asserted in `crates/cam-toolpath/tests/pocket.rs` and
`crates/cam-toolpath/tests/drill.rs`.
