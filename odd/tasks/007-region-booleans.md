# Feature: region booleans and pocket islands

Program plan: `odd/roadmap/index.md`. M4's decision D-M4-2 left pockets island-free "until the kernel has
boolean subtraction". This document is the executable specification for that kernel work and for the
pocket islands that consume it. Read this file until `## Log` before implementing any task.
Stack: Rust workspace; new kernel modules in `cam-geom`, then `cam-model`, `cam-toolpath`, `cam-cli`.
Branch: `feat/region-booleans`, cut from `feat/m4-toolpaths` at `9ad3f8b`.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — Owner's choice of scope (verbatim option label and description, this session): "Sólo islas:
  booleanas en cam-geom (grande)" — "Documento de feature propio: sustracción booleana sobre arcos +
  pockets con islas. Trabajo de kernel, multi-sesión, sirve también a M7/M9."
- **S2** — The roadmap's `cam-geom` description (verbatim, `odd/roadmap/index.md:66-68`): "2D kernel: Point2,
  LineSeg, Arc (center/radius/direction), Polyline with bulges, Bezier; chaining, winding,
  self-intersection, validation, bounding volumes, spatial index (rstar), offsetting (arc-preserving),
  boolean ops, tessellation".
- **S3** — Decision D-M4-2 (verbatim, `odd/tasks/005-m4-toolpaths.md`): "A pocket is one region per
  selected contour; islands are out of scope."
- **S4** — Decision D6 (verbatim, `odd/roadmap/index.md`): "Arcs first-class, no pre-tessellation".
- **S5** — Rule 2 of the architecture (verbatim, `odd/roadmap/index.md:98`): "**Determinism is a feature.**
  Identical input must produce byte-identical G-code. No `HashMap` iteration may reach an output path;
  ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S6** — Rule 1 of the architecture (verbatim, `odd/roadmap/index.md:95`): "**`cam-cli` is the contract.**"
- **S7** — Decision D-M4-1 (verbatim, from the M4 feature document): "New `Operation` variants stay
  inside `c2d/document@1`… Revisit trigger: any change to a field's meaning or units, which *is* a
  `@2`."
- **S8** — Decision D2 (user, verbatim option label): "Todo menos 3D y Laser".
- **S9** — Downstream consumers the owner named for this work (verbatim, S1): "sirve también a M7/M9" —
  V-Carve's medial axis and nesting's no-fit-polygon both need region subtraction.

Everything else here is a **derived acceptance criterion** — an engineering consequence of S1–S9 with
no new scope.

### Derived acceptance criteria

- **A1** — `cam_geom::winding::winding_number(contour, point)` is exact for arc-bearing contours: no
  tessellation, no sampling. It returns 0 for a point outside, ±1 for a point inside a simple contour,
  and is undefined-free at the contour itself (a point on a boundary is reported as *on* it, separately
  — `contains` must never answer "inside" and "on the edge" the same way for two contours that share
  an edge).
- **A2** — `cam_geom::winding::relation(first, second, tolerance)` classifies two closed contours as
  `Disjoint`, `FirstInside`, `SecondInside`, `Crossing` or `Touching`. It is decided by segment
  intersections plus one interior point, never by bounding boxes alone, and it is the only place that
  decides containment. Corrected while implementing (L5): an intersection reported as a single point
  counts as a crossing only when it lies **away from both segments' ends**; a `Point` at an endpoint is
  a shared corner and therefore `Touching`, not `Crossing`, or two squares meeting at one corner would
  look like overlapping material.
- **A3** — `Region { boundaries: Vec<Contour> }` with the **even-odd** membership rule: a point is
  material when it is inside an odd number of boundaries. `Region::contains(point)` is therefore
  `winding_number`s summed mod 2, and a hole is expressed by appending its boundary — no subtraction
  needed to *represent* it.
- **A4** — `Region::normalize()` resolves every crossing between its boundaries by splitting them at
  the intersections, discarding pieces on the wrong side of the parity, and chaining what is left into
  closed contours. The result has no crossings between boundaries, so the even-odd rule and the boundary
  orientation agree. Input with no crossings is returned unchanged and arc-exact. Two corrections made
  while implementing (L7): the chaining is **not** `cam_geom::chain` — `chain` matches endpoints
  greedily and every crossing is a four-way vertex where the greedy match rejoins a piece to its own
  boundary's continuation and returns the input, crossings included — so the pieces are walked with an
  angular continuation rule that turns at a crossing instead of passing through it; and normalizing is
  **parity-preserving, not a union** (D-RB-8).
- **A5** — `Region::subtract(hole)` appends the boundary and normalizes (A3, A4), so subtracting an
  island from a pocket is one call and the result stays a `Region`, not a special "pocket shape".
