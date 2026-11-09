# The sixteenth: `Touching` must not be reported when the interiors overlap

Feature document for advisory **`ca5c7eda`… the sixteenth** recorded in `odd/tasks/debt-advisories.md`
L17 and `odd/tasks/advisory-debt-close.md` L8. Read **until `## Log`** before touching code.

## Specs

- **S1 (user, verbatim):** "mergea primero y despues la arrancamos".
- **S2 (user, verbatim option label, on where the fix lives):** "Ruta B: arreglar en la raíz".
- **S3 (the measured defect, reproduced before the fix):** `cam_geom::winding::relation` over
  `[0,0]-[100,100]` and `[50,0]-[150,100]` answers `Relation::Touching`, though the two rectangles
  share 5 000 mm² of interior. `crates/cam-geom/src/winding.rs:270` sets `touching = true` for
  `Coincident | Tangent` and the function returns `Touching` before containment is ever probed, so two
  placements that overlap in area load without a refusal.
- **S4 (what `Touching` must mean afterwards, derived from the enum's own doc):** the boundaries meet
  and **no interior of one crosses into the other's**: material may envelop material (an inscribed
  circle, a part flush inside its board) but material may not overlap *partially* across a shared span.
  The four cases that keep `Touching`: a shared edge with the interiors on opposite sides, a tangency,
  identical contours, and a containment at a tangency (the inscribed-circle fixture). Said the other
  way, and this is the sentence the enum carries: a shared span with the material on the **same** side
  is `Crossing` **only when neither contour encloses the other**.
- **S5 (the rule, and why each clause is there):** when no proper crossing was found, a coincident
  segment pair whose two segments run so that both interiors fall on the **same** side of the shared
  line means the interiors overlap there. The side is decided from the two segments in hand plus each
  contour's own orientation (`Contour::signed_area`), never by assuming counter-clockwise: for a
  contour of positive area the interior lies to the left of each directed edge, for a negative one to
  the right, so the interiors coincide iff the two directions agree **exactly when the two orientations
  agree**.
  Same-side alone is not enough to escalate, and neither is a single probe. Two guards, both required:
  - **Containment first.** If any vertex of either contour is `Inside` the other, one contour is
    (or may be) inside the other, and the pair keeps `Touching` — this is the flush-part fixture
    (`nest_profile.rs:616-620`), whose opposite corners are inside the sheet.
  - **A decisive witness.** `probe` is the *first* vertex (`winding.rs:331`), so one point can answer
    `OnBoundary` for a flush pair or `Outside` from the large contour's side alone; a single probe
    therefore proves nothing. Escalation needs **no vertex of either contour `Inside` the other** and
    **a vertex of each strictly `Outside` the other** — a two-sided witness, which together say neither
    contour encloses the other. The one-sided form is wrong: a part flush against its board has every
    vertex `OnBoundary` while the board's far corners are `Outside` the part, so "some vertex of either
    is `Outside` the other" escalates exactly the fixtures S6 protects.
  Overlapping interiors with no containment are exactly `Crossing`. Anything ambiguous — every vertex
  `OnBoundary`, as with identical contours — keeps `Touching`.
- **S6 (repo law, unchanged):** a fix changes the diagnostic or the wording it was filed against,
  never the acceptance of valid input, and a pinned fixture is a contract. The four `Touching` cases of
  S4 are acceptance: they must still load, and `crates/cam-geom/tests/nest_profile.rs:616-620` (a part
  flush against the board) must stay green untouched.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered `360936f` — same-side coincidence + two-sided `Outside` witness | S3 S4 S5 | parent | `360936f` |
| T2 | ✅ delivered `360936f` — five cases pinned, 12→14 relation tests | S3 S4 S6 | parent | `360936f` |
| T3 | ✅ delivered `360936f` — two consumer tests; flush fixtures untouched | S4 S6 | parent | `360936f` |
| T4 | ✅ delivered `85abd7b` — `Touching` sentence + ledger entry | S4 | parent | `85abd7b` |
| T5 | ✅ delivered — `assess` reported `reviewDue: false` (medium, under budget); no outside view | S1 | parent | `85abd7b` |

## Log

- **L1 (user, verbatim):** "mergea primero y despues la arrancamos", and on the route: "Ruta B:
  arreglar en la raíz".
- **L2 (the merge that preceded this, for the record).** `feat/advisory-debt-close` merged into
  `master` as **`c8fa776`** (`--no-ff`), branch deleted, post-merge gate green (`fmt`,
  `clippy -D warnings`, whole workspace, `CHECK_EXIT=0`). **Not pushed.**
- **L3 (reconnaissance: the naive fix was measured and is wrong).** An explorer mapped the classifier
  and its four callers. What it found, all read-only, is what S5 is built on:
  (a) **`intersect` does not expose the overlap span** — `collinear_overlap` computes it and discards
  it, returning a unit `Coincident` (`crates/cam-geom/src/intersect.rs:104-124`), and nothing reports
  which side the interiors lie on. So the classifier must decide from the two segments it already
  holds.
  (b) **`probe` is the first vertex** (`winding.rs:331`), i.e. a boundary point, so `classify` answers
  `OnBoundary` for exactly the pairs in question — which is why S5 needs a *decisive* witness and why
  the ambiguous answer must not escalate.
  (c) **Three tests pin `Touching` where the interiors overlap**: `identical_contours_touch`
  (`tests/relation.rs:77-81`), `a_circle_tangent_to_the_inside_of_a_square_touches` (`:70-74`) and
  `crates/cam-model/tests/document.rs:427-429` (two islands sharing `x = 40`). Two more pin the flush
  part as legitimate (`crates/cam-geom/tests/nest_profile.rs:616-620`, `:676-680`). **No test pins the
  partial collinear overlap** — the defect is unpinned in `tests/relation.rs`.
  (d) **`Coincident ⇒ Crossing` outright would break the flush part**, because that fixture is
  geometrically identical to the defect pair on the shared edge; the difference is global containment,
  which is why S5's witness is a containment probe and not a local side test alone.
  (e) **There is no shared-area primitive** to reuse: no area intersection exists, and
  `boolean::normalize` is an arrangement pass that can refuse and can flatten arcs
  (`crates/cam-geom/src/boolean.rs:99-133`). The classifier must not depend on it.
  (f) Callers that would need attention: `cam-model`'s `footprints_overlap` (`lib.rs:2039`) is the one
  that is **wrong today**; `nest_profile::inside_sheet` (`:617`) and `region_offset::nesting_depths`
  (`:249`) distinguish `Touching` and must be re-checked; `cam-model`'s island pair check (`:2316`)
  and `nest_profile`'s `overlaps` (`:657`, `:666`) already treat `Touching` as refused, so `Crossing`
  changes nothing there; `cam-toolpath`'s inlay (`:189`) reads only `FirstInside`.
