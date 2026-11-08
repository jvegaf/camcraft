# Advisory debt: closing the seven that can be closed, re-filing the nine that cannot

Feature document for the sweep the owner asked for after `odd/tasks/debt-advisories.md` L16. Read
**until `## Log`** before touching code; the Log is the record, not the brief.

## Specs

- **S1 (user, verbatim):** "vamos a quitarnos la deuda".
- **S2 (user, verbatim option label, on `39a6d85c/R4-003`):** "Sólo el registro identidad (0 / 1) pasa".
  With zero placements a cost record is accepted only when it says the document used nothing —
  `utilisation 0.0`, `waste 1.0` — and every other in-range fraction is refused by name.
- **S3 (user, verbatim option label, on `94cfd234/R1-001` + `R4-001`):** "Sí, índice + salida
  temprana". The invalid-early-overlap failure path stops paying the whole candidate scan, and the
  pair the refusal names does not move.
- **S4 (user, verbatim option label, on the nine unrecoverable advisories):** "Re-filear con una
  review fresca". They are not closed by editing: a fresh native review of the current tree re-files
  them with new words and new identities, and their status is that review's to set.
- **S5 (repo law, inherited from `debt-advisories.md` S4):** a fix changes the diagnostic or the
  wording it was filed against, never the acceptance of valid input. S2 is the owner's sanctioned
  exception; it is refused nowhere and smuggled in nowhere else.
- **S6 (consequence of S3, not a user string):** the refusal keeps naming the pair with the smallest
  `(earlier, later)` placement indices in document order. An early exit that cannot preserve that is
  not the change S3 asked for. The existing test
  `the_pruned_overlap_check_names_the_same_pair_a_naive_scan_would` is the contract, not a guideline.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | `94cfd234/R2-002`: the reference's justification says strict inequalities are exact "because coordinates are whole millimetres"; the real reason a shared edge is not an overlap is `footprints_overlap`'s `Touching` rule | S5 | parent | |
| T2 | `39a6d85c/R2-002`: `NESTING_COST_SUM_TOLERANCE` no longer governs any sum — it is the absolute tolerance of a fraction against its derivation; rename it and align the doc | S5 | parent | |
| T3 | `39a6d85c/R2-001`: `NestingPart::bounds` and `Document::placement_footprint` enumerate two `None` cases each while the implementation has a third (`as_contour()?`) | S5 | parent | |
| T4 | `39a6d85c/R4-003`: a cost record with no placements to support it survives validation; only the identity record may, by S2 | S2 S5 | parent | |
| T5 | `94cfd234/R3-PRUNE-TOLERANCE`: no test exercises the prune's tolerance term or a pair where the box predicate and the winding predicate could disagree | S6 | parent | |
| T6 | `94cfd234/R3-MULTISHEET-PRUNE`: every test of the pruned check builds one sheet, so bucket keying and the global-minimum pair are unasserted | S6 | parent | |
| T7 | `94cfd234/R1-001` + `R4-001`: the failure path pays the full candidate scan before refusing; an index over placements plus an early exit at the first offending `earlier` index, with the named pair unchanged | S3 S6 | parent | |
| T8 | S4's fresh review: a committed candidate of the current tree, its findings written into this Log **before** the acknowledgement burns the receipt | S4 | parent | |

## Log

- **L1 (user, verbatim):** "vamos a quitarnos la deuda".
- **L2 (the three decisions, verbatim option labels).** `39a6d85c/R4-003`: "Sólo el registro identidad
  (0 / 1) pasa". `94cfd234/R1-001` + `R4-001`: "Sí, índice + salida temprana". The nine unrecoverable
  advisories (`bd99706d` ×6, `1ab52143` ×3, `b5fe4921/R3-001`): "Re-filear con una review fresca".
