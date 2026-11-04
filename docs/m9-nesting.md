# Nesting (M9)

Nesting is the tenth milestone, and it emits no program at all: it answers *which board, and where on
it*, for parts that are already drawn. Two owner decisions fix its shape — D4,
"Implementar nesting heurístico propio en v1", and S7, "Ship rectangle nesting first (covers cabinetry
parts), free profiles second" — and they are the same decision seen twice: an **own** heuristic engine
rather than a licensed one, rectangles before contours. So the milestone is two nesters and one
arithmetic: `cam-geom::nest`, maxrects with a guillotine pass for the cabinetry panels; and
`cam-geom::nest_profile`, a no-fit heuristic for everything a rectangle cannot express, which is
**sound but incomplete** and says which.

What it cost is worth stating in M8's terms, because the answer is not the footprint. It changes **no
post contract** — nesting emits no move at all — bumps **no schema** (the `nesting` section is an
optional top-level field beside `tools_ref` and `postp`, so a document written before M9 loads and saves
byte-identically and `DOCUMENT_SCHEMA` stays `c2d/document@2`), and adds **no
crate and no dependency** (`Cargo.lock` is unchanged across the milestone). What it spent is two
packers, a certificate each, and — the part that took two reviews — the one function that says what a
nesting is worth. The whole milestone is seven commits, ten files, **6 606 insertions and 7 deletions**
from `931812e` to `e3cd977`, and 124 of those insertions are the feature document.

The command, over a document that authors one 1000 × 600 mm board and one 600 × 400 mm panel with one
instance, and has never been nested:

```console
$ cam-cli nest panel.c2d
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
part: P1 rectangle, 1 instance(s)
placement: P1 on S1 at (0.000, 0.000), rotation none, 600.000 x 400.000 mm
algorithm: rectangle, maxrects
sheets used: 1
utilisation: 0.400
waste: 0.600
unplaced: none
```

`0.400` is not the nester's opinion of its own run: it is 240 000 mm² of placed footprint over the
600 000 mm² of the sheets the placements **name**, derived by `Document::nesting_utilisation` and
stored from there (S3). The distinction is the milestone's one arithmetic, and §"The one arithmetic"
below is where it lives.

| Piece | Where |
| --- | --- |
| Maxrects, the guillotine pass and the auto rule between them | `cam-geom::nest::{NestAlgorithm, NestOptions, NestPlan}` |
| The rectangle plan's certificate (inside the sheet, a gap apart on it) | `cam-geom::nest::NestPlan::certifies` |
| The free-profile no-fit heuristic and its coarse lattice | `cam-geom::nest_profile::{ProfileNestOptions, ProfileNestPlan}` |
| The profile plan's certificate (inside the material, disjoint from every no-fit region) | `cam-geom::nest_profile::ProfileNestPlan::certifies` |
| The `nesting` section: sheets, parts, placements, cost, and twelve refusals | `cam-model::Nesting`, `check_nesting` |
| The one arithmetic for utilisation and waste | `cam-model::Document::nesting_utilisation`, `placement_footprint` |
| `nest` — its flags, refusals, report, exit `7` and write-back | `cam-cli` |
| The contract row for the section | this document, and `odd/tasks/m9-nesting.md` |

## What S1–S4 decided

The owner's four answers (L2) are the specification M9 had, and the decisions below are its reading.
They are recorded as D-M9-1 … D-M9-10 in the feature document; the ones that are law for a caller are
here with what pins them.

