# M7 — vcarve

[Back to index](index.md)

### 4.1 Why V-Carve is its own milestone and not "just another strategy"

V-Carve computes cut depth from the distance between a point and the nearest boundary, scaled by the
V-bit's included angle, which means the toolpath is not a fixed-Z pass but a depth field. That
requires a medial-axis/straight-skeleton computation on geometry containing arcs, plus corner-relief
handling. The strategy is well defined — the depth is zero at a boundary and grows with the distance —
but the computation is genuinely research, and putting it inside M4 would have hidden a multi-week
unknown inside an otherwise mechanical milestone.


### 4.10 M7 delivered (evidence)

M7 is the milestone the roadmap isolated on purpose (§4.1), and it earned the isolation: it is the only
strategy whose toolpath is not a pass sequence but a **depth field**, and it needed two things the
workspace did not have — an exact distance from a point to a region's boundary, and a medial axis of that
region whose every vertex carries a certificate rather than a hope. Its shape is the mirror image of
M6's: before writing any specification M7 asked the post-contract question M6 had answered the expensive
way, and got the other answer (L4). `FEED_MOVE` is `"[N]G01[X][Y][Z]"` and D-M6-12's sweep interpolates Z
along a cutting move, so a per-vertex Z feed is already expressible: **M7 changes no post contract at
all**. What it spent instead is geometry, plus one defect M6 recorded and did not fix.

The law is derived once and asserted in both directions. A V-bit's body is above its cone, so a tip at
`−d(p)/tan(θ/2)` can only leave material standing — `|q − p| − d(p) ≥ −d(q)` because `d` is 1-Lipschitz —
and the region is exactly the union of the medial balls `B(p, d(p))` over the axis, so the axis is the
smallest set of tips that covers it. Two consequences are tests rather than claims: **gouge is exactly
zero** for any tip set inside the region, and a dropped piece of axis costs `uncut` and nothing else.
Error is one-signed by construction, which is why the tolerance is a coverage knob and never a depth knob.

The research unit was T2, and measurement changed its design three times: the route (a scalar bisection
along a lattice edge replaced "bisectors of boundary element pairs", so **arcs stay arcs and nothing is
tessellated** — D-M7-4's premise superseded), the certificate (contacts deduped at a tenth of the
tolerance, because a shared joint was being counted as two and the 5 x 2 ellipse reported **13 branches at
0.1 mm against 1 at 0.5 mm**), and the topology (a stitch pass joins the two ends a refused candidate
leaves, so the square's four branches stayed four, the L's ten became nine and the ellipse's thirteen
became one). The property that guards all of it is
`a_finer_tolerance_never_returns_more_branches_than_a_coarser_one`.