- **A6** — `Region::offset(distance)` moves each boundary the way that grows the material: outer
  boundaries inward, holes outward (their `offset_contour` sign is the opposite), then normalizes and
  drops every piece whose exact distance to any **original** boundary is under `distance − 0.002 mm`
  (the slack is two chord tolerances of the offset kernel, the form M4 proved necessary). The distance proof is
  D-M4-9's rule generalised from one contour to a region, and it is what makes the shrinking terminate.
  Settled while implementing (L8, D-RB-9): the sign comes from the **parity of the boundary's nesting
  depth**, and "any original boundary" is literal — a 100 mm square offset by 55 mm does not fail in
  `offset_contour` at all, it comes back as a well-formed 10 mm ring whose vertices are 45 mm from two of
  the walls, so measuring the proof against the piece's own boundary keeps a ring that gouges the far
  wall.
- **A7** — `Region::raster_spans(y, tolerance) -> Vec<(f64, f64)>` returns the x spans of material at
  height `y`, computed by intersecting the row against every boundary and pairing crossings even-odd —
  the same rule M4's raster pocket already implements against one contour, now against a region.
- **A8** — Everything above preserves arcs wherever the input had them (S4). A piece of an arc is an
  arc; flattening happens only where `offset_contour` already flattens (its self-intersection removal),
  and when it happens the caller is told, the way `Offset::flattened` tells it today.
- **A9** — `cam_model::Toolpath` gains `island_ids: Vec<String>`, omitted when empty, and
  `Document::validate` refuses an island that is not a closed contour, that is not strictly inside the
  pocket's boundary, that crosses **or touches** another island, or that is repeated. Nesting is legal:
  an island strictly inside another is a boss, which D-RB-2 and A14 both anticipate, and P1's first
  version refused it (L12). Islands are **explicit**: the
  model never infers containment from the selection (D-RB-1).
- **A10** — Because an older build would silently ignore `island_ids` and cut the island away, the
  document schema is bumped to **`c2d/document@2`** when this lands: the writer emits `@2`, the reader
  accepts `@1` (identity) and `@2`, and refuses anything else naming both. This is exactly D-M4-1's
  revisit trigger (S7).
- **A11** — `cam-cli toolpath --kind pocket --islands E3,E4` records the islands, and every refusal
  exits `2` leaving the document byte-identical; `inspect` names the island count.
- **A12** — Offset pocket over a region with islands cuts rings that follow the outer boundary inward
  **and** the island boundaries outward, and no ring passes through an island or leaves material
  against it beyond the allowance.
- **A13** — Raster pocket over a region with islands clips each row against the outer boundary and the
  islands through A7, so a row that crosses an island is cut as two spans and never as one.
- **A14** — Golden G-code for a pocket with an island (offset and raster) and for an island nested in an
  island (a boss), byte-identical across runs and across a save/load round trip. Clarified while
  implementing (L13): under parity a nested island is **material again and is cut**, so the boss disk is
  removed and what stands is the annulus between it and the hole. "Boss" is the shape's name, not a
  promise that it survives, and the golden states the arithmetic that proves it (the boss ring shrinks to
  `r9` and `r2`, i.e. it is cut inside its own boundary; a protected boss would ring at `r15`/`r18`).
- **A15** — Every earlier milestone is unchanged: gates green, `emit` output for island-free documents
  byte-identical, and an existing `@1` document loads and cuts exactly as before.

### Decisions recorded for this feature

- **D-RB-1 — Islands are explicit in the document, never inferred.** Whether a contour drawn inside
  another is an island or separate material is a convention of the toolpath, not a fact of the geometry
  (the same limit D-M4-5 recorded for drilling), and inferring "the inner contour must be a hole" would
  silently reverse a user's intent on a shape that is merely drawn inside another. The user names the
  islands.
- **D-RB-2 — Even-odd is the membership rule, not boundary orientation.** Orientation comes out of our
  own contour builders inconsistently enough (a reversed import, a mirrored contour) that basing
  material on winding polarity would make correctness depend on how a file happened to be written.
  Parity across boundaries is a property of the geometry the user selected, and it also expresses a
  boss inside an island with no extra concept.
- **D-RB-3 — Hand-written arrangement, no clipping dependency.** Splitting, classifying and chaining
  are the same primitives the kernel already has (`intersect`, `chain`, `offset_contour`); adding a
  polygon clipper would either flatten arcs (S4) or bring a second geometry model. Consistent with
  D-M3-1 and D7.
- **D-RB-4 — `Region` is a boundary list, not a tree.** "Outer plus holes plus bosses" as a nesting
  tree needs a normalization step before it can mean anything, and the pockets we cut are planar: a
  flat boundary list with parity (D-RB-2) is the whole model, and `normalize` is what keeps it honest.
- **D-RB-5 — `normalize` may flatten, and must say so.** When two boundaries cross, the surviving
  pieces are trimmed at the crossings, and a trimmed arc stays an arc (A8); what flattens is the same
  case `offset_contour` already flattens. The result carries `flattened: bool` per contour, so the
  strategy can report it instead of hiding it.
- **D-RB-6 — The offset distance proof lives in one place.** A6 reuses the rule M4 put in
  `pocket.rs::clears_boundary` (each vertex at least `distance − slack` from the original boundary),
  moved into the kernel so the pocket and any later consumer cannot drift apart.