| Decision | What it means | Where it is pinned |
| --- | --- | --- |
| A **master document** holds N sheets and N instances (S1) | A part is a selection of the document's own entities plus an instance count. There is no parts library and no second document, and the instance count is the nester's unit of work | `NestingPart`, and the CLI placing one entry per instance |
| The **placements** are stored in the document (S2) | A run writes `NestingPlacement`s, never a side file, and the document is rewritten in place or at `--out` | `a_nesting_section_round_trips_losslessly`, `nest_writes_the_out_path_and_leaves_the_input_document_alone` |
| The **cost** is both the report and a stored field (S3) | `NestingCost` carries `sheets_used`, `utilisation` and `waste`; the report prints the same three from the same derivation | `the_utilisation_is_the_placed_area_over_the_used_sheet_area`, and every CLI golden |
| The **sheets** are authored and live in the document (S4) | No sheet library, no default board: a run with no authored board names `--sheets` and exits `2`, and `--sheets` **replaces** the document's own list | `nest_refuses_a_document_with_no_parts_or_no_boards_by_name` |
| A placement's `position` is the **placed** footprint's lower-left corner, and `rotation` turns the part before anchoring (D-M9-1) | A quarter turn about the part's own origin would put the footprint at negative X — the defect T1's tests caught — so the anchor is the placed corner | `a_rotated_placement_is_measured_by_its_turned_footprint`, `a_quarter_turn_places_a_part_the_board_only_holds_sideways` |
| A placement's **footprint** is the part's turned axis-aligned bounding box (D-M9-2) | Load-time containment and overlap are therefore conservative for a free profile: the box is bigger than the part, never smaller | the L bracket's `0.200` in the report below |
| Overlap reuses `cam_geom::winding::relation` (D-M9-3) | `Crossing` and containment overlap; `Touching` does not, because parts nested edge to edge are the point of nesting — except equal bounds, two copies on one spot, which is refused explicitly | `two_placements_that_overlap_on_a_sheet_are_refused`, `two_placements_that_share_an_edge_are_accepted` |
| The **gap** is between parts and never insets the board's edges (D-M9-7) | A part exactly the board's size fits flush on all four edges; whether the board's edge leaves room for the cutter is the author's business | `the_certificate_holds_at_the_sheets_own_edges`, `a_part_flush_against_a_rectangular_board_is_placed_and_certified` |
| A quarter turn is a **fallback**, never a tie-break (D-M9-8) | A part that fits upright is never turned, in either nester, which is why the cabinetry fixture's plan is all `None` | `a_part_is_turned_only_when_it_does_not_fit_upright`, `a_part_is_turned_only_when_no_upright_placement_exists` |
| Two plans are equal only when the algorithm is (D-M9-9) | Identical placements from maxrects and guillotine are unequal plans; deliberate, and a caller comparing plans has to know it | `the_auto_rule_picks_the_better_of_the_two_and_names_it` |

Three more rules are smaller and still refusals rather than defaults: a duplicate sheet or part id is
refused through the model's existing `DuplicateId` (D-M9-4), because a lookup by id must be
unambiguous; `cost` is **absent** until a run writes it (D-M9-5), so an authored request and a nested
result are distinguishable; and a part with `instances: 0` is accepted while a zero-area selection is
skipped by the inside-sheet and overlap rules (D-M9-6) — nothing to nest is not an error. D-M9-10 is
recorded in the arithmetic section: the fractions' sum rule is implied, so the variant that checked it
was deleted instead of kept for its message.

## The two nesters, and the fixture where guillotine wins

Both rectangle heuristics run on the same sorted list — area descending, **stable**, so equal areas keep
the caller's order — and both are reachable by name (`--algorithm auto|maxrects|guillotine`). The auto
rule prefers **fewer unplaced parts**, then **fewer sheets**, and keeps maxrects on a tie, because a
fixed preference is what makes the answer reproducible. A plan names the algorithm that produced it and
never `Auto`, so a report says which heuristic packed rather than leaving the caller to infer it from a
module path.

* **Maxrects** keeps a list of maximal free rectangles, splits every free rectangle a placement touches
  into the strips around it, and prunes the ones another contains. Because those rectangles overlap, a
  split can leave two adjacent strips where a single wider one would do.
* **Guillotine** keeps free rectangles that never overlap: it packs into one and splits the remainder
  with a single cut running the **full length** of the rectangle being split, along the axis with more
  room left. That is the cut a panel saw can actually make, and it is why guillotine can win.

Half of S6 names guillotine, so the milestone owes a fixture where it wins, and L7 delivered one:
a **234 × 173 mm** board at **gap 8**, rotation off, with parts **110 × 60**, **111 × 87** and
**38 × 112**. Maxrects needs **2 sheets**, utilisation `0.25335951781038485`; guillotine needs **1
sheet**, utilisation `0.50671903562076970`, and both plans are certified — the difference is packing,
not legality. The reason is the algorithm and not the tie-break: maxrects splits around the 111 × 87
part into a 123 mm right strip and an 86 mm top strip, so the 38 × 112 part no longer finds 112 mm of
height and is pushed to a second board, while the guillotine's full-length cut leaves a 242 × 86 strip
above the first part for the 110 × 60 part and the full-height right strip for the 38 × 112. The same
auto rule picks guillotine here and maxrects on the cabinetry fixture, by name
(`guillotine_wins_the_contested_fixture_on_sheets`, `the_auto_rule_picks_the_better_of_the_two_and_names_it`).