- **L3 (reconnaissance: all seven are still open on the current tree).** Verified read-only at HEAD
  `70c5004`; every advisory's subject still exists, and two of its line numbers moved.
  (a) `NESTING_COST_SUM_TOLERANCE` is `crates/cam-model/src/lib.rs:1910`, used only at `:1966` and
  `:1971`; the doc `:1903-1909` already says absolute and inclusive, and the sum rule is gone, so
  only the name is stale. (b) `NestingPart::bounds` doc `:977-980` claims two `None` cases while
  `:984` is `document.entity(entity_id)?.kind.as_contour()?.bounds()` — a third path. (c)
  `Document::placement_footprint` doc `:1487-1506` has the same gap at `:1509`. (d) `check_nesting_cost`
  `:1931-1977` returns `Ok(())` at `:1957-1961` when `nesting_utilisation()` is `None`, and
  `crates/cam-model/tests/document.rs:2514` pins that with `0.4 / 0.6` and zero placements. (e) The
  prune is `lib.rs:1850` (`second_box.min.x > first_box.max.x + tolerance`) and `:1853-1854` (y),
  with `tolerance = cam_geom::DEFAULT_TOLERANCE` (`= 1.0e-6`, `cam-geom/src/lib.rs:52`); the four
  tests that reach it (`document.rs:1685`, `:1780`, `:1851`, `:1878`) are all single-sheet and none
  is a sub-tolerance near-miss. (f) The comment the finding quotes is now
  `crates/cam-model/tests/document.rs:1690-1691`, and the rule it omits is `lib.rs:1988`
  (`Touching => first.bounds() == second.bounds()`). (g) The failure path is the `first_offence`
  accumulation at `lib.rs:1840-1875`: it visits every candidate pair whatever the answer.
- **L4 (T1-T3, the wording three, and each says something the code is).** No behaviour moved; all three
  are the advisory's own subject. **T1**: the differential test's justification is now the rule that
  actually accepts a shared edge — `footprints_overlap`'s `Touching` arm refuses only *equal* bounds —
  and says out loud that whole-millimetre coordinates are a property of the fixture, not of the rule.
  **T2**: `NESTING_COST_SUM_TOLERANCE` is `NESTING_COST_FRACTION_TOLERANCE` (`lib.rs:1913`, both use
  sites renamed), and its doc states that the sum is implied rather than checked, which is the part of
  the old name the code had stopped doing. **T3**: `NestingPart::bounds` now names all three `None`
  cases — no entity, an entity the document does not hold, and an entity that is not a contour — with
  the third marked unconstructible while `EntityKind` has one variant, and
  `Document::placement_footprint` enumerates the same third case. `cargo test -p cam-model`: green.
- **L5 (T4, the contract change the owner authorised).** RED first:
  `a_nesting_that_cannot_say_what_it_used_refuses_no_record` became
  `…_loads_only_a_record_that_used_nothing`, and with the old code the `0.4 / 0.6` record and zero
  placements still validated — the assertion failed exactly there. The fix is one line and it is the
  whole rule: `document.nesting_utilisation().unwrap_or(0.0)`, so a document that cannot measure has
  *used nothing* and its derivation is utilisation `0.0`, waste `1.0`. A record claiming anything else
  is then refused by the existing comparison — `NestingCostFractionMismatch { field: "utilisation",
  recorded: 0.4, derived: 0.0 }` — instead of being exempted from it. Both shapes (no sheets; sheets
  with nothing placed) still load the identity record. No fixture in the workspace carried a non-identity
  record with zero placements, so nothing else moved.
