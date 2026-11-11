# Region booleans and pocket islands

M4 shipped a pocket as one region per selected contour and recorded the gap: islands were out of
scope "until the kernel has boolean subtraction" (D-M4-2). This feature builds that subtraction in
`cam-geom` and gives the pocket islands to cut around. The roadmap's `cam-geom` description lists
"boolean ops"; what exists now is **subtraction expressed as parity**, plus the arrangement and the
distance proof that keep parity honest. A union does not exist and is not claimed — see
[known limits](#known-limits-each-a-decision).

```console
$ cam-cli toolpath part.c2d --kind pocket --entities E1 --islands E2 --tool 1 \
    --stepover 3 --pass-depth 6 --cut-depth 6
added TP1 pocket (offset), tool T1, 1 entity(ies), 1 island(s)

$ cam-cli inspect part.c2d
toolpaths: 1
  TP1 "Pocket 1": pocket (offset), tool T1, 1 entity(ies), 1 island(s)
```

The document above holds a 100 mm square (`E1`) and one circle inside it (`E2`). The pocket names the
square as its region and the circle as an island, which is the whole of the authoring surface: there
is no "island" shape in the document, and nothing is inferred from the drawing.

| Piece | Where |
| --- | --- |
| `Region`, `Normalized`, `Region::contains`, `subtract`, `normalize` | `cam-geom::region` |
| The arrangement behind `normalize` | `cam-geom::boolean` |
| `Region::offset`, `Region::raster_spans`, `RegionOffset` | `cam-geom::region_offset` |
| The exact minimum distance between two segments | `cam-geom::distance` |
| `winding_number`, `Membership::classify`, `Relation::relation` | `cam-geom::winding` |
| `Toolpath.island_ids` and its validation | `cam-model` |
| `--islands`, and the island count `inspect` prints | `cam-cli` |
| Both pocket strategies over a region | `cam-toolpath::pocket` |

The contract rows for the document field and the CLI flag are in [format.md](format.md), and the
decision history is the [feature document](../../odd/tasks/007-region-booleans.md) (D-RB-1 … D-RB-11,
L1 … L11).

## The model: a boundary list under parity

`Region { boundaries: Vec<Contour> }`. A point is **material when it is enclosed by an odd number of
boundaries**, and that single rule is the whole membership model:

```rust
let enclosed = region.boundaries.iter()
  .filter(|b| classify(b, point, tolerance) == Membership::Inside)
  .count();
enclosed % 2 == 1
```

A point that is `OnBoundary` for any boundary is not material at all.

Consequences, in the order they matter:

- **A hole is expressed by appending its boundary.** No subtraction is needed to *represent* one, and
  `Region::subtract(hole)` is literally that append followed by normalization (A5). There is no
  boolean kernel entry point that a caller has to pick over another.
- **A boss inside a hole needs no third concept.** Appending the boss's boundary flips the parity a
  third time, so `square / hole / boss` is three boundaries and one rule. Read the consequence
  carefully, because "islands are the contours to leave standing" stops being a safe shorthand at the
  second nesting level: the boss disk is inside three boundaries, which is odd, so it is **material and
  is cut**, and what stands is the annulus between the boss and the hole. Its rings shrink to `r9` and
  `r2` in the golden program for exactly that reason. A nested region's offset
  signs alternate with depth for the same reason (see [the pocket contract](#the-pocket-contract)).
- **Orientation is deliberately not part of the model** (D-RB-2). A reversed or mirrored import, a
  contour builder that emits clockwise where another emits counter-clockwise, a DXF written by a
  different package — all of them leave parity untouched, while membership-by-winding-polarity would
  make correctness depend on how a file happened to be written. `Region` never reads a signed area to
  decide what is inside; `Relation::relation` decides nesting, and it decides it by geometry.
- **A boundary is the edge of what can be cut, not material.** `contains` answers `false` for a point
  within the tolerance of any boundary, so two contours that share an edge never both claim it.

Islands are **explicit in the document and never inferred** (D-RB-1). A contour drawn inside another
may be an island or may be a separate feature, and no rule for nested selections can be read out of
the drawing itself — the same limit D-M4-5 recorded for drilling. Inferring "the inner contour must be
a hole" would silently reverse a user's intent, so the user names the islands and the model refuses to
guess.

The primitives underneath are exact and arc-native (S4):

| Primitive | Contract |
| --- | --- |
| `winding_number(contour, point) -> i32` | The signed turns the contour makes about the point, computed from each segment's own geometry — a line contributes the signed angle between the vectors to its ends, an arc contributes the angle it subtends at the point. No tessellation, no sampling. |
| `classify(contour, point, tolerance) -> Membership` | `Outside`, `Inside` or `OnBoundary` (within `tolerance` of any segment), so a caller never has to guess which of two touching contours owns an edge. |
| `relation(first, second, tolerance) -> Relation` | `Disjoint`, `FirstInside`, `SecondInside`, `Crossing` or `Touching`, decided by segment intersections plus one interior point — never by bounding boxes, which is how a contour in a C-shaped pocket's notch stays outside the material while inside its bounds. |

A single-point intersection counts as a crossing only when it lies **away from both segments' ends**;
a `Point` at an end is a shared corner, so two squares meeting at one corner are `Touching` and not
overlapping material.

## Normalization

`Region::normalize(tolerance)` restores the invariant a boundary list must have to mean anything: no
two boundaries cross. It does four things:

1. **Split** every boundary at every transversal crossing with another boundary, reusing the kernel's
   intersection and its exact splitting — a piece of an arc is still an arc, with the same centre,
   radius and a trimmed sweep.
2. **Classify** each piece by the parity of the material beside it, probing a tiny step to either
   side. A piece with material on both sides is an interior seam and is dropped; a piece with material
   on neither bounds a slot of no material and is dropped too.
3. **Orient** each survivor so the material is on its left, which is what makes the result a
   boundary list rather than a soup of segments.
4. **Walk** the arrangement into closed contours: at a shared point the walk continues with the next
   piece clockwise, so it turns at a crossing instead of passing through it.

Determinism (S5): pieces are processed in boundary order then piece order, and the walk starts from
the piece with the lowest `(min.y, min.x, order)`, so the same input produces the same contours
vertex for vertex.

### Why the walk is not `cam_geom::chain`

`chain` matches one piece's end to the next piece's start within a tolerance and takes the first
match. Every crossing is a point where four pieces meet, and that greedy match happily rejoins a piece
to its own boundary's continuation — handing back the input, crossings included, which is the one
thing normalization exists to remove. The arrangement is therefore walked with the angular
continuation rule above, and `chain` is left untouched for the loose-segment jobs it is right for.

### Why it is parity-preserving and not a union (D-RB-8)

Normalization **preserves the parity answer**; it does not take a union of overlapping boundaries. Two
10 mm squares offset by `(5, 5)` normalize to two L-shaped contours of **75 mm² each** (6 vertices
each, 150 mm² together) — their symmetrical difference, because the overlap is inside two boundaries
and therefore not material. Their union would be one octagon of **175 mm²**, which is probably what a
user drawing two overlapping rectangles has in mind, and it is not what `Region::contains` answers.

A union would need boundary orientation to decide which side is inside, which is exactly what D-RB-2
refuses, and it would be a new operation rather than a normalization. Nothing in the pocket needs one:
validation refuses an island that is not strictly inside its boundary and refuses islands that
overlap each other, so a valid pocket always wants subtraction — which parity gives for free.

What can be lost is exactness, and it is never lost silently (D-RB-5):
`Normalized { region, flattened }` and `RegionOffset { region, flattened, warnings }` say so. What
flattens is the same case `offset_contour` already flattens — its self-intersection removal — plus a
piece whose arc the contour model's bulge cannot hold.

## The distance proof

This is the safety property of the whole feature, and it is the rule that stops a gouging ring. An
offset that lands outside the material comes back from an offset kernel looking perfectly well formed:
a 3 mm square inset by 2 mm is a 1 mm ring with its winding unchanged, and in a ring loop the next
stage then grows. The only defence is to **prove** that a piece really is the offset it claims to be,
and the proof is stated about the whole piece: every point of it at least `distance − slack` from an
**original** boundary, where `slack` is two chord sagittae at the offset kernel's 0.001 mm tolerance,
i.e. 0.002 mm. A piece is one chained contour, kept or dropped whole (M4's granularity).

The proof took three forms, and the first two were wrong for reasons worth keeping:

| Form | Rule | What it missed |
| --- | --- | --- |
| Vertices (D-RB-9) | Every vertex at least `distance − slack` from every original boundary | The interior of an arc. A lens-shaped piece at a crossing stage has its two crossings as its *only* vertices, both exactly the requested distance away, while the arc between them bulges into the material. |
| Arc-end stretches with a budget (D-RB-10) | Since the distance to a set changes by at most the distance travelled, split each stretch into sub-pieces whose interior stays within `s` of an endpoint and require every endpoint to clear `distance − slack + s` | The budget it first proposed was the stretch's **sagitta**. The farthest interior point of a stretch is about half a chord — hundreds of times its sagitta — so a sagitta budget does not pay for the step it was meant to justify. The budget is the stretch's **arc length**, which bounds the travel directly and is strictly finer. Straight pieces were still unchecked. |
| Exact segment-to-segment (D-RB-11) | The minimum distance between the piece and every original segment, computed in closed form and compared once | Nothing: the minimum over the piece is attained at a measured point, so the comparison is a statement about every point of the piece. There is no resolution knob left. |

**The two measurements that exposed the first two forms**, both recorded because neither was visible
from reading the rule:

- **6 mm on an arc.** In the 4 mm fixture (a 100 mm square with a centred r10 island) the crossing
  stage is `d = 23`: the wall's inset is the 23…77 square, the island grows to r33, and the two
  cross. Each lens between them is bounded by an arc of the grown island and has its only vertices at
  the crossings, exactly 23 mm from both originals — while the arc's apex comes down to `y = 17`,
  **6 mm inside** the stage's distance (L9, D-RB-10).
- **17 mm on a chord.** K5's fixture showed the same defect class survives on a straight piece: the
  chord between a lens's two crossings runs along `y = 23`, so it is **17 mm** from the island, while
  **both of its ends are 23 mm** away (L10, D-RB-11). Subdividing straight pieces to a sub-micron step
  would have been sampling dressed up as a proof.

### The closed-form minimum, and why its candidate set is complete

`segment_to_segment(first, second, tolerance)` answers one question exactly, for every pair the kernel
has. The argument is two lines long: the minimum of the distance over a pair of curves is attained
either at an **endpoint** of one of them, or at a point **interior to both** where the connecting
vector is perpendicular to both tangents. That second case collapses to a small finite set per pair
type:

| Pair | Interior candidates | Why that is all of them |
| --- | --- | --- |
| line / line | none beyond the four endpoint distances, plus `0` when they cross | Perpendicular to both straight tangents means the directions are parallel; between parallel segments the minimum is attained over an interval of the overlap, so an endpoint of one segment is always a minimum. |
| line / arc | `C ± r·n`, the two points where the line's normal through the circle's centre leaves the circle, kept when they lie on the arc's sweep | Perpendicular to the line makes the connecting vector parallel to its normal; perpendicular to the arc makes it radial. Zero covers a crossing. |
| arc / arc | `C1 ± r1·u` against `C2 ± r2·u`, where `u` is the unit centre line, kept when both points lie on their own sweeps | Radial for both curves puts the connecting vector on the centre line. Concentric arcs have no centre line direction and need none: an endpoint of one inside the other's sweep already returns `\|r1 − r2\|`. Zero covers a crossing. |

Every candidate is evaluated as a genuine distance **to the segment**, never to its supporting line or
circle, so a candidate whose foot falls past an end is still correct — it simply is not the minimum.
The proof calls it with tolerance `0.0` (unsnapped) for every `(piece segment, original segment)` pair,
which is why there is no check-point count, no step constant and no skip bound left anywhere in
`region_offset.rs`.

## The pocket contract

### In the document

A pocket toolpath carries `island_ids` beside `entity_ids`:

```json
{ "id": "TP1", "name": "Pocket 1", "entity_ids": ["E1"], "island_ids": ["E2"],
  "tool_number": 1,
  "operation": { "kind": "pocket", "strategy": "offset", "allowance": 0.0,
    "stepover": 3.0, "pass_depth": 6.0, "start_depth": 0.0, "cut_depth": 6.0 } }
```

The field is omitted when empty, so a document with no islands is byte-identical to one written before
islands existed. `Document::validate` refuses, by name and with the ids:

| Refusal | Message |
| --- | --- |
| An island that is not a closed contour | `toolpath "TP1" island "E2" is not a closed contour` |
| An island not strictly inside one of the toolpath's boundaries | `toolpath "TP1" island "E2" is not inside any of its boundaries` |
| Two islands that are not disjoint | `toolpath "TP1" islands "E2" and "E3" overlap or touch` |
| The same island named twice | `duplicate island id "E2"` |
| Islands on an operation that does not clear a region | `toolpath "TP1" takes islands only for a pocket or an inlay` |

The field is **not** pocket-only: an `inlay` male carries its region as `island_ids` and has no
boundary entity at all, because its boundary is the job rectangle and the job is not an entity. That is
also how the document says which half of the pair a toolpath is — see
[m6-toolpaths.md](m6-toolpaths.md#the-inlay-pair). The refusal above names both operations for that
reason.

"Inside" is the exact containment question, not a bounds test, and it is asked island-first because
`relation` is directional: a pocket's boundary *holds* its island, so the answer that means "valid" is
`FirstInside` with the island as the first argument. A contour sitting in a C-shaped pocket's notch is
inside the pocket's bounds and outside its material, and is refused.

### On the command line

`--islands <ids>` is a comma-separated entity list on `cam-cli toolpath --kind pocket`. A refusal exits
`2` and leaves the document byte-identical, because the document is written only after the whole
command succeeded:

```console
$ cam-cli toolpath part.c2d --kind pocket --entities E1 --islands E9 --tool 1 --stepover 3
error: toolpath "TP1" island "E9" is not inside any of its boundaries
$ echo $?
2
$ cam-cli toolpath part.c2d --kind profile --entities E1 --islands E2 --tool 1
error: --islands is only valid with --kind pocket, not "profile"
$ echo $?
2
```

A successful append says so on its `added` line, and `cam-cli inspect` appends the count — the
island-free line is exactly M4's.

### What the offset mode produces

The pocket builds one `Region` from its boundary and its islands and asks the kernel for
`Region::offset(inset + k·stepover)`, with `inset = diameter/2 + allowance`. The kernel picks each
boundary's sign from the parity of its **nesting depth**: material lies inside an even depth, so the
wall moves inward, and material lies outside an odd depth, so an island boundary moves **outward**. One
call therefore produces rings that follow the wall inward **and** the island outward, and a stage that
comes back as several contours is cut as several rings — that is material the tool can still reach, not
an error.

The strategy owns no distance proof of its own (D-RB-6): the rule lives in the kernel and nowhere else,
so a later consumer cannot drift from it. `Pocketpath::flattened` carries the kernel's flag up to the
caller. A refusal on the very first ring means the tool never fitted and is `OffsetCollapsed` carrying
the kernel's reason; a stage that leaves nothing after that simply ends the ringing.

### What the raster mode produces

The inset region comes from the same kernel offset, so the region covers wall plus islands. Rows sit on
the grid `region.min.y + k·stepover` while they fit `region.max.y`, and each row is clipped by
`Region::raster_spans`: the crossings of a long horizontal line with **every** boundary are sorted,
deduped within tolerance and paired even-odd. A row that crosses an island is therefore cut as **two
spans**, never engraved straight through the island.

`raster_spans` answers `None` for an **ambiguous** row — a leftover odd crossing (a tangency) or a row
lying along a straight boundary (a coincidence) — and an empty list when the row misses the material.
The strategy retries an ambiguous row once, nudged `1e-5 mm` inside, and drops it if the retry is
ambiguous too. The retried row is emitted **at the height the kernel actually clipped at**, not at the
nominal grid height: emitting the nominal one moves a tangent row's ends off the curve by about
1e-5 mm, and M4's tangent test pins 1e-6.

### The verified arithmetic

A 100 mm square with a centred r10 island, a 6 mm tool (`inset = 3 mm`) and an **8 mm stepover**:
stage `k` sits at `d = 3 + 8k`, and a stage has material while the grown island (`r = 10 + d`) is still
strictly inside the shrunken wall (half-width `50 − d`), i.e. while `d < 20`. That is `d = 3, 11, 19` —
three stages, two rings each, **6 rings** — and at `d = 27` the grown island's `r = 37` has swallowed
the pocket (the shrunken wall's corner is `23·√2 = 32.5` from the centre), so the kernel leaves no
material and the ringing ends there.

The same pocket at a **4 mm stepover** gives **14 rings**: five two-ring stages at `d = 3, 7, 11, 15,
19`, plus the crossing stage at `d = 23` where the grown island covers every edge of the shrunken wall
but not its four corners. The measured closest approach over the emitted moves, arc interiors
included, is **3.0000 mm** — the first stage's inset, which is the closest any ring may come. See
[worked examples](#worked-examples).

At a **3 mm stepover** the raster mode gives **32 rows and 40 spans**: 32 rows on the island-free grid,
the 8 rows crossing the island cut as two spans each, and the other 24 as one span `3…97`. The row at
`y = 63` is exactly tangent to the grown island, so the kernel answers `None` and the retry resolves it
just outside. The arithmetic and its assertions are in
`crates/cam-toolpath/tests/pocket_islands.rs` and `crates/cam-toolpath/tests/pocket_raster_islands.rs`;
the same stage counts are visible without a tool in `crates/cam-geom/tests/distance.rs`.

## The schema: `c2d/document@2`

`@2` added exactly one field, `Toolpath.island_ids`, and nothing else changed. The bump exists because
keeping `@1` would have been the worst available behaviour: an older build ignores a field it does not
know, so it would pocket straight **through** an island the user asked to keep, and a ruined part is
far worse than a refusal. A10 records this as D-M4-1's revisit trigger; the field is not inferred from
anything the older build already reads, so no reader can be left guessing what it meant.

| Reader | Behaviour |
| --- | --- |
| `c2d/document@1` | Loads. `island_ids` is absent and therefore empty, so every toolpath is what it was. Saving writes `@2` and the toolpaths are byte-identical. |
| `c2d/document@2` | Loads. |
| Anything else | Refused with exit `3`, naming the found and the supported tag. |

The migration is an identity, not a transformation: `@1` only lacks the field. The contract rows are in
[format.md](format.md#document-c2d).

## Worked examples

### Offset pocket: 100 mm square, r10 island, 6 mm tool, 4 mm stepover

`inset = 3 mm`, so stage `d = 3 + 4k`, and each stage offsets the boundary inward by `d` and the island
outward by `d`:

| Stage `d` | The region at that stage | Pieces cut |
| --- | --- | --- |
| 3 mm | 94 mm square, island ring `r = 13` | 2 |
| 7 mm | 86 mm square, island ring `r = 17` | 2 |
| 11 mm | 78 mm square, island ring `r = 21` | 2 |
| 15 mm | 70 mm square, island ring `r = 25` | 2 |
| 19 mm | 62 mm square, island ring `r = 29` — the last stage where the two are still separate | 2 |
| 23 mm | island ring `r = 33` has crossed the 23…77 wall: four corner slivers survive (a corner is `27·√2 = 38.18` from the centre, outside the disc), and the lens-shaped pieces where the disc stands outside the wall are dropped by the distance proof | 4 |
| 27 mm | nothing is 27 mm from both originals — the wall's own inset is nearer at 23 mm — so the kernel refuses and the ringing ends | — |

**14 rings**, and the island ring is two arcs where the wall ring is four straight moves. That is why
the island's contribution shows up in the program as arc blocks rather than as extra lines.

### Raster pocket: the same square and island, 3 mm stepover

The inset region is `3…97` on both axes and the grown island is `r = 13`, so the rows are the grid
`y = 3 + 3k` that fits — 32 rows, the island-free row count, because an island inside the boundary
cannot move the boundary:

| Rows | `y` | Spans |
| --- | --- | --- |
| 23 rows | everywhere except `39 … 60` and `63` | one span `x = 3 … 97` |
| 8 rows | `39, 42, … 60` (`\|y − 50\| < 13`) | two spans ending and resuming at `x = 50 ± √(169 − (y − 50)²)` |
| 1 row | `63` (`\|63 − 50\| = 13`, exactly tangent) | resolved by the `1e-5 mm` retry, still one span |

40 spans in total, each a straight move, and every cutting move stays at least `3 − 1e-6 mm` from the
island.

### Offset pocket: the same square and island at an 8 mm stepover

`d = 3, 11, 19` are the only stages where the grown island (`r = 13, 21, 29`) is still strictly inside
the shrunken wall (94, 78, 62 mm squares), so the pocket is **6 rings** — three stages of wall ring
plus island ring — and `d = 27` is where the grown island has swallowed the pocket. A stage that
leaves nothing is the collapse, not an error; only the *first* stage leaving nothing means the tool
never fitted.

## Known limits, each a decision

- **No union operation** (D-RB-8). Two overlapping boundaries describe their symmetrical difference,
  not their union: 75 + 75 mm² for two overlapping squares whose union would be 175 mm². A union needs
  boundary orientation to decide which side is inside (which D-RB-2 refuses) and would be its own
  operation. It is not needed by a pocket, because validation refuses an island that is not strictly
  inside its boundary and refuses islands that overlap each other.
- **A boundary that crosses itself is passed through** (L7). A4 asks normalization to resolve crossings
  *between* boundaries; a self-crossing contour arrives and leaves as it is, unresolved rather than
  guessed at. The parity rule still answers for every point of it, and its orientation is never
  consulted, so the region remains usable — the arrangement simply has not been asked to fix it.
- **An exact tangency in the arrangement is refused, not resolved** (recorded by K3, surfaced by
  `Region::offset`). At exactly 20 mm in the 100 mm square plus r10 island fixture the grown island
  (`r = 30`) is tangent to all four wall insets at 30 mm from the wall, and the kernel refuses that
  degeneracy by name instead of deciding which way round it goes. The kernel's own property test over
  0.5–23 mm assumes the call can refuse. An arrangement finer than the tolerance can resolve, and a
  piece that begins and ends at the same point, are refused the same way.
- **`intersect` at tolerance `0.0` can miss a real crossing.** Its circle-membership test compares
  distances to the radius with the tolerance, so at `0.0` its own floating-point noise misses a
  crossing the exact algebra finds (a quarter arc against a line that really does pass through it).
  That is `intersect`'s limit, not the distance's: the proof therefore asks `segment_to_segment(…, 0.0)`
  deliberately, and the test that cross-checks the two uses the kernel's nanometre rather than `0.0`.
- **Exactness of straight pieces and of arcs is settled** (D-RB-11). Both are now proved exactly, which
  is why this feature has no resolution knob at all. It is listed here only because two earlier
  documented forms — vertices only, and arc stretches on a budget — were unsound, and a reader of the
  logs will find them.
- **The drop granularity is a piece** (D-RB-9). A piece that fails the distance proof is dropped whole,
  so what it covered stays uncut: safe, and conservative. Finer trimming would keep more material and
  is a later refinement, not a defect.
- **No UI** (D-RB-7). The authoring surface is `cam-cli toolpath`, the same boundary M4 drew (D-M4-8).
  The shell keeps its M2 authoring surface and gains nothing here.