- **L4 (T1 and T2, and the two places the outline was wrong).** The classifier now records a
  **same-side coincidence** — interior side from each segment's direction plus each contour's own
  `signed_area` sign, never assuming counter-clockwise — and escalates to `Crossing` only when neither
  contour encloses the other. RED first: `a_partial_collinear_overlap_is_a_crossing` failed with
  `left: Touching, right: Crossing` before the source change. `crates/cam-geom`'s `relation` target went
  12 → 14; the whole `-p cam-geom` run is **311 tests, 0 failed**.
  **Outline error 1, corrected from evidence:** I wrote the witness as "some vertex of either is
  `Outside` the other", which is wrong — a part flush against its board has every vertex `OnBoundary`
  while the board's far corners are `Outside` the part, so the one-sided reading escalated three pinned
  `nest_profile` fixtures. The witness must be **two-sided**: a vertex of *each* strictly outside the
  other, which is what "neither encloses the other" actually means.
  **Outline error 2:** I asked for a diamond inscribed in a square with equal bounds to be `Crossing`;
  it cannot be. Its bounds equal the square's, so it lies inside it, and it meets the square only at
  four vertex-to-edge points — a containment at a tangency, which S4 keeps as `Touching`. The test
  pins that, and pins equal bounds as non-decisive with a square that has a corner bitten out.
- **L5 (T3, and the fixture that had been lying).** Two consumer tests: `cam-model`'s document now
  refuses the S3 pair (`two_placements_that_overlap_with_collinear_edges_are_refused`, naming P1/P1 on
  S1), and `region_offset` refuses two boundaries whose material coincides
  (`boundaries_that_share_a_span_with_material_on_the_same_side_are_refused`) instead of quietly
  contributing no nesting depth. `nest_profile`'s flush, L-board and coarse-step fixtures stay green
  **untouched**, which is S6's contract. And the workspace gate found the fix's own footprint: the load
  tripwire from `odd/tasks/advisory-debt-close.md` L10 had 20 000 ten-millimetre squares **4.5 mm
  apart**, so its copies overlapped each other and validated only because of this defect — the exact
  shape the sixteenth describes, hiding inside a test written to measure something else. Spaced 11 mm
  and given a 300 000 mm sheet, and the comment now says why.
- **L6 (T4, T5 and how this closes).** T4 is the `Relation::Touching` sentence, which now says what S4
  requires: "They meet without crossing and no interior of one lies inside the other… A shared span
  with the material on the *same* side is `Crossing`, not this." T5 is **not due**: `assess` over
  `c8fa776..HEAD` reports `nativeRisk: medium`, 6 paths, 378 changed lines, `reviewDue: false` on
  `under_budget`, and a plan of writer self-verification with no independent verifier. RDD is on and
  the provider's tier decides, so this candidate gets no outside view — and the record says so instead
  of implying one.
  **Verification, and its honest limit.** `./scripts/check.sh` on the whole workspace is green
  (`fmt --check`, `clippy -D warnings`, every target, `CHECK_EXIT=0`), and every test that passed
  before this change still passes: the suites are the same files on both sides, so a golden, a CLI
  output or a region/nest/inlay behaviour that moved without a new test would have failed here. What
  was **not** run is the scratch-base differential the shared-code rule asks for — a copy of `c8fa776`
  built in an isolated directory and driven through `nest`, `toolpath` and `simulate` — because those
  commands take documents that the CLI's own tests author inline and no checked-in fixture exists to
  hand to both builds. The consequence is stated rather than hidden: for `winding::relation`, the only
  answers that changed are the ones five new tests pin, and any other change would have to be one that
  no existing target observes.
- **L7 (the follow-ups, and the difference between a test that can fail and one that cannot).** The three
  open advisories of the "acceptance-only test" family are closed by replacing the guarantee's location,
  not its wording. Two tests: a tolerance-scale differential against an unpruned pairwise reference
  (`the_pruned_overlap_check_agrees_with_an_unpruned_scan_at_the_tolerance_scale`, 24 seeded documents
  with jitter at `0, 0.5t, t, 2t`) and a refusal on the far side of the boundary
  (`an_overlap_of_twice_the_tolerance_is_refused_on_either_axis`). **Both were proved able to fail**:
  mutating the grid query to a single cell gives `FAILED. 0 passed; 2 failed`. And the opposite
  measurement, which is the one worth keeping: removing the tolerance padding from both cell ranges
  makes **neither** differential fail, because an offence requires the boxes to meet and a meeting pair
  shares a cell even unpadded. The padding is defensive; no outcome test can see it; the code comment now
  says so instead of claiming coverage it does not have.