- **L6 (T5 and T6: the two gaps, and the measurement that says where the real guarantee lives).** Three
  tests, `crates/cam-model/tests/document.rs`: a sub-tolerance gap accepted on each axis at `0.5 x` and
  at exactly the tolerance; a 10 mm tab inside the notch of an L-shaped part accepted, which is the one
  reachable shape where the box predicate and the winding predicate genuinely disagree (boxes overlap
  10 x 10 mm, material shares only the notch's edges); and the two-sheet test, where coincident copies
  on two sheets load and a document with an offence on *both* sheets names the S2 pair — the earliest in
  document order even though S1 sorts first in the bucket map. **The mutation, and what it measured**:
  the prune's threshold was made ten times stricter
  (`second_box.min.x > first_box.max.x - 10.0 * tolerance`) and the whole `cam-model` suite stayed green.
  That is not a hole in the tests, it is the theorem behind the prune: an offence requires the two boxes
  to meet at all, so any threshold at or above zero is sound, and `DEFAULT_TOLERANCE` is there to drop
  the pairs the relation would answer `Disjoint` for and not to keep the answer right. A test of outcomes
  therefore cannot see the tolerance term, and the guarantee is the argument — which is why the argument
  now sits in the comment above the loop instead of only in the suite. The L13 lesson repeats: a
  boundary nobody can observe behaviourally has to be *argued* in the code, not pinned by a test that
  cannot fail.
- **L7 (T7, the index and the early exit; measured, not claimed).** The sweep iterated pairs in x order,
  so it accumulated the minimum pair instead of returning one: a document with its only overlap at the
  front paid for every candidate behind it. It now walks `earlier` in **document order**, which makes the
  first `earlier` with any partner the one the refusal is about; the partner taken is the smallest
  `later`, so the message is the same pair a naive document-order scan names — the differential test
  (`the_pruned_overlap_check_names_the_same_pair_a_naive_scan_would`) still passes untouched. The index
  is per sheet: placement indices in `min.x` order plus the running maximum of `max.x`, because a box far
  to the left still reaches a placement if it is wide — the prefix maximum is what lets the backward walk
  stop at a binary search instead of a scan. The new test
  `an_early_overlap_ends_the_check_instead_of_scanning_every_later_placement` stacks 5 000 copies on one
  spot: **61.99 s with the early exit removed and 0.05 s with it**, 12 495 000 winding relations against
  4 999. The ceiling is 5 s — loose on purpose, because that is a tripwire and not a benchmark. On the
  valid side each x-meeting pair is examined twice (once from each end) and the relation is asked once,
  so the relation count is what it was; the accumulation is gone.
- **L8 (the gate, the slice boundary, and the sixteenth advisory).** `./scripts/check.sh` ran
  `fmt --check`, `clippy -D warnings` and the whole workspace: **all gates green, `CHECK_EXIT=0`**,
  `crates/cam-model`'s document target at **96 passed** (92 before this work, plus T4's rewritten test
  and T5/T6's three and T7's tripwire). Delivered in three commits on `feat/advisory-debt-close`:
  `189fbe0` (T4 alone), `5a7d770` (T1-T3, T5, T6) and `7ed2ef9` (T7) — **427 authored changed lines**,
  so the slice boundary is here and the documentation rides on top of it rather than inside any of them.
  **And one finding none of the seven predicted.** While building T6's fixture, the honest shape for
  "two placements overlapping on one sheet" — `[0..100]` and `[50..150]` in x, same y extent —
  validated `Ok`. It is not a fixture mistake: `cam_geom::winding::relation` answers `Touching` for two
  rectangles that share 5 000 mm² of interior whenever their horizontal (or vertical) edges are
  collinear, because the classifier never asks whether the interiors overlap once a coincident edge
  exists. The shape with no collinear edge answers `Crossing`, which is why nothing in the workspace or
  in the seeded differential test (whose reference re-uses the same predicate) ever saw it. It reaches
  `check_nesting` — overlapping placements load — and `nest_profile`'s own collision test. Recorded as
  the sixteenth advisory in `odd/tasks/debt-advisories.md` L17, **not fixed**: the fix is shared
  geometry with four consumers and belongs in its own feature with its own review.
