# Feature: the reference CAM application-class CAM application — Milestone M7 (V-Carve)

Program plan: `odd/roadmap.md` (M7 = "V-Carve"). This document is the executable specification for M7
only. Read this file until `## Log` before implementing any task.
Stack: the existing Rust workspace (`cam-geom`, `cam-model`, `cam-toolpath`, `cam-move`, `cam-sim`,
`cam-cli`); no new crate and no new dependency. Branch: `feat/m7-vcarve`, cut from `master` at
`9ff0b8c`; baseline `cargo test --workspace` green with **975 tests** before the first write.

## Specs

Requirements as stated by the owner:

- **S1** — This session's instruction, verbatim: "conmenzamos con M7" (spelling as written).
- **S2** — The owner's two scope decisions in this session, by their exact option labels: "Nada extra:
  `simulate` lo reporta como uncut" and "No: M7 es V-carve a punta pura". Together they bound M7: no
  corner-relief pass of any kind, and no Flat Depth / flat-bottom phase. Both are recorded out of
  scope below rather than quietly dropped.
- **S3** — Roadmap M7 row (verbatim, `odd/roadmap.md`): "| **M7** | V-Carve | Depth from medial-axis
  distance to boundary, per V-bit included angle, corner relief | Research-grade geometry (medial axis
  of arcs); deliberately isolated so it cannot delay the rest |". The row's "corner relief" and the
  per-bit angle are addressed by S2 and A1 respectively.
- **S4** — Why this milestone is isolated (verbatim, `odd/roadmap.md` §4.1): "V-Carve computes cut
  depth from the distance between a point and the nearest boundary, scaled by the V-bit's included
  angle, which means the toolpath is not a fixed-Z pass but a depth field. That requires a
  medial-axis/straight-skeleton computation on geometry containing arcs, plus corner-relief handling.
  The original's binary contains the strategy but the algorithm is not recoverable from static
  analysis — this is a genuine research task".
- **S5** — What the README promises is next (verbatim, `README.md`): "V-Carve is next (M7)."
- **S6** — The strategy's own vocabulary, recovered from the executable
  (`evidence/derived/exe_utf16_strings.txt`, line numbers in that file): "V-Carve / Engraving Toolpath"
  (`:1457`), and the parameter dialog's field names (`:3411-3425`): "`_vcpdVersion`", "`_vcpdInMM`",
  "`_vcpdToolpathName`", "`_vcpdStartDepth`", "`_vcpdFlatDepth`", "`_vcpdTolerance`",
  "`_vcpdPreserveArcs`", "`_vcpdDoFlatBottom`", "`_vcpdUseAreClearTool`", "`_vcpdProjectToolpath`",
  "`_vcpdUseSelectionOrder`", "`_vcpdStartDepthFormula`", "`_vcpdFlatDepthFormula`", "`_vcpdDropTool`",
  "`_vcpdUseStartPoint`".
- **S7** — The tolerance is the product's own, and it is a **post-calculation simplification**
  (verbatim, `:865-866`): "VCarve Toolpath Tolerance" / "Tolerance to apply to VCarve toolpaths after
  calculating to reduce file size. This should only be changed if your machine supplier has recommended
  a value". With its defaults (verbatim, `:2862-2864`): "`DefaultInchTolerance3d`",
  "`DefaultInchTolerance2d`", "`DefaultInchToleranceVCarve`". This is the evidence that an
  approximation with a declared tolerance is the product's model and not a compromise invented here.
- **S8** — The product validates the flat bottom, which M7 does not ship (verbatim, `:431`, `:3312-3320`):
  "Flat Depth must be greater than zero if creating flat bottom"; "The carving tool never reached the
  flat bottom depth." / "The flat bottom depth is %s, the carving tool only reached %s" / "Although
  the carving tool reached the flat bottom depth," / "The flat area has been cleared with the carving
  tool." / "No Flat Depth specified with no angled tools on the list".
- **S9** — Rule 1 of the architecture, in force since M0 (verbatim, `odd/roadmap.md` §3): "**`cam-cli`
  is the contract.** Any behaviour that cannot be produced headless through the CLI is not considered
  testable, and therefore not done."