The feature document is `odd/tasks/009-m7-vcarve.md`; the law, the axis's certificate, the walk, the
refusals, the tolerance's four declared costs, the simulation's claim and the three defects T0 found are
in [docs/old/m7-vcarve.md](../../docs/old/m7-vcarve.md). The workspace stands at **1056 tests across 79 test
targets** — 81 more than the 975 the branch cut from — green under `./scripts/check.sh` (`fmt --check`,
`clippy -D warnings`, the whole suite). In the crates M7 touches that is cam-geom **240**, cam-model
**58**, cam-toolpath **136**, cam-sim **109** and cam-cli **121**.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The exact distance from a point to a region's boundary, arcs native, with membership in the same answer | `crates/cam-geom::region::Region::distance_to_boundary`, `cam-geom::segment::Segment::nearest_point` | 11 tests in `tests/region_distance.rs`; a point inside a square is as far from a wall as the nearest wall, the centre of a circle is exactly one radius away and **not** the apothem of a chord, an annulus reports the nearer of its two radii, a point on the boundary is exactly `0.0` away and not material, and the tolerance decides membership but never the distance; a region with no boundary answers `None` rather than `0.0` or a panic |
| The verified medial axis: branches and vertices with exact clearances, candidates dropped whole, deterministic order | `crates/cam-geom::medial` | 14 tests in `tests/medial.rs` and 3 in `tests/medial_coverage.rs`; `every_retained_vertex_of_every_fixture_is_a_certified_medial_point` asserts the empty-ball certificate on seven fixtures, and the fixtures' shapes are asserted by name (a square's four diagonals meeting at the centre, a disc's centre alone — with a caller-tessellated disc producing 72 branches, so the difference is the caller's chords, not the module's); two runs return the same branches in the same order vertex for vertex; `a_finer_tolerance_never_returns_more_branches_than_a_coarser_one` and `a_finer_tolerance_never_covers_less` pin the direction, and **coverage is measured at both tolerances on six fixtures: `0.97694`–`0.99997` at 0.5 mm and `0.99670`–`1.00000` at 0.1 mm**, with the largest missed clearance `0.20` mm at 0.5 and `0.00` at 0.1 |
| `Operation::VCarve` and its record inside `c2d/document@2`, the depth law beside the tool, refusals at load | `crates/cam-model` | 6 tests in `tests/document.rs`; the record round-trips losslessly with its angle and tolerance, `VCarveParams::depth` is `clearance / tan(θ/2)` (10 mm of clearance on a 60° bit is `10·√3` deep), **no `start_depth` field exists** (D-M7-3), an angle outside `(0, π)` and a tolerance that is not a positive length are refused at load naming the value, an open contour is refused as such, and the schema tag does not move |
| The walk: one linear feed per tip at that tip's own Z, a retract between branches, no arc move | `crates/cam-toolpath::vcarve` | 10 tests in `tests/vcarve.rs`; the emitted moves equal, move for move, a walk built from the axis's own certified vertices at `clearance / tan(θ/2)` written out by the test, so a wrong depth cannot agree with itself; a 90° bit reaches the clearance itself; a disc's program is one plunge and one retract; a `top_left` origin maps every tip into the machine frame; the tolerance changes the axis's step and never a single Z; an open contour, an angle outside the cone, a tool with no angle and a flat-tipped bit are each refused by name; a branch that passes a junction is walked through it |
| `cam-cli toolpath --kind vcarve`: the flag, its default, its 17 named refusals, and `inspect` | `crates/cam-cli` | 9 tests plus the goldens below; the stored angle and tolerance are the library's and the flag's, `--tolerance` defaults to the documented `0.1 mm` (a document authored with the flag and one without are byte-identical), a flag the kind has no meaning for is refused by name with the file byte-identical (17 of them, `--start-depth` and `--clear` among them), a tool with no angle and a flat-tipped bit exit `2` naming the tool and the flat, and `inspect` names the kind |
| An empty axis is refused by name instead of written as a document whose only outcome is `uncut` (T7, the gap T3–T6 left) | `cam-toolpath::ToolpathError::VCarveNothingToCarve`, `crates/cam-cli` | `a_vcarve_whose_axis_is_empty_exits_two_naming_the_tolerance_and_the_region`: a 20 x 20 mm region at `--tolerance 50` exits `2` with `the V-carve's 50 mm tolerance leaves nothing to carve in the 20 x 20 mm region …` and the document byte-identical; the error contract's exhaustive message list grew **23 → 24**; `emit` and `simulate` refuse the same document in the same words, because they compute the same walk |
| The simulation's claim for a V-carve, and the oracle in both signs (S11) | `crates/cam-sim::target::claim_v_carve`, `tests/vcarve.rs` | 5 tests on a 0.5 mm grid with a 0.1 mm axis: a 20 x 20 mm square removes **1332.5** of its analytic `1333.333 mm³` and a radius-15 disc **3534.152** of `3534.292`, each with `uncut 0`, `gouge 0` and the apex at `−10` and `−15`; a **deliberately thinned axis** reports `uncut 142.043 mm³ over 642 cells` and `gouge 0`; a program **20% too deep** reports `gouge 629.557 mm³ over 1937 cells`; and the same square at a **0.5 mm axis** reports `uncut 0.207 mm³ over 4 cells` with still `gouge 0` — D-M7-5's convex-corner band, measured |
| The frame is honoured at every boundary, with a test per origin corner (T0, A7) | `crates/cam-model::Frame`, `cam-sim::sweep`, `cam-toolpath`'s three arc/helix construction sites | 13 tests: 4 in `cam-model/tests/frame.rs` (each corner maps the job's corners to the machine corners it names, machine↔document is a round trip, only bottom-left is the identity, only top-left mirrors handedness), 3 in `cam-sim/tests/frame.rs` (an arc, a helix and a crash cut and are reported where the document puts them), 3 in `cam-toolpath/tests/frame.rs` (a reflected origin emits the complementary arc and helix, a translated one does not, and `every_origin_corner_emits_the_program_bottom_left_emits_once_read_back`), 2 in `cam-sim/tests/origin.rs` (a correct program gouges nothing and reports the same numbers at all three corners) and 1 in `cam-cli/tests/cli.rs` |
| Golden programs for a square and a circle region, each asserted three ways | `crates/cam-cli/tests/cli.rs` | 6 tests; a 20 x 20 mm square at a 90° bit and a 0.1 mm axis emits **413 lines** — four branches, 396 feed tips, the corner tip at `−0.2000`, the apex at `−10.0000` — and a radius-15 disc emits **12 lines**, one plunge at `−15.0000` and not one lateral move; each is pinned as built, after a `c2d` save/load round trip and with the toolpath authored through `cam-cli toolpath`, and every feed's Z was **read against the geometry** before pinning (396 of 396 exactly `−distance`, worst deviation `1.4e-15` mm) |
| Determinism and the old bytes, and no new dependency | workspace | two runs of `toolpath --kind vcarve` and of `emit` print and emit identical bytes, asserted from the document on disk through the program; the pre-existing command surface was run against the branch's cut point `e0d39a3` in an identical sandbox — **35 invocations** over `validate`, `inspect`, `check`, `postp validate`, `tools list`, `tools add`, `toolpath` for profile, pocket offset, pocket raster with an island, drilling with a bank and thread, `emit` (to a file, to stdout, with an arc-less post, with an arc-only post and without a library), `simulate` at 1, 0.5 and 2 mm and at a resolution the cell cap refuses, `import` as a report and with `--out`, the error paths, an unknown command and an unknown kind — comparing exit code, stdout, stderr and **every produced file** (**844 file snapshots** hashed, documents as their `document.json` payloads). Exactly two differ, both by design: `--help` and the unknown-`--kind` sentence. `Cargo.lock` is unchanged: the same **470** `[[package]]` entries |

