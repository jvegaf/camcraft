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
  and **no interior of one lies inside the other**. The four cases that keep `Touching`:
  a shared edge with the interiors on opposite sides, a tangency, identical contours, and a
  containment at a tangency (the inscribed-circle fixture).
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
    **some vertex of either `Outside` the other**, which together say neither encloses the other.
  Overlapping interiors with no containment are exactly `Crossing`. Anything ambiguous — every vertex
  `OnBoundary`, as with identical contours — keeps `Touching`.
- **S6 (repo law, unchanged):** a fix changes the diagnostic or the wording it was filed against,
  never the acceptance of valid input, and a pinned fixture is a contract. The four `Touching` cases of
  S4 are acceptance: they must still load, and `crates/cam-geom/tests/nest_profile.rs:616-620` (a part
  flush against the board) must stay green untouched.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | The classifier: a same-side coincidence plus a decisive `Outside` witness is `Crossing`; direction compared through each contour's own orientation sign, never assumed counter-clockwise | S3 S4 S5 | parent | |
| T2 | The unit tests the class lacked: the partial collinear overlap (S3), the flush part (must stay `Touching`), identical contours, the inscribed tangency, and two different contours sharing bounds | S3 S4 S6 | parent | |
| T3 | The consumers, each with its own test: `footprints_overlap` refuses the S3 pair; `cam-model`'s island rules still refuse what they refuse today; `nest_profile::inside_sheet` still accepts the flush part; `region_offset::nesting_depths` still refuses only a true crossing | S4 S6 | parent | |
| T4 | The docs: `Relation::Touching`'s own sentence states S4, and the ledger records the sixteenth closed | S4 | parent | |
| T5 | The review of this candidate | S1 | parent | |

## Log

- **L1 (user, verbatim):** "mergea primero y despues la arrancamos", and on the route: "Ruta B:
  arreglar en la raíz".
- **L2 (the merge that preceded this, for the record).** `feat/advisory-debt-close` merged into
  `master` as **`7717a44`** (`--no-ff`), branch deleted, post-merge gate green (`fmt`,
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