- **S10** — Rule 2 of the architecture, in force since M0 (verbatim, `odd/roadmap.md` §3):
  "**Determinism is a feature.** Identical input must produce byte-identical G-code. No `HashMap`
  iteration may reach an output path; ordering is explicit (`BTreeMap`, sorted IDs, stable sorts)."
- **S11** — Verification rule 5 (verbatim, `odd/roadmap.md` §5): "**Simulation as oracle** — for each
  strategy, a test asserts the simulated removed volume matches the analytic expectation within
  tolerance; a too-shallow pocket fails the test, not a screenshot."
- **S12** — The post can already express a varying-Z linear move, so M7 changes no post contract
  (verbatim, `crates/cam-post/tests/fixtures/post_mm.pp:59-60`): "begin FEED_MOVE" / "`\"[N]G01[X][Y][Z]\"`",
  and `FIRST_FEED_MOVE` is "`\"[N]G01[X][Y][Z][F]\"`" (`:56-57`). This is the difference from M6, whose
  helix needed four new block kinds.
- **S13** — The sweep already interpolates Z along a cutting move, which is what makes a varying-Z move
  measurable (verbatim, `odd/roadmap.md` §7): "**D-M6-12** | The sweep **interpolates Z** along a
  cutting move, so a ramp is one linear move and not a staircase (amends D-M5-3)".
- **S14** — Decision D-M4-6, whose cost M7 does not re-open (verbatim as quoted by M6's S14,
  `odd/tasks/m6-depth-variants.md`): "**D-M4-6 — Linking moves stay conservative.** Between
  rings, rows and holes the tool retracts to `safe_z` and rapids."
- **S15** — Decision D-M4-8, still in force (verbatim, `odd/tasks/m4-toolpaths.md`):
  "**D-M4-8 — No toolpath UI in M4.** ... `cam-cli toolpath` is how a toolpath is authored."
- **S16** — Decision D-M6-11, the boundary M7 inherits (verbatim, `odd/roadmap.md` §4.9): "**D-M6-11**
  leaves the CLI's "a flag no kind uses is silently dropped" open milestone-wide — closing it changes
  the contract for callers that pass a superset today".
- **S17** — The known defect M7 must fix before it can trust its own oracle (verbatim, `odd/roadmap.md`
  §6): "**The simulation's field and the program use different origins** (M6 L17). `cam_sim::grid::grid_for`
  builds the field at `(0, 0)` from `job.width/height` while programs are emitted in machine
  coordinates, so at any origin corner other than bottom-left the report and the preview disagree with
  the document about *where* the cut is — for every strategy since M5". The roadmap's own remedy
  (verbatim): "the fix carries a **test per origin corner**; it moves the grid's origin and the sweep's
  cell mapping in one change, and the job frame is mapped onto the machine frame exactly once, at the
  field's boundary".
- **S18** — Scope boundary D2 (user, verbatim option label): "Todo menos 3D y Laser". M7 ships no 3D
  machining, no laser strategy, and no projection (`_vcpdProjectToolpath`, S6).

- **D-M7-8 — The frame is mapped where a program meets the field, not by moving the grid.** The
  roadmap's §6 sketch proposed moving `grid_for`'s origin and the sweep's cell mapping together. The
  implementation found a cheaper and safer crossing point, and the reason is a tab: M5 places a tab by
  **arc length from the contour's start** (`Walk::walked_at`), and reflecting a contour reverses its
  traversal, so mapping the *target's* geometry into the machine frame would move every tab to the other
  side at `top_left`. So the field, the target and the preview stay in the document frame, and the
  program is read into it once, in `sweep::apply_move`. What the sketch got right is the invariant —
  exactly one mapping, at exactly one boundary — and that is what the code does.

## The claim, stated once

Everything below follows from one fact, which is why it is written before the acceptance criteria.