T0 found **three** defects older than the milestone, and none was in the feature document's scope. The
first is M6's recorded one, now closed: the field and the program disagreed about the origin at every
corner but bottom-left, and on M6's own fixture (an 80 x 30 mm band in a 200 x 100 mm sheet, 1 mm cells) a
**correct** program reported `gouge 14376.000 mm³ over 2396 cells` and `uncut 14400.000 mm³ over 2400
cells` at `top_left`, against `gouge 0.000` at bottom-left. After `d8cb895` (the `Frame`, the field's
mapping) and `4d6849e` (the program's handedness) the same fixture reports its bottom-left numbers
**identically at all three corners**: removed `14376.000`, gouge `0.000`, uncut `24.000 mm³ over 4 cells`.
The second and third were found by the fix itself: a reflection **reverses the handedness of an arc**, and
`cam-toolpath` passed `ccw` through unchanged while mapping only the points, so at `top_left` the emitted
program stated the **complementary** arc — a `G02` where the machine needs `G03`, a wrong cut on real
hardware since M4, invisible because every fixture was bottom-left. The third was `thread.rs`'s helix,
where only the orbit's start was in machine space: `top_left` emitted the opposite orbit **and** the
opposite hand. All three are fixed, all three have tests, and bottom-left programs are byte-identical
afterwards (five `emit` fixtures compared with `cmp`, 22 `G02`/`G03` lines among them).

Recorded deviations, each a decision in the feature document: **D-M7-1** ships V-carve to the point — no
relief pass and no flat bottom — and that is a consequence of the law rather than an omission, because at
a boundary `d = 0` so the surface tapers to zero depth exactly at the edge, sharp corners included;
**D-M7-2** separates cheap candidate generation from an exact certificate, with the nearest set deduped by
**point** so a shared vertex is one contact; **D-M7-3** defers `start_depth`, by name, because with a start
depth the cone's top reaches outside the vectors and "gouge is exactly zero" would be false for a
legitimate program; **D-M7-4** is recorded as **superseded in its premise** — it said arcs would be
tessellated for the skeleton, and the implementation bisects a scalar instead, so arcs stay arcs and no
chord tolerance exists anywhere; **D-M7-5** is **superseded in its second half** as well: it said "M7 adds
no tolerance flag and takes the job's existing tolerance", and the implementation made the tolerance a
stored, defaulted `--tolerance` (S9/S10: `emit` recomputes the axis from the document alone, so a flag that
was accepted and dropped would make the program depend on a constant nobody named); its four costs are now
declared and measured. **D-M7-6** made the origin fix a prerequisite rather than a detour, which is why
the milestone opens with a coordinate change; **D-M7-7** does not extend `Move::Arc` and does not reuse a
helical block, because a V-carve's Z is a function of position; and **D-M7-8** records the crossing point
the implementation found: the field, the target and the preview stay in the document frame and the program
is read into it once, rather than moving the grid — because M5 places a tab by **arc length from the
contour's start**, so reflecting the target's geometry would move every tab to the other side.

**This milestone's diff has had no native outside view.** The candidate was submitted twice and both times
refused in preflight with `lens_context_budget_exceeded` (`mutation_performed: false`, no authority
created, nothing abandoned): **4 425 inserted lines across 7 commits**, with an explicit `--base-ref` of the
branch's cut point `e0d39a3`. The second attempt asked for **one** lens instead of three and was refused
identically, which is the fact worth keeping: the budget is bounded by the candidate's size, not by the
lens count. The owner's disposition was to disable the review switch for this clone and deliver M7 on its
own evidence, the path M6 took — recorded in the feature document as L12 and here rather than softened.
The milestone's delivery therefore rests on its tests, its simulation oracle in both signs, its two golden
programs and its 35-invocation differential; M6's "merged after a review the tooling refused" has become
M7's "delivered with the review switch off, at the owner's decision", which are not the same fact either.