The cabinetry fixture is the one S7 exists for: a **2440 × 1220** board, gap 8, the two 720 × 580 sides
and three 560 × 520 shelves of one base cabinet. It packs **5 parts on 1 sheet, 0 unplaced, utilisation
`0.57403923676431068`** — 1 708 800 of 2 976 800 mm² — and the two-sheet fixture gives **2 sheets, 8
placed, 0 unplaced**, identical on both runs. The gap is imposed by growing each part by the gap into a
board grown by the same gap, so two grown rectangles that do not overlap are at least one gap apart in
the parts they stand for, and a grown part that fits the grown board fits the real one. A part larger
than the board is **unplaced**, not an error: it opens no board.

## The certificate each states

Each nester exposes the promise as **one predicate** instead of restating the arithmetic at every call
site — `StepoverLaw::covers`'s precedent from M8 — so a plan and its certificate cannot disagree.

* `NestPlan::certifies(sheet, gap)` states that every placement lies **inside its sheet**, the sheet's
  own edges included, and that every pair of placements **on one sheet** is at least the authored
  `gap` apart, measured as the larger of the two axis clearances. That is also the statement that no
  two placements overlap. It is a *check*, not a precondition: the packer builds plans that satisfy it
  by construction, and a caller asserts it to prove the plan it was handed did. A gap that is not a
  distance certifies nothing, and a plan nested with one gap is not certified by asserting a smaller
  one.
* `ProfileNestPlan::certifies(sheet, gap)` states that every outline is **inside the board's material**
  and that every ordered pair on one board is `Disjoint` from the other outline **and** from the other
  outline's no-fit region — the region a candidate may not enter, computed by growing the placed
  outline by the gap. Pairs are checked in one direction only, because the distance between two
  outlines is symmetric.

The two are deliberately not the same sentence: a rectangle is certified by the clearance between its
own bounds, a contour by the kernel's `winding::relation` over its own outline and every no-fit region
already on the board. Writing either twice would be a second implementation of a certificate, and two
implementations can disagree — which is exactly what the first review found in the free-profile one.

## Free profiles: sound, and incomplete by measurement

The published answer to a free-profile nest is the **no-fit polygon** — the locus of the moving part's
anchor for which it collides with a part already placed, the Minkowski difference of the two outlines.
Building it needs a boolean **union**: one placed part grown around each of its own edges and corners.
`Region` offers `new`, `contains`, `distance_to_boundary`, `subtract` and `normalize` — and no union
and no intersection — so the polygon cannot be built, and the module claims no NFP anywhere. What it
claims instead is the pair of properties that make a heuristic usable:

* **Sound.** A placement is accepted only when the candidate's own outline is `Disjoint` from every
  outline already on that board, and from every no-fit region on it. Overlap is therefore impossible,
  not unlikely, and the no-overlap half never depends on the offset kernel's accuracy: it is decided
  between the parts themselves. The two halves are kept separate for that reason.
* **Incomplete, and measured.** Candidate anchors come from a coarse lattice of `--step` millimetres
  (default **10**), so a placement that exists between two lattice points is missed.

The price is pinned with every number written out (`the_coarse_search_step_misses_a_tight_placement_that_exists`):
a **100 × 100** board, a **40 × 100** bar placed at `(0, 0)`, a **50 × 50** square, gap **5**, step
**20**, so the only anchors tried are `0, 20, 40, 60` along each axis. A square at **`(45, 0)`** is
legal — inside the board, exactly 5 mm from the bar, 5 mm from the right edge — and no lattice point
reaches it: 40 is inside the bar's no-fit region and 60 puts the square past the edge. So the run
**reports the square on a second board**, `2` sheets where one would do, and the plan is still
certified and reports nothing unplaced. The step is the whole of the miss: the same fixture at step
**5** finds a one-board placement, and it lands at **`(50, 0)`** — the lattice point past the no-fit
region, not on its corner. That second fact is the module's other declared cost: `Relation::Disjoint`
is strict, so contact is refused, and a touching candidate is refused because a candidate lying
*inside* a no-fit region and touching its boundary — a thin cap hugging the grown edge of the placed
part — is also `Touching`. Refusing contact means the plan pays **one search step beyond the gap**,
measured here as 10 mm where the gap is 5, and a zero gap means "further apart than touching": fewer
placements than an exact nester would find, never an illegal one.

Two more rules complete it. The gap is honoured as a **floor** — no two parts are closer than the gap,
and the lattice decides how much further apart neighbours end up — and it is the room a cut needs
*between* parts, so a part may sit flush against the board's own edge. A quarter turn is offered only
for a part that has **no** upright placement on any open board, never as a tie-break. Boards open one at
a time, only when the parts already placed leave no room; the first anchor tried on an empty board is
the board's own lower-left corner, so a part that fits an empty convex board is never reported unplaced.
Parts are sorted by area descending with a stable sort, anchors advance in `y` then `x`, boards are a
`Vec` searched from the first, and nothing iterates a hash map — two runs return the same placements in
the same order, and `the_same_input_twice_is_the_same_plan` is that sentence as a test.