- **D-RB-8 — Normalization preserves parity; it is not a union.** Overlapping boundaries describe their
  **symmetrical difference**, because `Region::contains` is parity (D-RB-2) and `subtract` works by
  appending a boundary (A5). A union would need boundary orientation to decide which side is inside,
  which is exactly what D-RB-2 refuses, and it would be a new operation rather than a normalization.
  This does not weaken the pocket: an island that overlaps its boundary, and two islands that overlap,
  are refused by validation (A9), so a valid pocket never needs a union — it needs subtraction, which
  parity gives for free.
- **D-RB-9 — The offset distance proof is measured against every original boundary, and it is the only
  thing that ends a shrink.** `offset_contour` refuses an offset that crosses the shape's *interior*, but
  an over-shrink of a square by more than half its width comes back as a well-formed smaller ring whose
  vertices sit the requested distance from two walls and much less from the other two. The proof
  therefore tests each surviving piece against all original boundaries, not against the one it came
  from, and a piece that fails is dropped whole (M4's per-ring granularity). Refusing rather than
  trimming at the vertex level is the recorded limit: finer trimming would keep more material and is a
  later refinement.
- **D-RB-10 — The distance proof is sound for the whole piece, not for its ends.** The vertices-only proof
  D-RB-9 described is unsound for an arc: a lens-shaped piece at a crossing stage has its two crossings as
  its only vertices, both exactly the requested distance away, while the arc between them bulges inside the
  material. Found by P3, which produced two such pieces 6 mm too close to the wall (L9). The fix is a
  Lipschitz argument rather than sampling: the distance to a set changes by at most the distance travelled,
  so if a piece is cut into sub-pieces whose interior stays within `s` of an endpoint and every endpoint
  clears `distance - slack + s`, then every interior point clears `distance - slack`. Corrected while
  implementing (L10): the budget is the sub-piece's **arc length**, not its sagitta — the farthest interior
  point of a stretch is about half a chord, several hundred times its sagitta, so a sagitta budget does not
  justify the step it was meant to pay for, while a length bound bounds the travel directly and is
  strictly finer. A closed-form lower bound per (arc, original segment) skips the check points an arc does
  not need, so the proof costs nothing measurable; the bound may only ever skip work, never keep a piece.
- **D-RB-11 — A straight piece needs the exact distance too.** K5 recorded that the same defect class
  survives on a chord: in its own fixture the chord between two crossings runs 17 mm from a boundary while
  both of its ends are 23 mm away, because the chord is a straight cut across a region that bulges
  between them. Subdividing straight pieces to a sub-micron step would be sampling dressed as a proof, so
  the answer is the exact minimum distance between two segments — line/line, line/arc and arc/arc all have
  closed forms built from a handful of candidates (endpoints against the other curve, the perpendicular
  foot inside the segment, the circle centre line) — and the proof becomes one comparison per (piece,
  original) pair with no resolution knob at all. K6 builds it.
- **D-RB-7 — No UI in this feature.** Same boundary as D-M4-8: the authoring surface is
  `cam-cli toolpath`. The shell keeps its M2 authoring surface and gains nothing here.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **K1** — A1 — inline — `feat(geom): exact winding number and contour containment`
- **K2** — A2 — inline — `feat(geom): contour relation by intersection and interior point`
- **K3** — A3, A4, A5, A8 — writer — `feat(geom): region model, normalization and subtraction`
- **K4** — A6, A7 — writer — `feat(geom): region offset with the distance proof, and raster spans`
- **K5** — A6 correction: the proof must be sound for arcs, not only for vertices — writer —
  `fix(geom): a distance proof that holds for the whole piece, not its ends`
- **K6** — A6 correction: the proof must be sound for straight pieces too, which needs an exact
  segment-to-segment distance rather than a finer subdivision — writer —
  `feat(geom): exact segment-to-segment distance, and a proof that uses it`
- **P1** — A9, A10 — inline — `feat(model): explicit islands and c2d/document@2`
- **P2** — A11 — inline — `feat(cli): islands on the toolpath command`
- **P3** — A12 — writer — `feat(toolpath): offset pocket over a region with islands`
- **P4** — A13 — writer — `feat(toolpath): raster pocket over a region with islands`
- **P5** — A14, A15 — inline — `test(cli): golden g-code for pockets with islands`
- **P6** — docs — inline — `docs(booleans): the region model, and roadmap evidence`

Route notes: K1 and K2 are the primitive every later task calls, and are small enough to be inline.
K3 and K4 are two independent kernel units over disjoint modules (`boolean.rs`/`region.rs` versus
`region/offset.rs` — they touch one shared file at most), which is the writer reason, launched
together once P1 has fixed the model surface. P3 and P4 are two strategies over the same
`pocket.rs`, so they are sequential, not parallel. P1 and P2 are small and inline.

### Task detail

**K1 — Winding.** `cam_geom/src/winding.rs`:

```rust
pub fn winding_number(contour: &Contour, point: Point2) -> i32;
pub enum Membership { Outside, Inside, OnBoundary }
pub fn classify(contour: &Contour, point: Point2, tolerance: f64) -> Membership;
```

Exact for arcs (S4): a line segment contributes the signed angle `atan2(cross, dot)` between the
vectors to its endpoints; an arc contributes the signed angle it subtends at `point`, computed from the
arc's own sweep and the geometry of its circle rather than by sampling. A contour adds nothing when it
is not closed. `classify` answers `OnBoundary` when `point` is within `tolerance` of any segment, so the
caller never has to guess which of two touching contours owns an edge. Tests: a square and a circle
report inside/outside for a grid of points; a point on an edge or on a vertex is `OnBoundary`; a point
just inside/outside within tolerance is still decided by the boundary rule; a clockwise contour reports
`-1` where the counter-clockwise one reports `1`; a non-convex C shape reports inside for a point in the
notch only when it really is inside; a property test compares `classify` against a fine tessellation of
the same contour for random points, which is how an arc-only bug shows up.

**K2 — Relation.** In `winding.rs`:

```rust
pub enum Relation { Disjoint, FirstInside, SecondInside, Crossing, Touching }
pub fn relation(first: &Contour, second: &Contour, tolerance: f64) -> Relation;
```

Decided by `intersect` over every pair of segments (a `Point`/`Points` result is a crossing; a
`Coincident` or `Tangent` is `Touching` unless a proper crossing also exists), then one interior point
— a vertex of one contour offset slightly inward, or its bounds centre tested with `classify` — decides
which contour contains which. Bounding boxes are a fast path to `Disjoint`, never the decision. Tests: a
circle inside a square, a square inside a circle, two overlapping squares, two disjoint squares, a
shared edge (`Touching`), a contour inside another with a hole-like notch, and a property test asserting
the relation is antisymmetric (`FirstInside(a,b)` iff `SecondInside(b,a)`).

**K3 — Region, normalization, subtraction.** `cam_geom/src/region.rs` (public as
`cam_geom::region::{Region, Normalized}`), with the arrangement in `cam_geom/src/boolean.rs` if the file
would otherwise pass a few hundred lines:

```rust
pub struct Region { pub boundaries: Vec<Contour> }
impl Region {
  pub fn new(boundaries: Vec<Contour>) -> Result<Self, GeomError>;  // closed contours only
  pub fn contains(&self, point: Point2, tolerance: f64) -> bool;     // parity, A3
  pub fn subtract(&self, hole: &Contour) -> Result<Self, GeomError>; // A5
  pub fn normalize(&self, tolerance: f64) -> Result<Normalized, GeomError>;
}
pub struct Normalized { pub region: Region, pub flattened: bool }
```

Normalization: split every boundary at every intersection with every other boundary, bounded by
`tolerance` (reusing `intersect` and the splitting that `trim.rs` already performs); classify each piece
by the parity of its midpoint against the *original* region; drop pieces whose parity says they are not
material; chain the survivors with `cam_geom::chain` into closed contours; if a chain cannot be closed
the operation is refused by name, never returned half-built. Determinism (S5): pieces are processed in
boundary order then piece order, and chains are started from the piece with the lowest
`(bounds.min.y, bounds.min.x, index)` so the same input yields the same vertex order every run. Tests: two
overlapping squares normalize to one hexagon-ish contour with arc-free pieces; a square minus a circle
inside it yields two boundaries and `contains` is false in the circle; two contours that touch along an
edge do not produce a doubled edge; a contour minus itself is refused or empty by name; the result is
`flattened: false` when nothing had to be re-made, and `true` when the input required it; two runs give
identical `Vec<Contour>`; and a property test that a normalized region's `contains` agrees with the
original region's on random points.

**K4 — Region offset and raster spans.** In `cam_geom/src/region.rs`:

```rust
impl Region {
  pub fn offset(&self, distance: f64, tolerance: f64) -> Result<RegionOffset, GeomError>;
  pub fn raster_spans(&self, y: f64, tolerance: f64) -> Option<Vec<(f64, f64)>>;
}
pub struct RegionOffset { pub region: Region, pub flattened: bool, pub warnings: Vec<OffsetWarning> }
```

`offset` offsets each boundary by `distance` with the sign that grows material (A6), normalizes, then
drops every piece closer than `|distance| − slack` to any original boundary — the rule M4 proved
necessary (D-M4-9), moved here (D-RB-6). A first ring that collapses entirely is
`GeomError::EditRefused` with a reason, not an empty region. `raster_spans` implements A7 and returns
`None` when the row is ambiguous (a tangency or a coincidence at that exact `y`) so the caller owns the
retry policy it already has. Tests: a square with a circular island offset inward gives rings that
follow both, and their distance from the outer and the island is `|distance|` within 1e-6; the same
region offset inward by more than half its width is refused rather than returning a gouging ring (the M4
defect, now in the kernel); `raster_spans` on a row crossing an island returns two spans whose ends are
the analytic circle x's; a row tangent to the island returns `None`; a region with no islands behaves
exactly as M4's single-contour path did.

**P1 — Model.** `Toolpath.island_ids` (A9), `#[serde(default, skip_serializing_if = "Vec::is_empty")]`,
validation of closedness, strict containment, no crossing between islands and no duplicates. Schema
`c2d/document@2` with the reader accepting `@1` (A10) — `cam-store`'s save/load and `cam-model`'s
`DOCUMENT_SCHEMA` both move, and `docs/format.md` states the migration. Tests: an island round-trips; an
island outside its boundary is refused naming both ids; two islands crossing are refused; a duplicate
island id is refused; a `@1` document loads and re-saves as `@2` with identical toolpaths; a document
with `@3` is still refused naming both versions.

**P2 — CLI.** `--islands <ids>` on `toolpath --kind pocket`, refusing non-pocket kinds, naming a
missing entity, and requiring the island to be contained (the model's own refusal, surfaced as exit `2`
with the file untouched). `inspect` prints `pocket (offset), tool T1, 1 entity(ies), 2 island(s)` when
islands exist and the M4 line unchanged when they do not.

**P3 — Offset pocket with islands.** `pocket.rs` builds a `Region` from the toolpath's boundary and
islands, and rings come from `Region::offset(k * stepover + inset)` instead of `offset_contour`. A
`flattened` ring is reported in `Pocketpath`. Tests: an island-free region reproduces M4's ring count
and geometry exactly (the existing 12-ring test must pass unchanged, A15); a square pocket with a
central circular island has rings that clear both, with no ring vertex inside the island; an island that
leaves no material refuses by name.

**P4 — Raster pocket with islands.** Rows come from `Region::raster_spans`, so a row crossing an island
emits two spans, and the existing row retry stays where it is. Tests: a row crossing a central island
emits exactly two spans with the analytic x's; a pocket whose island covers the entire row emits nothing
for that row; the island-free case is byte-identical to M4's program (A15).

**P5 — Golden.** A pocket with a circular island, offset and raster, asserted whole through the real
post; the same after a save/load round trip; and an island nested in an island (a boss) cut as material
again. Plus the `@1` document from M0-M4 emitted byte-identically (A15).

**P6 — Docs.** `docs/region-booleans.md`: the parity model and why it is not orientation-based, what
normalization does and what it flattens, the distance proof's origin (D-M4-9), the islands' document
contract and the `@2` migration, and the pocket's behaviour with islands in both modes. `docs/format.md`
gains the `island_ids` field, the `@2` row and the `--islands` CLI row. `odd/roadmap/index.md` records the
kernel capability and removes D-M4-2's restriction from M4's known limits, pointing at this feature.

## Delivery slices

| Slice | Tasks | Subject |
| --- | --- | --- |
| 1 | K1, K2 | The primitives: winding, containment relation |
| 2 | K3, K4 | The region: normalization, subtraction, offset, raster spans |
| 3 | P1, P2 | Model islands and the `@2` migration; CLI surface |
| 4 | P3, P4, P5, P6 | The two pocket modes with islands, golden G-code, docs |

## Evidence — commit per task

| Task | Commit | Notes |
| --- | --- | --- |
| K1 | this commit | 9 tests (`crates/cam-geom/tests/winding.rs`). Analytic winding with arcs, `Membership::{Outside, OnBoundary, Inside}`, tolerance-driven boundary test, open contours enclose nothing, and a property test against a fine tessellation over three shapes (a circle, a four-bulge shape, a three-arc shape) |
| K2 | this commit | 11 tests (`crates/cam-geom/tests/relation.rs`): circle in square and the inverse, arc in arc, overlapping squares, plate crossing a circle, disjoint squares, shared edge, internal tangency, identical contours, the C notch (bounds contain, geometry does not), the tolerance changing "meeting" into "apart", and a property test for antisymmetry |
| K3 | `51c9d2a` | 15 tests. Overlapping squares → 2 contours, 6 vertices, 75 mm² each (parity, D-RB-8); touching squares → 1 contour of 200 mm² whose perimeter is 60 mm, so the shared edge is not doubled; square + edge-crossing circle → 74.867 + 25.133 mm² with every surviving arc at radius 4 and its original centre; a plus sign (4 crossings) → 4 contours of 100 mm²; a bulge the model cannot hold is rebuilt and `flattened` says so; determinism vertex-for-vertex; a property test that a normalized region's `contains` agrees with the original's. One decidability limit is refused by name (an arrangement finer than the tolerance), and a boundary that crosses **itself** is passed through unaltered, because A4 only asks about crossings *between* boundaries (`region.rs` says so where it documents `normalize`; nothing pins either behaviour with a test — L12) |
| K4 | `8c35bef` | 14 tests. A 100 mm square inset by 10 → bounds (10,10)–(90,90), area 6400, every vertex 10.0 mm from its wall. A square with a centred r10 island inset by 8 → outer ring 84 mm wide, island ring r18 (area 1017.876), every vertex clearing both originals. An odd-depth hole grows: area 314.159 → 706.858. Nested parity (square / hole / boss) → 90, 70, 10 mm widths. Over-shrink and wider-than-material both refused where a naive `offset_contour` returns a well-formed gouging ring (the D-M4-9 evidence). A row crossing the island → spans at `50 ± √91`; a tangent row → `None`; a row along a boundary → `None`; `Some(vec![])` when the row misses. A U bracket's narrow arms flatten and say so |
| K6 | `831be66` (+ seed `1ab6396`) | 13 tests. `segment_to_segment` exact for all four pair types from a closed-form candidate set, with the completeness argument stated in the module docs: parallel lines 5 mm apart, staggered spans `√125`, a T-junction at 0, a line reaching 4 mm past a circle's centre giving 6 = r − 4, a tangent line at 0, concentric arcs `|r1 − r2|` = 6 and `√116` when their sweeps do not overlap, side-by-side circles 8 = 20 − 5 − 7, nearest point an endpoint (`√109`, `√101`), and an interior arc foot (2.0, 5.0). The K5 chord now measures 17.0 against ends at 23.0 and is dropped; the 4 mm pocket fixture still gives 14 pieces, and no fixture's count changed — the gain is that the rule no longer depends on a piece happening to carry a betraying arc. Three property tests: symmetry, never exceeding sampled point distances, and zero exactly when `intersect` meets. Recorded pre-existing defect: `intersect` at tolerance `0.0` misses real crossings, so `segment_to_segment` carries its own exact meeting tests |
| P1 | this commit | 9 model tests plus 1 store test. `island_ids` round-trips and is omitted when empty; an island outside the boundary, one in a C-shaped pocket's notch (bounds contain it, material does not), two overlapping islands, a repeated island, an open island and islands on a profile toolpath are each refused by name; a `@1` document loads, keeps its toolpaths byte-identical and saves as `@2`; `@3` is still refused naming both |
| P2 | this commit | 3 binary tests: a pocket records its islands and reloads with them; an island outside the boundary exits `2` with the file byte-identical (the model's refusal surfaced, not a second copy of the rule); `--islands` on a profile exits `2` naming the flag and the operation it belongs to. `inspect` appends `, N island(s)` and the island-free line is unchanged |
| K5 | `41733e9` | 17 cam-geom tests (3 new: the lens case with the old proof reproduced from the public API and its 23 mm ends against a 17 mm apex, plus two property tests that no surviving piece comes closer than its stage distance, arcs and nested regions) and 7 tightened `pocket_islands` tests. The 4 mm fixture now gives the analytic **14** rings instead of 16, and the pocket test measures the closest approach over the emitted moves: 3.0000 mm at the first stage, 23.0000 mm at the crossing stage where the dropped lenses reached 17 mm. The new CLI assertion counts arc blocks (0 without the island, 14 with it) instead of comparing line counts, which was only passing while the gouging pieces were cut |
| P3 | `a417b98` | 8 tests. `PocketRequest::islands: &[&Contour]`; rings from `Region::offset(inset + k*stepover)`, so the wall shrinks and the island grows; the strategy's own proof deleted (D-RB-6); `Pocketpath::flattened` reported. 8 mm stepover → 6 rings (3 stages × wall + island) ending at `d = 27`; 4 mm stepover → stages 3, 7, 11, 15, 19, 23 with the wall ring at `3..97` and the island ring at `r = 13`; a region that leaves nothing and a 3 mm pocket with a 4 mm tool are both `OffsetCollapsed` carrying the kernel's reason; a narrow neck reports `flattened`; two runs are identical. **All 12 M4 pocket tests passed with their expectations untouched**, and a base-vs-change dump over seven island-free fixtures in both strategies is move-for-move identical except the wording of one refusal reason the spec asks for |
| P4 | `62088b4` | 5 tests. 100 mm square + centred r10 island, 3 mm stepover → **32 rows, 40 spans**: the 8 rows at y = 39…60 cross the island and cut two spans ending at `50 ± √(13² − (y−50)²)`, the other 24 cut one span `3…97`, and the row at y = 63 is exactly tangent so the kernel answers `None` and the retry resolves it just outside. A full-width island band → 24 rows / 24 spans (the grown island crosses the shrunken wall and the kernel normalizes the region into two bands, so those rows get no material). A narrow strip at 7 mm stepover → 14 rows / 24 spans with no unpaired span. Every cutting move stays ≥ 3 − 1e-6 mm from the island. **`tests/pocket.rs` needed no edit at all** (12/12) and a differential over eight island-free raster fixtures is 875 identical output lines |
| P5 | `075f8da` | 5 whole-program goldens plus 2 model cases, 707 in the workspace. Offset island (45 lines: three stages, wall squares 3..97 / 11..89 / 19..81, island rings r13/r21/r29 as `G03` half turns, no stage at d = 27 because the kernel proves it gouges), its save/load round trip, raster island (45 lines, 6 rows / 9 spans with gaps at `30 ± √(13² − (y−30)²)`), the `@1` round trip emitting identical bytes, and the nested boss (43 lines, three rings per stage at d = 3 and d = 10, hole growing to r27/r34 and boss shrinking to r9/r2, no stage at d = 17). Fixing the nesting rule was P1's defect: `check_islands` refused every pair that was not `Disjoint`, so the boss golden could not be authored at all |
| P6 | _pending_ | |

## Log

- **L1** — Owner decision (verbatim, this session): "Sólo islas: booleanas en cam-geom (grande)". Read
  as: build region boolean subtraction in the kernel and then pocket islands, as its own feature,
  multi-session, chosen over the cheaper raster-angle option in M4's limit list.
- **L2** — Plan: 10 tasks over four slices, branch `feat/region-booleans` from `9ad3f8b`. Baseline
  recorded before the first write: `./scripts/check.sh` green, 610 tests.
- **L3** — Kernel inventory taken before planning, so the tasks name what exists and what does not:
  `intersect` (line/arc with tangency and coincidence), `trim` (splits one contour against another),
  `chain` (chains segments into contours), `offset_contour` (arc-preserving, refuses some collapses),
  `Contour::{signed_area, bounds, reverse, tessellate}`, `Segment::{point_at, distance_to_point}`.
  Missing: parameters from intersections, point-in-contour, region model, arrangement, region offset.
  That is why K1 and K2 exist before any boolean.
- **L13** — P5 evidence and the last correction: 5 goldens, 2 model cases, 707 in the workspace, gates
  green, merged at `075f8da`. The nested case needed P1's rule fixed rather than tested around, and the
  worker also corrected **my instruction**, which had the parity backwards: I wrote that the boss "is
  material and the pocket leaves it standing", but three boundaries is odd, so the boss disk is material
  **and therefore cut**, while the annulus between boss and hole is inside two and stands. The worker
  matched the program and the spec, refused to encode the inverted sentence, and said so — the golden's
  `r9`/`r2` rings are only consistent with cutting. A14 now says this in the open, because a user reading
  "islands are contours to leave standing" will not expect the innermost one to be removed.
- **L12** — Two corrections after P6 read the feature document against the code. P6's own review found the
  first two in **my** text, and they matter because the document is what a later session trusts: K3's
  evidence row claimed a self-crossing boundary is "refused by name" while `region.rs` documents it as
  passed through (the row is fixed; neither behaviour has a test, which is recorded rather than papered
  over), and A9 said an island that "crosses" another is refused while the code refused *any* pair that
  was not `Disjoint` — so a legitimate **boss inside a hole** could not be authored at all, contradicting
  D-RB-2 and A14. P5 found that one by trying to write the nested golden: `emit` refused the document it
  was supposed to cut. That is P1's defect, mine, and P5 is fixing the rule under a widened surface rather
  than dropping the test. Also fixed here: A6 never stated the 0.002 mm slack the proof actually uses.
- **L11** — P4 evidence: 5 tests, 687 in the workspace, gates green, merged at `62088b4`. The raster
  pocket cuts a region now, which closes the gap that mattered most for shipping: before it, a raster
  pocket that named an island cut the island away. `tests/pocket.rs` needed no edit and a differential
  over eight island-free raster fixtures came back identical line for line. One decision the spec had
  not settled and the worker got right: the retried row is emitted at the height the kernel actually
  clipped at, not the nominal grid height, because emitting the nominal one moves a tangent row's ends
  off the curve by ~1e-5 mm — and M4's tangent test pins 1e-6. Also recorded: "a row the island covers
  entirely" cannot be built from a concentric fixture (such a row becomes two spans); it needs a
  full-width band whose grown island crosses the shrunken wall, and then the *kernel* hands back no
  material — which is the raster path owning no parity logic of its own.
- **L10** — K5 evidence: 17 + 7 tests, 682 in the workspace, gates green. Two corrections of my own
  reasoning, both worth having found. First, D-RB-10 as I wrote it was **unsound**: it budgeted the
  subdivision by the piece's sagitta, but the farthest interior point of an arc stretch is about half a
  chord — hundreds of times its sagitta — so a sagitta budget does not pay for the Lipschitz step. The
  budget is the stretch's arc **length**, which bounds the travel directly and is strictly finer.
  Second, K5 found that the same defect class survives on **straight** pieces: the chord between two
  crossings can cut across a bulge that none of its ends sees (17 mm against ends at 23 mm in K5's own
  fixture). Subdividing straight pieces to a sub-micron step is sampling pretending to be a proof, so
  D-RB-11 records the real answer — the exact segment-to-segment distance, which has a closed form for
  every pair the kernel has — and K6 is the task. The out-of-scope CLI assertion was repaired by the
  parent, and the replacement asserts what the island actually adds (arc blocks: 0 without it, 14 with
  it) instead of a line count that only held while the gouging pieces were still cut.
- **L9** — P3 evidence and the defect it surfaced: 8 tests, 679 in the workspace, gates green, P3 merged
  at `a417b98`. The offset pocket now cuts a region and island-free output is move-for-move M4's. The
  worker also found something the feature has to fix before it can close: at a crossing stage the
  vertices-only distance proof of D-RB-9 is **unsound for arcs** — a lens-shaped piece whose only vertices
  are the two crossings clears the proof while its arc bulges 6 mm into the wall, and two of the 16 pieces
  the 4 mm fixture produced were exactly that. The worker's test asserts a floor (`rings >= 14`) instead of
  pinning the artefact, which is the right call for a task that could not fix the kernel, and K5 exists to
  make the proof sound and tighten that assertion to the analytic 14. Recorded rather than absorbed,
  because a ring that gouges is the one class of bug this project treats as unacceptable.
- **L8** — K4 evidence: 14 tests, 671 in the workspace, gates green. One decision was genuinely missing
  from the spec and the answer is the most valuable thing this task produced: the distance proof has to
  be measured against **every** original boundary (D-RB-9), because the offset kernel happily returns a
  well-formed ring for a shrink wider than the material. Three more recorded: crossing input is refused
  rather than normalized internally, since the proof needs the boundaries to *be* the material boundary
  (D-RB-8 made a crossing list a parity statement, not an outline); `Some(empty)` means the row misses
  the material while `None` means the row is ambiguous, which keeps M4's retry policy where it belongs;
  and the flattening fixture had to become a U bracket because a dumbbell's neck makes `offset_contour`
  refuse with `DegenerateArc` before self-intersection removal runs — a kernel limit, recorded, not
  fixed, because `offset.rs` is outside this feature.
- **L7** — K3 evidence: 15 tests, 657 in the workspace, gates green. Three spec corrections, none of them
  cosmetic. (1) The task text asked for overlapping squares to normalize to "one contour with the
  expected area" and a crossing configuration to reach "the analytic union area" — with even-odd
  membership that is the *symmetrical difference*, so the tests pin 75 mm² L-shapes and 100 mm² for a
  square plus an edge-crossing circle, and D-RB-8 states why a union would be a different operation
  needing orientation. (2) A4 named `cam_geom::chain` for chaining; every crossing is a four-way vertex
  and `chain`'s greedy endpoint match rejoins a piece to its own continuation, handing back the input
  with its crossings — the arrangement is walked with an angular continuation rule instead, and `chain`
  is left untouched. (3) A4 did not say what happens to a boundary that crosses itself: it is passed
  through unresolved rather than guessed at, which is now recorded as a limit. The worker also reported
  that its own RED was weaker than the others' — the module was written before the tests — and that two
  of its tests caught real defects (a chain guard counting tessellated segments, a fixture placing a
  tangent edge exactly on the circle so the parity correctly dropped the arc).
- **L6** — P1 evidence: 639 tests, gates green. Two things the tests caught. The island containment
  check asked `relation(boundary, island)` and compared it to `FirstInside`, but the relation is
  directional and a boundary *holding* an island reports `SecondInside` — every valid island was being
  refused. And adding `island_ids` broke every `Toolpath` literal in the workspace, which is the
  compiler doing its job; the field is added explicitly rather than defaulted through a builder,
  because a wrong toolpath is not something a default should decide.
- **L5** — K2 evidence: 11 tests, 630 in the workspace, gates green. A2's wording was wrong in exactly
  the way the kernel caught: it said any `Point` intersection is a crossing, and a shared corner or a
  shared edge's two ends are `Point` results, which made two touching squares report `Crossing`. A
  meeting point now only counts when it is interior to both segments (measured along the segment, arcs
  included), so shared corners are `Touching`. A tolerance test was added in place of a nested-contour
  test whose premise was wrong (the circle I drew was not inside the square, so the test was asserting
  geometry that did not exist) — the property test for antisymmetry and the C-notch case carry the
  "bounds are not containment" coverage.
- **L4** — K1 evidence. Two defects were found by the tests, both real and both fixed in the root rather
  than in the expectation. First, the arc's contribution was computed by cutting the arc at its
  **tangency** points and taking the short-way angle between each piece's endpoint directions; that is
  wrong for the far side of a circle, whose visual span exceeds half a turn, and it reported the
  complement with the wrong sign. The arc is now cut where it crosses the horizontal line through the
  query point, so each piece lies in one half plane and its angle is exact. Second, and subtler, the
  branch for a piece under that line was chosen from the *sign* of an `atan2` result; a point
  mathematically on the line comes back as `+5.5e-17` about as often as `-0.0`, so a bulge dipping below
  its own chord read as outside. The branch is now decided by the endpoint's distance from the line, and
  the failing case is pinned in `a_bulged_shape_reports_inside_and_outside`. The property test's
  reference ray also had to be nudged off exact vertex levels — that is the sampler's degeneracy, not
  the contour's, and it is documented where it happens.
