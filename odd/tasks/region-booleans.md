# Feature: region booleans and pocket islands

Program plan: `odd/roadmap.md`. M4's decision D-M4-2 left pockets island-free "until the kernel has
boolean subtraction". This document is the executable specification for that kernel work and for the
pocket islands that consume it. Read this file until `## Log` before implementing any task.
Stack: Rust workspace; new kernel modules in `cam-geom`, then `cam-model`, `cam-toolpath`, `cam-cli`.
Branch: `feat/region-booleans`, cut from `feat/m4-toolpaths` at `f1673e8`.

## Specs

Requirements as stated by the owner (verbatim):

- **S1** — Owner's choice of scope (verbatim option label and description, this session): "Sólo islas:
  booleanas en cam-geom (grande)" — "Documento de feature propio: sustracción booleana sobre arcos +
  pockets con islas. Trabajo de kernel, multi-sesión, sirve también a M7/M9."
- **S2** — The roadmap's `cam-geom` description (verbatim, `odd/roadmap.md:66-68`): "2D kernel: Point2,
  LineSeg, Arc (center/radius/direction), Polyline with bulges, Bezier; chaining, winding,
  self-intersection, validation, bounding volumes, spatial index (rstar), offsetting (arc-preserving),
  boolean ops, tessellation".
- **S3** — Decision D-M4-2 (verbatim, `odd/tasks/m4-toolpaths.md`): "A pocket is one region per
  selected contour; islands are out of scope."
- **S4** — Decision D6 (verbatim, `odd/roadmap.md`): "Arcs first-class, no pre-tessellation".
- **S5** — Rule 2 of the architecture (verbatim, `odd/roadmap.md:98`): "**Determinism is a feature.**
  Identical input must produce byte-identical G-code. No `HashMap` iteration may reach an output path;
  ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S6** — Rule 1 of the architecture (verbatim, `odd/roadmap.md:95`): "**`cam-cli` is the contract.**"
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
  closed contours (`cam_geom::chain`). The result has no crossings between boundaries, so the even-odd
  rule and the boundary orientation agree. Input with no crossings is returned unchanged and arc-exact.
- **A5** — `Region::subtract(hole)` appends the boundary and normalizes (A3, A4), so subtracting an
  island from a pocket is one call and the result stays a `Region`, not a special "pocket shape".
- **A6** — `Region::offset(distance)` moves each boundary the way that grows the material: outer
  boundaries inward, holes outward (their `offset_contour` sign is the opposite), then normalizes and
  drops every piece that is closer than `|distance|` to any **original** boundary. The distance proof is
  D-M4-9's rule generalised from one contour to a region, and it is what makes the shrinking terminate.
- **A7** — `Region::raster_spans(y, tolerance) -> Vec<(f64, f64)>` returns the x spans of material at
  height `y`, computed by intersecting the row against every boundary and pairing crossings even-odd —
  the same rule M4's raster pocket already implements against one contour, now against a region.
- **A8** — Everything above preserves arcs wherever the input had them (S4). A piece of an arc is an
  arc; flattening happens only where `offset_contour` already flattens (its self-intersection removal),
  and when it happens the caller is told, the way `Offset::flattened` tells it today.
- **A9** — `cam_model::Toolpath` gains `island_ids: Vec<String>`, omitted when empty, and
  `Document::validate` refuses an island that is not a closed contour, that is not strictly inside the
  pocket's boundary, that crosses another island, or that is repeated. Islands are **explicit**: the
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
  island (a boss), byte-identical across runs and across a save/load round trip.
- **A15** — Every earlier milestone is unchanged: gates green, `emit` output for island-free documents
  byte-identical, and an existing `@1` document loads and cuts exactly as before.

### Decisions recorded for this feature

- **D-RB-1 — Islands are explicit in the document, never inferred.** the reference CAM application's observable rule for
  nested selections is not recoverable from static evidence (the same limit D-M4-5 recorded for
  drilling), and inferring "the inner contour must be a hole" would silently reverse a user's intent
  on a shape that is merely drawn inside another. The user names the islands.
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
- **D-RB-7 — No UI in this feature.** Same boundary as D-M4-8: the authoring surface is
  `cam-cli toolpath`. The shell keeps its M2 authoring surface and gains nothing here.

## Tasks

One line per task: ID — acceptance criteria — route — commit.

- **K1** — A1 — inline — `feat(geom): exact winding number and contour containment`
- **K2** — A2 — inline — `feat(geom): contour relation by intersection and interior point`
- **K3** — A3, A4, A5, A8 — writer — `feat(geom): region model, normalization and subtraction`
- **K4** — A6, A7 — writer — `feat(geom): region offset with the distance proof, and raster spans`
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
gains the `island_ids` field, the `@2` row and the `--islands` CLI row. `odd/roadmap.md` records the
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
| K3 | _pending_ | |
| K4 | _pending_ | |
| P1 | _pending_ | |
| P2 | _pending_ | |
| P3 | _pending_ | |
| P4 | _pending_ | |
| P5 | _pending_ | |
| P6 | _pending_ | |

## Log

- **L1** — Owner decision (verbatim, this session): "Sólo islas: booleanas en cam-geom (grande)". Read
  as: build region boolean subtraction in the kernel and then pocket islands, as its own feature,
  multi-session, chosen over the cheaper raster-angle option in M4's limit list.
- **L2** — Plan: 10 tasks over four slices, branch `feat/region-booleans` from `f1673e8`. Baseline
  recorded before the first write: `./scripts/check.sh` green, 610 tests.
- **L3** — Kernel inventory taken before planning, so the tasks name what exists and what does not:
  `intersect` (line/arc with tangency and coincidence), `trim` (splits one contour against another),
  `chain` (chains segments into contours), `offset_contour` (arc-preserving, refuses some collapses),
  `Contour::{signed_area, bounds, reverse, tessellate}`, `Segment::{point_at, distance_to_point}`.
  Missing: parameters from intersections, point-in-contour, region model, arrangement, region offset.
  That is why K1 and K2 exist before any boolean.
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