## Inside-ness is the board's material

A free profile's inside-ness rule was the milestone's one CRITICAL defect, and the fixed rule is worth
stating in full because it is the module's soundness.

Inside-ness is the board's **material**, never the mere absence of a crossing. A candidate whose outline
meets the board nowhere is inside when `Relation::FirstInside` says so: its outline is connected and
meets no boundary, so it and the material it encloses lie in the material's interior. A candidate that
*touches* the board's boundary — a part flush against an edge, which is legal — is inside only when the
material is on the part's side of that boundary, which is decided by **insetting the part by
`INSET_STEP`** and asking the same relation again: a flush part insets to a part strictly inside the
board, while a part outside a concave board insets to a part still outside it. `INSET_STEP` is **0.01
mm** — far above `TOLERANCE` (1 µm), so an inset part that was flush is still measurably inside, and far
below any feature a cut can hold, so the step refuses no part a shop makes. It is the only slack the
rule has, it is a private constant in `nest_profile`, and the tests restate its value so that moving it
moves the boundary cases with it. A part the offset kernel refuses cannot prove it is inside, so it is
refused rather than placed: soundness is what the module guarantees.

As first written the rule accepted `Relation::Touching` as proof of being inside a board, and `Touching`
also covers a candidate wholly **outside** a non-rectangular board that shares one boundary point. The
review's three CRITICAL findings were that one defect: a **50 × 50** square landed in an L board's
**60 × 60** notch at **`(40, 40)`**, a disc tangent to a circular board from outside landed at
**`(0, 0)`**, and `ProfileNestPlan::certifies` repeated the same test — **certifying a part outside the
material**. The fix (`033a291`) accepts `FirstInside` outright, accepts `Touching` only when the inset
candidate is `FirstInside`, and leaves `certifies` calling the same predicate. Three REDs were observed
before any `src/` edit, no existing test changed (85 insertions, 0 deletions), and a differential
against `3508eb9` ran the 11 pre-existing profile tests in a `git archive` copy of the base: 11 passed
on both sides.

## The one arithmetic for utilisation and waste

Utilisation has one derivation and one only: `Document::nesting_utilisation` is placed footprint area
over the area of the sheets **the placements name** — never every sheet in the section — with waste its
complement, and `None` when the document cannot say (no sheets, no placements, or a placement naming a
sheet that does not exist). The stored record must equal what that function says: `validate` compares
each fraction with the derivation and its complement, and `NestingCostFractionMismatch` names the field,
the recorded fraction, the derived fraction and the tolerance they were compared within.

The denominator is the trap, and it is pinned with exact numbers. Five placements of a 100 × 100 part
over a **500 × 400** sheet is **50 000 / 200 000 = 0.25**, with a second **1000 × 1000** sheet authored
but named by no placement **deliberately excluded** from the denominator; the T1 fixture pins
**30 000 / 240 000 = 0.125**. The tolerance is the kernel's `DEFAULT_TOLERANCE` (1e-6) and not
`DEFAULT_CHORD_TOLERANCE`, which is a tessellation chord and not a comparison epsilon, and the boundary
test is seated on the 0.25 fixture for a stated reason: on 0.125, `+1e-6` computes
`1.000000000001e-06` and the inclusive check refuses that side, so a boundary test on the wrong fixture
would test the wrong side.

What the strong rule was for, it did: it indicted **T1b's own fixture**, which recorded `0.25` / `0.75`
— sums to one, wrong about the area — and six tests went red for that one reason. The fixture was wrong
and the rule was right, so the fixture now carries the derived pair with a comment citing the two areas,
and says why: a hand-invented pair that sums to one is exactly the record the rule refuses.
`NestingCostFractionsDoNotSumToOne` was then **deleted** rather than kept for its message (D-M9-10):
the sum is implied by comparing each fraction with the derivation, and a `pub` error variant nothing can
produce is dead contract.

The one thing the CLI must not do is reopen the trap from the other side: `NestPlan::utilisation`
divides the placed area by **every board the plan opened** (`sheet_area * sheets_used`), which is the
nester's view of its own run, not the document's. `cam-cli nest` therefore writes `NestingCost` from
`Document::nesting_utilisation` and prints the same values, and when the document cannot answer it
writes no record at all rather than an invented zero — which is why the exit-7 report below prints
`utilisation: none` and `waste: none`.