- **L9 (T8, the review: six findings, one of them CRITICAL, and it was right).** Lineage
  **`review-ca5c7eda65951600`** over `master` (`c20c7cf1`..`5635855`), tier `high` on a
  `process_boundary` signal, four provider-chosen lenses (risk, resilience, readability, reliability),
  579 original changed lines, correction budget **200**, one refuter run and one targeted validator run
  besides the four reviewers. Four reviewer runs produced **six findings, five against this sweep's own
  new code**. Their words are in Engram observation #193, saved **before** any acknowledgement, and
  again here because the store keeps none of them:
  1. **`R4-LOAD-REGRESSION`** (resilience, **CRITICAL**, deterministic, `introduced`, the only one the
     provider judged blocking): the backward half of the index was bounded by a *running maximum of
     `max.x`*, which one wide placement holds high for every placement behind it, and `consider` did not
     check the candidate's own `max.x`. A valid document with one wide part plus N small ones in another
     y band therefore did **O(N²) winding relations** — the very quadrant L7 claimed to improve.
  2. `R1-001` (risk, WARNING) and 3. `R3-001` (reliability, WARNING): the same defect from two more
     lenses, each prescribing a per-candidate x-interval guard or an interval index.
  4. `R2-001` (readability, WARNING): the two prune-tolerance tests only assert that `validate`
     **succeeds**, so a prune that wrongly skipped the pair would leave both green.
  5. `R2-002` (readability, SUGGESTION): the index is spread over four parallel arrays defaulting to `0`
     and `""`, with nothing enforcing one assignment per footprint.
  6. `R3-002` (reliability, WARNING): the sub-tolerance test cannot detect an unsound prune; add a pair
     that truly overlaps by less than the tolerance and assert the refusal.
  **The refuter corroborated** `R4-LOAD-REGRESSION` (`outcome: corroborated`) and the state went
  `correction_required` with a budget of 200 diff lines and one finding in scope.
- **L10 (the correction, and the two walls in front of it).** Plan declared at **161 diff lines** and
  accepted; the correction is `5635855`: the running maximum is gone, replaced by a **uniform per-sheet
  grid over x** — a candidate is reached only through a cell the placement being tested also occupies,
  with a `visited` stamp so a sheet-spanning box is met once per placement, and **both** cheap interval
  guards (x and y) now run before `footprints_overlap`. Measured by mutation: the finding's own load
  shape (one 100 000 mm part plus 20 000 small ones, authored in reverse x order) takes **9.70 s with
  the bound collapsed and 0.36 s with the grid**, and the tripwire test asserts under 5 s. The
  **targeted validator passed**, in its own words: "the defective prefix-max/x-sorted backward walk is
  deleted entirely… zero winding relations are computed instead of the measured 12,495,000", plus "no
  detection regression" (any pair whose boxes overlap in x within tolerance shares a cell, because the
  padded insert and query ranges are both monotone in `cell_of`) and "no refusal-ordering regression"
  (the outer loop still climbs `earlier` in document order and takes the minimum `later`). Closure
  **`approved`**, receipt burned (`gentle-ai.review-acknowledged/v1`, consumed revision
  `sha256:65a470aa…`).
  **Two walls, both worth recording.** (a) The facade **rejected its own provider-issued
  correction-plan binding** twice — `collectBinding is missing or stale for current STATUS` — while the
  native CLI accepted the identical tokens, which is the defect `odd/tasks/debt-advisories.md` L2's
  predecessor already recorded for M10. (b) The native CLI then answered `managed_assets_outdated`, and
  the owner authorised `gentle-ai sync`; it took **two** syncs, the PATH binary's and then the harness's
  own `…/gentle-pi/.gentle-ai/v4.4.0/gentle-ai sync --agent pi`, because the store had recorded the
  first binary's asset digest and the second binary then saw itself as stale. After that the plan, the
  validation and the acknowledgement all went through the offered routes.
- **L11 (what the review left open, which is not a reason to re-review anything).** Five findings are
  **informational** by the provider's own statement and none reopens the candidate:
  `R1-001` and `R3-001` are the load regression's other two lenses and are **answered** by `5635855`
  (both called for the guard the grid now carries); `R2-001`, `R2-002` and `R3-002` are **open**, small
  and real, and they are the sweep's own successor advisories — the tests that assert only acceptance
  (`R2-001`, `R3-002`) and the four parallel arrays that would silently read a wrong sheet
  (`R2-002`). They belong to a later change with its own candidate.
