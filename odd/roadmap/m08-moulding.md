# M8 — moulding

[Back to index](index.md)

### 4.11 M8 delivered (evidence)

M8 is the eleventh and last strategy scope D2 names, and it is the cheapest of the three strategy
milestones that followed M5 — which is worth recording, because the row that scheduled it expected the
opposite ("Needs the 3D-adjacent math from M7's distance field"). The cost was not the distance field:
S3's decision made the rail an XY path, the cross-section authored `(lateral, depth)` data and the
stepover the spacing of the lateral passes, so **M8 changes no post contract** (a pass has one Z, and
`FEED_MOVE` already carries `[X][Y][Z]`), bumps **no schema** beyond the new `Operation` variant — the
rail rides on `entity_ids`, so a document written before M8 loads and saves byte-identically — and adds
**no crate and no dependency**. What it spent is geometry: a rail walked by **arc length** with the
contour's arcs kept as arcs, a cross-section that is a *function* of lateral distance, and a stepover law
whose coverage is a certificate rather than a hope.

The owner's decision is the whole specification (S3, verbatim: "Sección Z barrida por el rail" — rail in
XY, profile of `(lateral, depth)`, stepover as overlapped lateral passes), and its five readings are
decisions with tests behind them: lateral is positive **to the right of travel** (so a counter-clockwise
closed rail's positive side is *outside* the loop, which a circle fixture pins by claiming material on
both sides of the rail and nothing in the middle); the rail may be **open or closed**; the profile's
laterals **strictly increase** because a cross-section is one depth per lateral distance; a stepover
**equal to** the profile's width is the **two edge passes**; and a stepover **wider** than the profile is
**one pass centred on it**, because with that spacing no second distinct pass exists and a pass at one
edge would cut one authored edge and leave the other. The law lives in `cam-geom` and not in the strategy
because it is lateral-offset geometry, and it states its own guarantee as one predicate —
`StepoverLaw::covers`, "every lateral position of the profile is within half a stepover of a pass" — so
the strategy and the claim rely on a sentence instead of restating the arithmetic.

The gouge argument is a **different** one from the V-carve's, and the milestone says so rather than
reusing M7's words. M7 rests on a 1-Lipschitz distance field; M8 rests on **coverage plus a monotone tool
law**: the claim and the program read the same pass polyline — `Rail::offset_walk` over the same
`Rail::samples(DEFAULT_CHORD_TOLERANCE)` walk — the sweep then samples that polyline more finely at
points that lie *on* it, and the tool's `rise` grows with the distance from it — so a cell's claimed
depth is never shallower than the program's and a correct program can never remove more than was
authorised, for a cylinder and a cone alike. The claim reads the rail, the profile and the stepover from
the **document**, never from the emitted moves, and claims are never summed (D-M5-2: the deepest wins).