The report says the rest in its own words. A free profile is placed by the other nester and the report
names it, with the lattice step it searched:

```console
$ cam-cli nest bracket.c2d
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: used
part: Bracket free profile, 1 instance(s)
placement: Bracket on S1 at (0.000, 0.000), rotation none, 300.000 x 400.000 mm
algorithm: free profile, free-profile no-fit heuristic at 10.000 mm
sheets used: 1
utilisation: 0.200
waste: 0.800
unplaced: none
```

The bracket is 60 000 mm² of material inside a 300 × 400 box, so its own area would be `0.100` of the
600 000 mm² board; `0.200` is the **stored** footprint's (D-M9-2), and it is the same number
`validate` used to accept the placement. One arithmetic, read twice.

## The CLI surface, and why exit 7

```text
cam-cli nest <document> [--sheets <WxH>]… [--gap <mm>] [--rotation <on|off>]
                        [--algorithm <auto|maxrects|guillotine>] [--step <mm>] [--out <path>]
```

The document authors the parts and normally the sheets; `--sheets` replaces the document's list and is
required when it authors none, `--gap` defaults to `0`, `--rotation` to `off`, `--algorithm` to `auto`,
`--step` to `10`, and `--out` defaults to the document that was read. The run reports on stdout and
writes the placements plus the cost into the section, so the state the user authored (S1: boards and
instances, no placements) becomes the state a run produced.

Exit codes `0`–`6` were taken before M9, and a nesting that could not place every instance is **not**
invalid input and **not** a simulation finding: it is a finding about the document's own request, so it
takes the next code. Exit **7** means an instance fits no board, the report has already printed the
shortfall, and **nothing is written** — a document whose placements fall short of a part's instance
count does not load (`NestingInstanceCountMismatch`), so a partial nesting has no honest stored form
and the file stays exactly as the run found it.

```console
$ cam-cli nest wide.c2d --sheets 1000x600
nest: 1 sheet(s) of 1000.000 x 600.000 mm, 1 part(s), 1 instance(s)
options: gap 0.000 mm, rotation off
sheet S1: unused
part: P1 rectangle, 1 instance(s)
algorithm: rectangle, maxrects
sheets used: 0
utilisation: none
waste: none
unplaced: P1, 1 of 1 instance(s)
$ echo $?
7
```