A V-bit cutting with its tip on `p` at depth `z_p` occupies the closed cone above its tip: at lateral
distance `s` from `p` its surface is at `z_p + s / tan(θ/2)`, with `θ` the included angle. The **tool
body is above that cone**, so the material left standing after sweeping a set of tips is the **lower
envelope** of their flanks, not the upper one. With `z_p = -d(p) / tan(θ/2)`, where `d(p)` is the exact
distance from `p` to the boundary of the region `R`:

1. For any `p` and any `q` inside `R`, `|q - p| - d(p) >= -d(q)`, because `d` is 1-Lipschitz. So the
   swept surface never rises above `-d(q) / tan(θ/2)`: **the carve can only leave material standing, it
   can never cut deeper than the distance field.**
2. Equality is attained at the tip whose nearest boundary point is the one radially aligned with `q`,
   so the surface is exactly `-d(q) / tan(θ/2)` **provided the tips reach every point of `R` that way**.
3. `R` is exactly the union of the balls `B(p, d(p))` over the **medial axis** of `R`, and no ball
   `B(p, d(p))` leaves `R`. That is the definition of the medial axis, and it is why the medial axis is
   the toolpath: it is the smallest set of tips whose cones cover `R`.

Two consequences the tests assert rather than assume: **gouge is exactly zero for any tip set inside
`R`** (point 1), and **coverage is complete if and only if the tips' medial balls cover `R`** (points 2
and 3). Error is therefore one-signed by construction: a skeleton piece that is dropped or misplaced
costs material coverage and shows up as `uncut`, never as gouge. This is the same one-signed discipline
the region booleans recorded as D-RB-9.

## Acceptance criteria (derived)

What "M7 delivered" means, testable through `cam-cli` unless the criterion names the window (S9):