**One corner of that argument was false at delivery, and it is now closed.** A *closed* rail whose vertex
list **starts at a corner** has a seam where the walk's last sample and its first are the same point
carrying two different normals; the strategy bridged them with a closing feed the claim's walked path did
not contain. On a 40 x 20 mm frame that was `gouge 2.000 mm³ over 1 cell` — the seam diagonal's own
centre — on a program that was otherwise exact. A seam **in the middle of an edge** is collinear, joins
itself and was exact in both signs, which is the fixture the frame oracle and the frame golden use. The
trigger (the drawing's own start vertex, arbitrary in a DXF and wherever the user began a rectangle) made
it a real case, so it was pinned rather than hidden by
`a_closed_rail_whose_start_vertex_is_a_corner_reports_the_seam_diagonal_as_gouge_a_known_limit`, and the
fix came with that test's inversion: the pass's polyline is now `Rail::offset_walk` in `cam-geom` — the
samples offset by the pass's lateral plus the closing edge a corner seam needs, and nothing when the seam
is mid-edge (`SEAM_EPSILON`) — which the strategy emits and the claim authorises against, so the two
cannot disagree about where a pass cuts. The corner frame is exact in both signs (`496.000` mm³ removed
against `496.000` claimed, over the seam-smooth frame's own **248** cell centres), the mid-edge frame's
numbers did not move, and a differential against `898cab6` over the moulding command surface — five rails
(imported corner-seam, mid-edge, circular, rounded-rectangle and open-arc frames), `toolpath`, `inspect`,
`emit`, `simulate` at 0.25, 0.5, 1 and 2 mm, and eight refusals, 42 compared sections — differs in
`simulate` alone, on the corner-seam documents: `gouge 3.000 mm³ over 24 cell(s)` → `0.000` at 0.25 mm,
`3.000` over 6 → `0.000` at 0.5, `2.000` over 1 → `0.000` at 1, exit **6** → **0**. Every emitted program
and every produced file is byte-identical (all file hashes equal), which is the fact that says the fix
moved the claim and not the program.

The feature document is `odd/tasks/019-m8-moulding.md`; the sweep, the stepover certificate, the walk,
the refusals and the limits are in [docs/old/m8-moulding.md](../../docs/old/m8-moulding.md). The workspace stands at
**1374 tests across 101 test targets** (88 binaries and 13 doc-test targets) — 79 more than the 1295 the
branch cut from `5bdb81a`, and 16 of them M8's closing evidence — green under `cargo test --workspace`.
In the crates M8 touches that is cam-geom **274**, cam-model **62**, cam-toolpath **154**, cam-sim **126**
and cam-cli **140**.

| Deliverable | Where | Proof |
| --- | --- | --- |
| The rail by arc length, the cross-section as a function, the swept band and the stepover law | `crates/cam-geom::moulding` (`Rail`, `CrossSection`, `SweptBand`, `StepoverLaw`) | 24 tests in `tests/moulding.rs`; a point asked for on an arc is on the arc and not on a chord, a corner appears twice with two normals, a cross-section's depth is read between its own authored points and holds its edge depth outside them, a profile that doubles back is refused by name, the band's own containment certificate holds for every sample of the offset family, and the law's five cases are pinned: start-and-step passes, both edges always cut, `stepover == width` as two passes, `stepover > width` as one centred pass, and `coverage_radius ≤ stepover / 2` |
| `Operation::Moulding` and its record inside `c2d/document@2`, validated at load | `crates/cam-model` | 4 tests in `tests/document.rs`; the record round-trips losslessly with its profile and stepover, **no `Toolpath` field and no schema bump** — the rail is the entity, so a document written before M8 is byte-identical — a profile of fewer than two points, a non-finite coordinate and a stepover that is not positive are refused naming the value, and a **closed** rail is accepted as well as an open one |
| The walk: one lateral pass per stepover offset, one feed per point of the pass's offset walk, a retract between passes | `crates/cam-toolpath::moulding` | 13 tests in `tests/moulding.rs`; the emitted moves equal, move for move, a walk built from the authored laterals and depths written out by the test, a closed rail closes its walk and a seamless one emits no duplicate move, the program carries the tool's real shape so the simulation sweeps the cross-section the bit cuts, and a profile shorter than two points, a lateral that does not increase, a stepover with no distance behind it, a rail with no length and a tool with no diameter are each refused by name |
| The simulation's claim, and the oracle in both signs (S6) | `crates/cam-sim::target::claim_moulding`, `tests/moulding.rs` | 17 tests in one file, 7 of them T7's closures: a straight rail removes its analytic **660 mm³** with `gouge 0` and `uncut 0`; a **closed frame** (the whole rail turning four corners and joining itself) removes its **496 mm³** with `gouge 0` and `uncut 0`; 1 mm shallow gives **`uncut 300` over 300 cells** on the rail and **`uncut 248` over 248** on the frame, both with `gouge 0`; 1 mm deep gives **`gouge 300`** and **`gouge 248`** over every claimed cell with `uncut 0`; a **V-bit cone** removes its claim to the last bit (`1027.127 mm³`) with both signs zero and reports **`gouge 603.671 mm³ over 676 cells`** 1 mm deeper; two mouldings on one rail keep the deeper claim and never the sum; and the **corner seam** frame is exact — `496.000` mm³ removed against `496.000` claimed, `gouge 0` and `uncut 0` over 248 cells, pinned by `a_closed_rail_whose_start_vertex_is_a_corner_claims_its_seam_diagonal` (post-delivery, from the `gouge 2.000 mm³` over 1 cell it was delivered with) |
| `cam-cli toolpath --kind moulding`: the flags, the 22 named refusals, `inspect`, and determinism | `crates/cam-cli` | 11 tests, with the goldens and the reports in the two rows below; the stored cross-section and stepover are the flags' (and the library's when `--stepover` is absent), `--profile` belongs to this kind and is refused by name elsewhere, `--stepover` defaults to the library's and refuses with the flag named when it has none, a flag the kind has no meaning for is refused by name with the file byte-identical (22 of them, `--tolerance` and `--clear` among them), a profile below two points and laterals that do not increase exit `2`, `inspect` prints `moulding (3 points, 2 mm stepover)`, and two runs print and emit identical bytes |
| Golden programs for a straight rail and a closed frame, each asserted three ways | `crates/cam-cli/tests/cli.rs` | 6 tests; a 60 mm open rail emits **21 lines** — 3 passes at `Z-1.0000`, `Z-3.0000`, `Z-1.0000` — and a 40 x 20 mm closed frame emits **33 lines** — 2 passes at `Z-2.0000`, the inner one with a repeated corner sample per corner (4 bare `G01` blocks) and the outer one with a 45 degree diagonal across each corner; each is pinned as built, after a `c2d` save/load round trip and with the toolpath authored through `cam-cli toolpath`, and every line was read against the law before pinning |
| Three reports end to end, and the multi-rail case | `crates/cam-cli/tests/cli.rs` | 3 tests; a correct straight-rail moulding prints `program: 12 move(s)`, `removed: 300.000 mm3`, `gouge: 0.000`, `uncut: 0.000` and exits `0`; a frame asking 2 mm out of a **1 mm stock** prints `uncut: 248.000 mm3 over 248 cell(s)` and exits **6**; and a toolpath naming **two rails** cuts and claims both (`removed: 600.000 mm3`, `gouge: 0.000`, `uncut: 0.000`, exit `0`), which is the multi-rail claim proven end to end rather than assumed |
| Determinism, and no new dependency | workspace | two runs of `toolpath --kind moulding`, of `simulate` and of `emit` are byte-identical (each golden test emits its program twice, and the authoring tests compare the documents on disk); and `Cargo.toml`/`Cargo.lock` are unchanged from the branch's cut point `5bdb81a` (`git diff 5bdb81a -- Cargo.toml Cargo.lock crates/*/Cargo.toml` is empty): no crate and no dependency was added |
| The shared chord constant, and the limit M8 no longer has | `cam-sim`'s oracle, `docs/old/m8-moulding.md` | `DEFAULT_CHORD_TOLERANCE` (`0.001` mm) is recorded as a constant `cam-geom`, `cam-toolpath` and `cam-sim` all read — which is what makes the claim's polyline the program's — which **no test pinned** when this row was written; **and the gap was wider than the row said**, because every moulding fixture in `cam-sim` is a straight rail, where a segment samples to one chord at any tolerance, so mutating `claim_moulding`'s call site to `0.01` left all 17 of its tests green. Pinned now, on curved rails, by `a_mouldings_program_is_walked_at_the_one_chord_tolerance_the_claim_reads` (`crates/cam-cli/tests/cli.rs`: the value, the tolerance proved load-bearing, and one `G01X` per further sample of each pass) and `a_mouldings_claim_on_an_arc_rail_is_the_walk_at_the_one_chord_tolerance` (`crates/cam-sim/tests/moulding.rs`: the claimed cells and volume are the default walk's), each proved by installing the mutation and watching it fail. The closed-rail **seam corner**, delivered as a recorded limit at `gouge 2.000 mm³ over 1 cell` on a correct program, was closed post-delivery by `Rail::offset_walk`, with the test it was pinned by inverted and the CLI differential recorded above |

Recorded deviations, each a decision in the feature document: **no tolerance flag, by design** — a
moulding's error is the rail walk's chord error and nothing else, so `--tolerance` is refused by name, and
the constant that sets it is shared by three crates and pinned by none (recorded, not papered over); **no
start depth and no cut depth**, because the cross-section *is* the depth law; **a stepover wider than the
profile is one centred pass** rather than a refusal, which is the honest single answer for that spacing;
and **the seam corner**, delivered as the one place where "gouge is exactly zero" was false for a correct
program and closed afterwards rather than argued away.

The candidate's native review disposition is the delivery's own record, written by the transaction that
delivers it; what this section states is what the tests, the oracles in both signs and the two golden
programs establish.