The 1200 × 500 part fits no 1000 × 600 board, so no board is opened, `sheets used` is `0`, and stderr
carries `the nesting left 1 of 1 instance(s) unplaced: P1 (0 of 1 instance(s)); the document was not
written`. A document authored with no placements and no cost is a **request** and loads (that is the
section's own rule); from the first placement on, the counts must agree.

## What it refuses

Twelve load-time refusals of its own, plus the model's existing duplicate-id rule, each naming the value
it refused rather than a default or silence. The messages below are the `#[error]` templates, so `{…}`
is the value the refusal names.

| Refusal | Message |
| --- | --- |
| `NonPositiveSheetDimension` | `nesting sheet {sheet:?} {field} must be positive, got {value} mm` |
| `DuplicateId` (kind `nesting sheet` / `nesting part`) | the model's existing rule, reused so a lookup by id is unambiguous |
| `NestingPartTakesNoEntities` | `nesting part {part:?} names no entity ids, so there is nothing to cut it from` |
| `NestingPartUnknownEntity` | `nesting part {part:?} references unknown entity {entity:?}` |
| `NestingPlacementUnknownPart` | `nesting placement names unknown part {part:?}` |
| `NestingPlacementUnknownSheet` | `nesting placement names unknown sheet {sheet:?}` |
| `NestingPlacementPositionNotUsable` | `nesting placement of part {part:?} must sit at a non-negative finite point of its sheet, got {x} mm and {y} mm` |
| `NestingPlacementOutsideSheet` | `nesting placement of part {part:?} on sheet {sheet:?} leaves the sheet: its footprint reaches x {x} mm and y {y} mm, the sheet is {width} mm by {height} mm` |
| `NestingPlacementsOverlap` | `nesting placements of parts {first:?} and {second:?} overlap on sheet {sheet:?}` |
| `NestingInstanceCountMismatch` | `nesting part {part:?} asks for {wanted} instances but {placed} placements name it` |
| `NestingCostSheetCountMismatch` | `nesting cost records {recorded} sheets used but the placements name {placed} distinct sheets` |
| `NestingCostFractionOutOfRange` | `nesting cost {field} must be a finite fraction between 0 and 1, got {value}` |
| `NestingCostFractionMismatch` | `nesting cost {field} {recorded} is not the {derived} the document's own placements and sheets derive, within {tolerance}` |

The kernels refuse their own inputs by name too, through the crate's one `GeomError` (M9's T2b folded
the nester's first, private error enum back into it): `GapNotMeasurable`, `SheetNotMeasurable`,
`PartNotMeasurable`, `SheetNotClosed`, `PartNotClosed` and `SearchStepNotMeasurable`, each carrying its
value. A part larger than the board is not a refusal at all — it is `unplaced`.

The command refuses, with exit `2` and the document byte-identical: **a flag its document gives no
meaning to** (`--step` when every part is a rectangle, because the no-fit heuristic never searches a
lattice; `--algorithm` when no part is one, because neither maxrects nor guillotine packs anything);
**a flag value it cannot honour**, naming the flag and the value (`--rotation sideways`,
`--algorithm triangle`, `--gap -1`, `--sheets 1000`, `--sheets 0x600`, `--step 0`); **a flag from
another command**, as `unknown nest option "--cut-depth"`, because a depth nobody applied still looks
applied (M8's rule); **boards of two sizes**, because both nesters pack identical boards and one run
cannot pack two; **nothing to nest** (no section, no parts, and no boards with no `--sheets`); and **a
part whose selection is not one closed contour** — several entities, an unknown entity, a non-contour,
or an open wire — naming the part. `--help` lists the command, its six flags and `7`, and
`the_help_lists_the_nest_command_its_flags_and_its_exit_code` asserts each string.

## The differential, and what M9 did not change

The milestone's differential (L11) is the measurement that says M9 touched nothing it did not own. The cut
point is `deb8b1a` — verified as `69cef77^`, with three seam commits between it and M9's first commit
`cc2d609` — and it was exported with `git archive` into a scratch copy built with its own
`CARGO_TARGET_DIR`, then run against the work tree's binary. Each side ran **42 invocations** from
inside its own sandbox with relative paths, so no sandbox path could reach stdout or stderr: two
`tools add` plus `tools list`; three `import` forms (the report, `--out`, and `--out` with `--layer`
and `--tolerance`); `validate`, `inspect` and `check`; `toolpath --kind` for profile (written back in
place), pocket, engrave and drilling; five `emit` runs and their four programs, to `--out` and to
stdout; `simulate` at 2, 3 and 5 mm; a document carrying a `nesting` section through `validate`,
`inspect`, `check` and a `toolpath` write; `nest` twice; the error paths; and `--help`. Exit code,
stdout and stderr were compared per invocation, and **21 produced files** by sha256 — every document as
its zip bytes and as its `document.json` payload, the four programs, and the library. Determinism came
first: the current binary run twice gave **42/42** identical result triples and **21/21** identical
hashes.

**39 of 42 invocations are byte-identical and 19 of 21 files**, and the three differences are exactly
the three M9 owns. `nest` **did not exist**: exit `2` with `error: unknown command "nest"; see --help`
on the base against exit `0` with an 11-line report here (`1 sheet(s)`, placements at `(0.000, 0.000)`
and `(0.000, 45.000)`, `utilisation: 0.080`, `waste: 0.920`), and a second `nest` invocation that is
that same unknown command against this tree's `error: nest needs parts: the document authors no nesting
section, so there is nothing to nest`, both exit `2`. `--help` **grew, additions only**: 90 lines →
**108**, **18 added and 0 removed**, the `nest` block starting at line 70 and the exit-code-7 line at
108. And the `nesting` section: `nest_roomy.c2d` is the base's unchanged copy (`4233f87c…`) against
this tree's written-back document (`30e0d623…`), while `nesting_tp_out.c2d` — re-saved by a pre-existing
command — is the base **dropping** the section (`141ee2c2…`) against this tree **preserving** it
(`bf6a460b…`); deleting the key makes the two payloads identical, so the section is the whole
difference. Everything else is byte-identical in exit code, stdout, stderr and produced files.

One half of the instance-count relaxation is **not** observable this way, and saying so is the point:
the base's `Document` has no `deny_unknown_fields`, so it ignores the unknown `nesting` field and
`validate`, `inspect` and `check` agree on the parts-with-no-placements document on both sides (exit
`0`, byte-identical). The relaxation shows only where the section is used or written — which is `nest`
placing both instances where the base has no command at all.

## Two review lineages, and the commit neither covered

Both candidates of this milestone were reviewed at tier high and both ended `approved` with the receipt
burned (`native-approved-acknowledgement-completed`, authority `burned`). What they cost is the record:

**`review-1ab521437bba8607`** covered the committed geometry slice `cc2d609..3508eb9` — 2 801 lines,
lenses risk + readability + reliability, correction budget 200. The **full-range** candidate
(`deb8b1a..3508eb9`, 16 paths) was refused first in preflight with `lens_context_budget_exceeded`,
`mutation_outcome: not_started`, no authority created and nothing to abandon; the reduced range was
accepted. That is the same wall M7 hit at 4 425 lines, and the fact worth keeping is that the budget is
a property of the candidate's **size** and the base ref is the lever. Three reviewers ran on the host
relay, the provider demanded a refuter, and the refuter returned `correction_required` with three
CRITICAL findings that were one defect — inside-ness accepting `Touching`, with `certifies` repeating
it, so a plan could certify a part outside a non-rectangular board (§"Inside-ness" above). Fixed in
`033a291` with **161 of the 200 authorised lines**; a targeted validator judged all three findings
`met` with no regressions. Four advisory findings were emitted, informational and later work, tagged by
lineage: `1ab52143/R1-STEP-DOS` (risk, WARNING, `nest_profile.rs:521-522`), `1ab52143/R2-002`
(readability, WARNING, `nest_profile.rs:298`), `1ab52143/R2-003` (readability, SUGGESTION, `nest.rs:348`)
and `1ab52143/R3-002` (reliability, WARNING, `nest_profile.rs:241-246`). The review also named a gap
this milestone owns: every new placement test asserted a refusal, so the **accept** branch of the
fix had no positive case — closed afterwards by `03c4790`, which pins a flush part placed and certified
on a rectangle and on an L's arm and one inset step past the edge refused. A tooling defect was recorded
too: the facade cannot capture this lineage's correction-plan or validation slots (it renders
`--target=b528be6e` while reporting `target_identity: 984997e0` and rejects its own binding twice), so
both ran through the native CLI with the provider-issued tokens.

**`review-33ff9cdba8c8746d`** covered `90d6a1f..55e759e` — 375 lines, lenses **risk + reliability**
(two, not three: the change is a format rule and the tests that pin it, and the docs are comments
explaining that same rule), budget 188 of which **12** were used. The refuter returned
`correction_required` with one finding, `R3-001` (reliability, **BLOCKER**): "the new determinism test
uses an impossible fixture: two 600x400 instances on a single 1000x600 sheet with rotation off cannot
both fit. `nest` will exit 7 with one unplaced, so the assertion expecting exit code 0 fails and the
test never proves idempotence." **The premise was false and the substance was true**, which is the fact
worth keeping: `nest` does not exit 7 there, it **opens a second sheet**, so the assertion held — and
the reviewer could not see that because `main.rs` was outside the candidate's paths and only the tests
were in the diff. What was true is that the fixture proved sheet growth **and** idempotence at once, so
a failure would not have said which. The correction (`e3cd977`, 12 lines) makes the fixture fit one
sheet (two 400 × 400 on the authored 1000 × 600, rotation-invariant so rotation-off is irrelevant), says
in a comment that it must fit one sheet and why, and adds `assert_eq!(after_first.sheets.len(), 1)` —
the assertion that would have caught the ambiguity, because it fails if the fixture ever stops fitting.
A validator judged the finding resolved on the frozen evidence, naming `cli.rs:8228` (the 400 × 400
contour) and `cli.rs:8244-8248` (the one-sheet assertion) as the deciding lines. No advisory findings
were emitted beyond `R3-001`. The same tooling defect blocked this lineage's slots, and the validator's
artifact had to bind `correction_target_identity` (the provider's `5226b038…`) rather than the plain
`target_identity`, the transcription error that got the previous artifact refused.

**Neither range contains `cc2d609`, and that is the milestone's honest gap.** The document format — the
`nesting` section, its refusals and the one utilisation — is the *base* of the first candidate,
which git reads as excluded, and the full-range candidate that would have included it was the one refused
with `lens_context_budget_exceeded`; once HEAD moved past it, that range could not be re-cut. `90d6a1f`,
the `nest` command itself, sits between the two ranges and is covered by neither. So **M9's document
contract rests on its 31 tests in `crates/cam-model/tests/document.rs` and the pre-M9 byte-identical
proof** (`a_document_written_before_nesting_loads_and_re_saves_byte_identically`, plus
`the_nesting_section_does_not_move_the_schema_tag`), and the command on its 16 tests in
`crates/cam-cli/tests/cli.rs`. That is weaker evidence than a lineage, and it is not softened here —
what stands beside it is the differential above, which shows the section surviving a re-save by a
pre-existing command and nothing else on that surface moving at all.

## Known limits, each a decision

- **The no-fit heuristic is incomplete, and the price is measured.** The 100 × 100 board at step 20
  costs a second board for a 50 × 50 square although `(45, 0)` is legal; step 5 finds it, at `(50, 0)`.
  A complete nester would enumerate the no-fit polygon's boundary, and `Region` cannot build one.
- **`Region` has no union, so no exact NFP exists to reach for.** Its whole surface is `new`,
  `contains`, `distance_to_boundary`, `subtract` and `normalize`; the Minkowski difference of two
  outlines needs a union of the placed part grown around each of its own edges and corners. The module
  therefore claims no NFP anywhere, and the lattice is what stands in its place.
- **Contact is refused, so a plan pays one search step beyond the gap.** Measured: the 5 mm-step plan
  lands 10 mm from the bar where the gap is 5, and a zero gap means "further apart than touching". The
  alternative — accepting tangency — would accept a cap lying inside a no-fit region and break the gap
  by an arbitrary amount.
- **A placement's stored footprint is the turned bounding box (D-M9-2).** Measured on the L bracket:
  60 000 mm² of material in a 300 × 400 box reports utilisation `0.200` where its own area would be
  `0.100`. Load-time containment and overlap are conservative for a curved or notched part by design,
  and the exact shape is the nester's business.
- **The gap is a floor between parts and never insets the board's edges (D-M9-7).** A part exactly the
  board's size fits flush on all four edges; `the_certificate_holds_at_the_sheets_own_edges` and
  `a_part_flush_against_a_rectangular_board_is_placed_and_certified` pin it, and `a_flush_part_one_inset_step_past_the_edge_is_refused`
  pins the other side at exactly `INSET_STEP`.
- **The search step is the caller's, and defaults to 10 mm.** Its cost is the incompleteness measured
  above; the report states the step it searched with (`free-profile no-fit heuristic at 10.000 mm`), so
  a reader can see the lattice the plan paid for.
- **A quarter turn is a fallback, not a tie-break (D-M9-8).** The cabinetry plan is all `None`; a
  500 × 900 part on a 1000 × 600 board enters only with `--rotation on`, as a 900 × 500 footprint at
  `0.750` utilisation.
- **A run packs one board size.** Two sizes are refused by name (`sheet "S1" is 1000.000 x 600.000 mm
  but sheet "S2" is 800.000 x 600.000 mm: a nesting packs identical boards…`), exit `2`, because both
  nesters pack identical boards — one plan is one board, repeated.
- **A mixed document nests each class on boards of its own.** Neither nester can be handed the other's
  placements as an occupied board, so a rectangle part and a free profile never share one. Measured by
  the determinism fixture: two panels on `S1`/`S2`, the bracket on a board neither of them names, three
  sheets authored by the run.
- **Exit 7 writes nothing.** Measured: the document's bytes before and after a failed run are equal, and
  the report says `sheets used: 0`, `utilisation: none`. A partial nesting has no honest stored form
  because `Document::validate` refuses a placements-versus-instances disagreement.
- **No shell drafting form** (S7's assumption, carried from M4 and M7): parts, sheets and the run are
  authored through `cam-cli nest`, and the shell's nesting surface is future work rather than a silent
  gap.
- **`docs/format.md` carries no row for the `nesting` section.** The format document describes the
  document, the tool library, the post-processor and the command-line contract, and M9's section is
  documented here and in the feature document instead. T6's edit surfaces did not include
  `docs/format.md`, so the gap is recorded rather than closed: a reader who starts at
  [format.md](format.md#document-c2d) will not find `nesting` there.
- **The document format's commit has no native review** (see the lineage section above): `cc2d609` is
  in neither candidate, so M9's document contract rests on its tests and the pre-M9 byte-identical
  proof.

## Where the record lives

| Question | Where |
| --- | --- |
| What M9 had to be, and every decision behind it | `odd/tasks/m9-nesting.md` — S1 … S7, T1 … T6, L1 … L11 |
| What the `nesting` section stores, and the document schema | this document, and `crates/cam-model/src/lib.rs`'s `Nesting` |
| How the simulation authorises a cut | [m5-simulation.md](m5-simulation.md#the-target-surface) |
| How the depth-variant strategies behave | [m6-toolpaths.md](m6-toolpaths.md) |
| How a V-carve's depth field behaves | [m7-vcarve.md](m7-vcarve.md) |
| How a moulding's cross-section behaves | [m8-moulding.md](m8-moulding.md) |
| What M9 delivered, and what it cost | [odd/roadmap.md](../odd/roadmap.md) §4.12 |
| The project-level summary | [README.md](../README.md) |