- **A1 — The V-carve surface is the distance field, at the V-bit's own angle.** For a region `R` and a
  V-bit of included angle `θ`, the material left standing after the toolpath has its top at
  `-d(q) / tan(θ/2)` for every `q` in `R`, and the volume removed is `(1 / tan(θ/2)) * ∫_R d dA`. The
  law is derived **once**, beside the tool, so the program and the simulation's claim cannot disagree
  (M6's D-M6-2 precedent). A tool the library does not describe as an angled bit is an input error
  naming the tool, never a defaulted 90 degrees (D-M6-9).

- **A2 — The skeleton is verified, never guessed.** `cam-geom` exposes the medial axis of a region as
  branches; a point on a branch is a medial point **because the exact empty-ball test says so** — the
  nearest boundary *points* (deduped by point, so a vertex shared by two segments is one point) number
  at least two, or the boundary is smooth there and every nearest point is at the same exact distance.
  A candidate that fails the test is dropped whole, never nudged into place, and the drop is counted.
  Candidates are generated from a nearest-feature lattice and refined by bisecting the two distances
  along a crossing lattice edge, which is what makes an exact certificate reachable without constructing
  a bisector; the refinement is a function of the region and the tolerance alone, so two runs give the
  same branches in the same order. *(Amended by execution: the lattice+bisection route replaced "the
  bisectors of boundary element pairs" because a bisector involving an arc is a conic the kernel has no
  type for — and bisecting a scalar needs none. See L10.)* **Topology is an answer, so it is tested**: a
  finer tolerance never returns more branches than a coarser one, because a finer tolerance that
  fragmented one axis into thirteen would retract the tool thirteen times for a better measurement.

- **A3 — The program is a walk of the skeleton at a varying Z, and it needs no post change.** Each
  branch is a rapid to safe Z, a plunge at the first vertex's own depth, then one `Move::Feed` per
  vertex with that vertex's own Z (`S12`), and a retract to safe Z between branches (`S14`). No arc
  move is emitted: `Move::Arc` is constant-Z by contract and a V-carve's Z is a function of position.
  The emitter's existing `FEED_MOVE` block carries `[Z]`, so the workspace's post contract is
  untouched, and the sweep measures the move honestly (`S13`).

- **A4 — The document carries the operation, and the schema tag does not move.** `Operation::VCarve`
  with a parameter record holding the bit's included angle; it round-trips losslessly inside
  `c2d/document@2`, an unknown `kind` is still a hard load error, an angle outside `(0, 180)` is refused
  at load naming the value, and every previously serialised document loads and re-saves byte-identically.
  The variant is additive under the existing `kind` tag, which is why the tag does not bump (M4's and
  M6's precedent).

- **A5 — `cam-cli toolpath --kind vcarve` authors it, and refuses the rest by name.** `--entities`
  (closed contours: one or many, the region is their even-odd union), `--tool`. It refuses, naming the
  flag and the reason, every flag whose meaning it does not have: `--cut-depth` (the depth is derived),
  `--pass-depth`, `--stepover`, `--allowance`, `--side`, `--tab`, `--lead-*`, `--ramp`, `--spacing`,
  `--angle`, `--pitch`, `--chamfer-width`, `--inlay-allowance`, `--clear`, `--start-depth` (D-M7-3).
  A flat-tipped or unangled bit exits `2` with the file byte-identical. `inspect` names the kind.

- **A6 — The simulation authorises the distance field and the oracle has both signs (S11).** The
  target's authorised depth for a V-carve cell is `d(centre) / tan(θ/2)` clamped to the stock, computed
  from the document alone. Tests assert the analytic volume for a square and a circle, assert that a
  **deliberately thinned skeleton** reports `uncut` and no gouge, and assert that a deliberately deeper
  toolpath reports `gouge` — so neither sign can pass as the whole truth. Gouge on a correct V-carve is
  **exactly zero**, which is assertion 1 of the claim above, not a tolerance.

- **A7 — The field and the program agree about the origin (S17).** One `Frame` definition relates the
  document frame to the machine frame, and it is honoured at every boundary: the field's grid and cell
  mapping, the program's own coordinates, and **the handedness of every arc**. A reflection reverses
  orientation, so at `top_left` a curve the document traverses counter-clockwise is clockwise in the
  machine's own numbering, and an emitter that maps only its points emits the complementary arc. Tests
  cover each origin corner (`bottom_left`, `top_left`, `center`), each asserting that a program emitted
  at that origin reports the same removed volume and the same zero gouge as the same geometry at
  bottom-left, that an arc and a helix cut where the document puts them, and that a crash is reported
  at the program's own coordinates. The bottom-left numbers are unchanged bit for bit. This is T0
  because A6 cannot be trusted while the blind spot that hid this defect for two milestones is open.
  *(Widened during T0's own work: the arc handedness half was found by the implementation, not by this
  document — see L6.)*

- **A8 — Determinism, no new dependency, and the old bytes (S10).** Two runs of every new subcommand
  print and emit identical bytes, the branch order is asserted equal across runs, and a differential
  over the pre-existing command surface against `9ff0b8c` reports differences only in `--help`, the
  unknown-`--kind` sentence and the added commands. `Cargo.lock` gains no `[[package]]`. The 975
  pre-existing tests keep passing unmodified except where A7 moves an expectation, and each such move
  is re-derived, never fitted.

## Decisions

- **D-M7-1 — M7 is V-carve to the point: no relief pass, no flat bottom.** The owner's two decisions
  (S2). Corner relief is *not needed for the carve to meet the vectors*, and that is a consequence of
  the law rather than an omission: at a boundary point `d = 0`, so the surface tapers to zero depth
  exactly at the edge, sharp corners included. What a real machine leaves in a corner is the tip's own
  finite radius, and that is `uncut` the simulation reports. Flat Depth, `DoFlatBottom` and
  `UseAreClearTool` (S6, S8) are recorded out of scope, not dropped: they change what the operation
  *is*, and the milestone's risk is the skeleton.

- **D-M7-2 — The medial axis is the toolpath, and the empty-ball test is the only thing that certifies
  it.** Candidate generation is cheap and unreliable (a bisector can be sampled anywhere); the
  certificate is exact and cheap. So the implementation separates them: generate candidates, verify
  each with the exact distance the kernel already has (`Arc::distance_to_point`,
  `LineSeg::distance_to_point`, `segment_to_segment`), keep only certified points, chain them, and drop
  whatever fails whole. The nearest set is deduped **by point**: a vertex shared by two segments is one
  nearest point, not two, or every corner of every polygon would certify itself as a medial point.

- **D-M7-3 — `start_depth` is deferred, so the law stays exact.** With `start_depth > 0` the tip sits
  `s` below the surface, so the cone's intersection with the stock top reaches `s * tan(θ/2)` **outside**
  the vectors and the authorised surface is no longer `-d(q)/tan` inside `R` alone. That is a claim of
  its own, and shipping it now would make the milestone's cleanest assertion — `gouge` exactly zero —
  false for a legitimate program. `_vcpdStartDepth` (S6) is recorded as observed and deferred, and the
  CLI refuses `--start-depth` by name (A5) rather than accepting and ignoring it (S16).

- **D-M7-4 — Arcs are tessellated **for the skeleton only**, counted, and the depth is always measured
  against the original boundary.** The kernel has no conic, and every bisector involving an arc is one
  (a parabola or a hyperbola). Tessellating the *boundary* to build the *axis* is what makes the
  straight-line machinery reach `A2`'s certificate, and the count is reported the way every importer
  reports a flattened curve. The Z at each vertex is then taken from the **true** boundary, arc-native,
  so tessellation can only displace a tip laterally; by the claim's point 1 that costs coverage, never
  depth. `_vcpdPreserveArcs` (S6) — the product keeping arcs in its output — is therefore out of scope
  and recorded as such.

- **D-M7-5 — The tolerance is the branch refinement step, declared once.** It is the same *kind* of knob
  the product calls `VCarve Toolpath Tolerance` (S7) but not the same knob: the product simplifies a
  computed path afterwards to reduce file size, while M7 refines the skeleton to a step and reports the
  coverage its step costs. Naming both here keeps the CLI's `--tolerance`-shaped flag honest if one is
  ever added; M7 adds no tolerance flag and takes the job's existing tolerance.

- **D-M7-6 — T0 is the origin fix, and it is a prerequisite rather than a detour.** M7's acceptance test
  is a simulation report (S11). While `grid_for` and the sweep disagree about the origin at three of
  four corners, an oracle can only be written at the one corner where the bug is invisible — which is
  precisely how it survived M5 and M6. The fix is the roadmap's own recorded next step (S17).

- **D-M7-7 — `Move::Arc` is not extended and no helical block is reused.** A V-carve's constant-`d`
  contour is not a constant-Z curve, so an arc move cannot carry it, and `Move::Helix` is a descent at
  a constant radius, not a bisector. The walk emits linear feeds with per-vertex Z, which the existing
  post blocks and the D-M6-12 sweep already handle (S12, S13). The alternative — chording the level
  sets into arc blocks — would trade the law's exactness for a shorter file, and M7 has no evidence
  that the file length matters at this scale.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T0 | The frame is honoured at every boundary: one `cam-model::Frame` definition, the field's grid and the sweep's cell mapping, a crash's own coordinates, and the **handedness of arcs and helixes** (a reflection reverses orientation), with a test per origin corner | A7, S17, D-M7-6, D-M7-8 | inline, test-first | `786c3d8` + `228ecc7` |
| T1 | `cam-geom`: the exact distance from a point to a region's boundary, over every boundary segment, arcs native; property-checked against a sampled lower bound | A1, D-M7-2 | inline, test-first | |
| T2 | `cam-geom::medial`: the verified medial axis — nearest-feature lattice, exact bisection, the empty-ball certificate (contacts deduped by point), a stitch pass over refused crossings, deterministic branch order, and the monotonicity property that a finer tolerance never returns more branches | A2, D-M7-2, D-M7-4, D-M7-5 | delegated, test-first (the milestone's research unit, three rounds: T2, T2b, T2c) | `eb989ab` |
| T3 | `cam-model`: `Operation::VCarve` + the parameter record, one shared law beside the tool, refusals at load, `@2` round trip | A1, A4 | inline, test-first | |
| T4 | `cam-toolpath::vcarve`: walk the branches at `-d/tan(θ/2)`, per-vertex Z feeds, retract between branches, flat-tip and unangled-bit refusals | A3, D-M7-7, S14 | delegated, test-first | |
| T5 | `cam-cli toolpath --kind vcarve`, the named refusals for every flag it does not have, `inspect`, `--start-depth` refused | A5, D-M7-3, S16 | inline, test-first | |
| T6 | `cam-sim`: the distance-field claim for a V-carve, `simulate` integration, and the oracle in both signs (square, circle, island; thinned skeleton; too-deep path) | A6, S11 | delegated, test-first | |
| T7 | Closing evidence: golden programs for a square and a circle region, `docs/m7-vcarve.md`, README status, roadmap §4.10 + M7 ✅ + §6 risk closure, `docs/format.md`, the `9ff0b8c` differential, `./scripts/check.sh` | A1–A8 | delegated | |

## Recorded out of scope

- **Corner relief / corner sharpen** (S3's row wording, owner decision S2): no extra pass, no extended
  walk. The law reaches every boundary point, so what a machine leaves in a corner is `uncut` — and
  that is reported, not papered over. The product's `CornerSharpen`, `CornerLimit`, `SharpCornerAngle`,
  `PreserveCorners` strings are profile-strategy vocabulary; they are not adopted for this operation.
- **Flat Depth / flat bottom / area-clear tool** (`_vcpdFlatDepth`, `_vcpdDoFlatBottom`,
  `_vcpdUseAreClearTool`, and the whole validation family in S8): owner decision S2. The product's
  messages are quoted in S8 so a later milestone starts from the evidence rather than from memory.
- **`_vcpdStartDepth`** (D-M7-3): refused by name in the CLI, deferred with its reason.
- **`_vcpdProjectToolpath`** (S18), **`_vcpdPreserveArcs`** (D-M7-4), **`_vcpdUseSelectionOrder`**,
  **`_vcpdDropTool`**, **`_vcpdUseStartPoint`**, **`_vcpdTolerance`/`_vcpdStartDepthFormula`/
  `_vcpdFlatDepthFormula`**: observed in the product's dialog (S6), unscheduled here. Recorded so that
  "V-Carve is done" later cannot mean "everything in the product's dialog".
- No toolpath form in the window (S15), no 3D or laser strategy or projection (S18), no new crate or
  dependency (A8).

## Log

- **L1 (owner, verbatim).** "conmenzamos con M7".
- **L2 (owner decisions, verbatim option labels).** "Nada extra: `simulate` lo reporta como uncut" and
  "No: M7 es V-carve a punta pura". The two questions and why they were asked are recorded here because
  the answers bound the milestone: the roadmap's M7 row says "corner relief" (S3), a phrase the product
  does not use for this strategy — what the executable has is `CornerSharpen`/`CornerLimit`, which are
  profile vocabulary — and the row says nothing about Flat Depth, which the product's own dialog does
  have (S6). Asking instead of choosing kept the milestone from either inventing a semantic or
  inheriting one silently.
- **L3 (pre-flight, evidence).** `master` was clean at `9ff0b8c` (`merge: M6 depth-variant strategies`),
  one worktree, no stale branches, and `cargo test --workspace` green at **975 tests** — the number M6's
  handoff recorded, re-measured rather than trusted. The analysis report had **no** V-Carve section;
  the evidence in S6/S7/S8 was recovered from `evidence/derived/exe_utf16_strings.txt`, which the report
  quotes for other strategies but not this one. Recorded because "the dossier has no V-Carve
  parameters" is itself a finding: it is why S3's row is the only specification of intent, and why every
  parameter named in this document carries a string and a line number.
- **L4 (pre-flight, the post needs no change — the opposite of M6's finding).** M6's thread milling cost
  four new post block kinds because `Move::Arc` is constant-Z and no arc block carried `[Z]`. M7 checked
  the same question before writing any spec and got the other answer: `FEED_MOVE` is
  `"[N]G01[X][Y][Z]"` and `FIRST_FEED_MOVE` is `"[N]G01[X][Y][Z][F]"` (S12), so a per-vertex Z linear move
  is already expressible, and D-M6-12's interpolating sweep is already able to measure it (S13). The
  milestone therefore carries no post-contract risk at all, which is the main reason D-M7-7 rejects any
  arc-carrying alternative.
- **L5 (plan, D-M7-6).** The roadmap's §6 records an open defect that M6 found and did not fix (S17).
  T0 pulls it forward rather than leaving it: A6's oracle is a simulation report, and the defect makes
  such a report trustworthy at exactly one of the four origin corners. Scheduling it as M7's first slice
  is the roadmap's own recorded next step, not new scope.
- **L6 (execution, T0, a defect older than the milestone and one the document did not anticipate).**
  T0's worker found that a reflection **reverses the handedness of an arc**, which the red test showed
  directly: an arc emitted at `top_left` cut nothing where the document's own arc cut `-4 mm`, and its
  helix likewise. The sweep's mapping was fixed to flip `ccw` when the frame reflects. Verifying that
  finding turned it into a bigger one: `cam-toolpath` passes `ccw: arc.is_ccw()` through unchanged
  (`crates/cam-toolpath/src/lib.rs:659` and `:772`) while mapping only the points, so at `top_left` the
  **emitted program itself** states the complementary arc — a G02 where the machine needs G03. That is
  not a reporting error like §6's: it is a wrong cut on real hardware, it has been there since arcs
  first reached the emitter (M4), and no fixture noticed because every fixture is bottom-left. A7 was
  widened to cover it and T0 was widened with it; the mapping's `is_reflection` (which exists for the
  sweep) is the primitive the toolpath layer needs, which is why the two halves share one definition.
- **L7 (execution, T0, the third site and a second half only the first fix revealed).** A first writer
  argued that `thread.rs` needed no change because its helix is built in machine coordinates, and the
  argument is wrong in a way worth recording: only the orbit's *start* was in machine space. The
  strategy maps the centre but offsets the start along **+x**, which a Y reflection leaves alone, and
  computes the fractional turn's end point with `+sin` around that machine centre — so `top_left`
  emitted the opposite orbit (the read-back test measured `39.608` where `bottom_left` emits `60.392`)
  *and* the opposite hand, which would have shipped a left-hand thread from a document that describes a
  right-hand one. Fixing `ccw` alone was therefore not enough, and the test that states the rule — a
  program read back into the document frame must be the program `bottom_left` emits — caught the second
  half only after the first was fixed. Three construction sites in the workspace build a `Move::Arc` or
  `Move::Helix` from document geometry and all three now state their direction in the machine's
  numbering; the only other construction site, `thread.rs`'s, was the same defect. Bottom-left programs
  are byte-identical after all of it: five `emit` fixtures compared with `cmp` and the thread's own
  move dump, 22 `G02`/`G03` lines among them, no difference.
- **L8 (process, T0's slice and a warning that measures the wrong range).** T0 landed as two commits —
  `786c3d8` (the `Frame`, the field's mapping, 1044 lines of code and tests plus this document) and
  `228ecc7` (the program's handedness, 476 lines) — because a new `Frame` with no caller is dead code and
  the handedness fix cannot be separated from the map it uses, while the two are still reviewable alone
  and each passes the suite on its own. Both are past the ~400-line guideline and that is deliberate:
  splitting by layer would leave one commit uncompilable. The commit-slice warning reports "19 commits
  since `origin/master`", which is M6's 17 unpushed commits plus these two — the range it measures is not
  this branch's work. `git log --oneline 9ff0b8c..HEAD` is the honest count.
- **L9 (execution, the suite's numbers).** The workspace stood at 975 tests at `9ff0b8c`; T0 added 13
  (2 `cam-sim/tests/origin.rs`, 4 `cam-model/tests/frame.rs`, 3 `cam-sim/tests/frame.rs`, 3
  `cam-toolpath/tests/frame.rs`, 1 in `cam-cli/tests/cli.rs`) for **988**, green under
  `./scripts/check.sh` with `fmt` and `clippy -D warnings` clean. No pre-existing expectation was moved:
  every change here was additive at `bottom_left`, which is why the differentials could be byte-level.
