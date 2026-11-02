# cam-geom-arc-consistency — `intersect` answers for circles when asked about arcs

Two functions of the kernel disagree about the same pair of partial arcs. One of them is wrong. Fix it, pin it,
and stop the gate from writing a seed into a tracked file on a case that should never have been reachable.

## Specs

- **S1 — the goal.** The owner's words, in order: *"ya he mergeado yo todo e hice limpia, por donde seguimos"*, then
  the choice *"El defecto del kernel (recomendado)"* over the alternative features, with this document's own
  description as the requirement: *"Doc propio. Arregla la discrepancia entre intersect y segment_to_segment, y saca
  la seed del archivo versionado"*. The review of the closing documentation commit was declined explicitly
  (*"No lo revises (mi recomendación)"*), so this document starts no review lineage of its own.
- **S2 — the defect, as the record already measured it.** From `odd/tasks/camui-toolpath.md` L10, verbatim: *"the gate
  is flaky for a reason that predates this feature: `cam-geom --test distance`'s
  `the_distance_is_zero_exactly_when_the_segments_meet` finds two concentric arcs where `intersect` answers
  `Coincident` while `segment_to_segment` answers 7.071"*. Reproduced as a rule, not an anecdote: `7.071` is
  `5·√2`, the exact distance between the endpoints of two disjoint quarters of one radius-5 circle.
- **S3 — which function is wrong, and the one-sentence reason.** `intersect::arc_arc`
  (`crates/cam-geom/src/intersect.rs:213-218`) returns `Coincident` from the *supporting circles* — same centre
  within tolerance and same radius within tolerance — and never consults the two sweeps. `Coincident` is documented
  as *"They overlap along a non-zero length"* (`intersect.rs:27-28`) and `is_coincident()` as *"share more than a
  point"* (`intersect.rs:47-49`); two disjoint quarters satisfy neither, so `intersect` contradicts its own contract.
  `distance::arcs_meet` (`crates/cam-geom/src/distance.rs:246-248`) decides the same configuration by the sweeps
  (`sweeps_share_angle`, `distance.rs:268-277`) and is right. **The arc interval, not the circle, is the subject.**
- **S4 — what must not change.** Full circles on one circle still coincide: `concentric_circles_either_coincide_or_miss`
  (`crates/cam-geom/tests/intersect.rs:195-203`) pins `Coincident` for two closed radius-5 contours and `None` for a
  radius mismatch, and it must keep passing unchanged. `line_line` coincidence
  (`identical_segments_are_coincident`, `partially_overlapping_collinear_segments_are_coincident`) is untouched.
  `segment_to_segment`'s answers are the reference and are not edited to match `intersect`. No new dependency, no
  change to `c2d/document@2`, no change to any string an operator reads.
- **S5 — the test's premise is corrected, never hidden.** The proptest's own docstring
  (`crates/cam-geom/tests/distance.rs:407-410`) asserts that *"the generator draws distinct centres and radii so that
  case is not reachable"*. `any_arc` (`distance.rs:317-352`) has no such guard, so the claim is false. The fix makes
  the case **arbitrated** and then makes the docstring true; filtering the generator is the one repair this document
  rejects, because it would delete the evidence instead of answering the question.
- **S6 — the tracked seed gets an argued fate.** `crates/cam-geom/tests/distance.proptest-regressions` is tracked and
  carries one pinned seed (`cc 53f0ee71…`). A gate failure appends to it, which is a second way to dirty the tree.
  Whether that seed is kept or removed is decided by running it, never by deleting a file that might be masking a
  case.
- **S7 — risk.** This changes `cam_geom::intersect`'s answer for a public function of a shared crate (Task Size item
  3) and is pinned by tests (item 6, because the current suite does not catch the regression). It gets its own
  commit and its own native review candidate. No call site outside `cam-geom` exists today; six in-crate consumers
  branch on `Coincident` (`boolean.rs:165-175`, `winding.rs:248-275`, `trim.rs:125-126`, `trim.rs:287`,
  `medial.rs:973-975`, `validate.rs:82-96`, `region_offset.rs:212-215`) and each must be re-read against the new
  answer.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | Pin the defect with a RED test: two disjoint quarters of one radius-5 circle are `√50` apart and do not meet. Both assertions in one case, so the two functions are compared and not each trusted alone | S2 S3 | inline | `e12dc61` |
| T2 | Fix `intersect::arc_arc`'s concentric branch to decide by sweeps, reusing the angular-coverage rule this module already owns (`intersect.rs:277-283`) instead of copying `distance`'s | S3 S4 | inline | `e12dc61` |
| T3 | Preservation, not hope: full circles still `Coincident`, partially overlapping quarters of one circle still `Coincident`, radius mismatch still `None` | S4 | inline | `e12dc61` |
| T4 | Correct the proptest's false premise and give the tracked seed an argued fate, measured by running it | S5 S6 | inline | `e12dc61` |
| T5 | Re-read the six in-crate `Coincident` consumers against the new answer; run the gate; record the evidence | S4 S7 | inline | `e12dc61` |

## Log

- **L1 (owner, verbatim).** *"ya he mergeado yo todo e hice limpia, por donde seguimos"* → the choice *"El defecto
  del kernel (recomendado)"*. After the merge of `camui-toolpath` into `master` (`d806f8d`) and the branch cleanup,
  the next feature opened here is the kernel discrepancy the previous feature had to record as somebody else's debt.
- **L2 (the diagnosis, before any code).** A read-only exploration measured the disagreement in the tree, and every
  claim below is `path:line` in this document rather than an inference. **The two decisions**: `intersect::arc_arc`
  (`intersect.rs:213-218`) answers from the supporting circles and never reads a sweep; `distance::arcs_meet`
  (`distance.rs:246-248`) answers from the sweeps via `sweeps_share_angle` (`distance.rs:268-277`). **On the same
  exact premises** (`|C2−C1| ≤ ε`, `|r1−r2| ≤ ε`) they reach opposite conclusions, so this is not a tolerance-scale
  argument: the generator's centres and radii are integers (`distance.rs:337-343`), concentricity is exact, and the
  sweeps are multiples of `π/4` (`distance.rs:318-335`), so a disjoint pair cannot be rounded into an overlapping one.
  **Blast radius, measured rather than assumed**: outside `cam-geom` there is *no call site* of `intersect` or
  `segment_to_segment` — only prose hits in two ODD documents; `cam-toolpath` reaches them indirectly through
  `region.offset`/`raster_spans`, and `region_offset`'s single `intersect` consumer
  (`region_offset.rs:212-214`) is always handed a `Segment::Line`, so the arc/arc path cannot reach toolpath
  emission today. **What the suite sees**: `concentric_circles_either_coincide_or_miss` (`tests/intersect.rs:195-203`)
  pins full circles only, and `two_concentric_circles_are_the_radius_difference_apart` (`tests/distance.rs:130-147`)
  pins disjoint sweeps only with *different* radii — the equal-radius disjoint case is pinned nowhere, which is why a
  proptest was the only thing that ever found it.
  **One thing this entry does not claim:** the seed currently pinned in
  `distance.proptest-regressions` (`cc 53f0ee71…`) is an arc-plus-line case, not the two-arc one, so it is not
  evidence of this defect. T4 decides its fate by running it.

- **L3 (T1 and T2: the defect pinned, the answer moved to the sweeps, and a bug this entry caught in its own
  diff).** The RED came before the fix and said exactly what L2 predicted:
  `two_quarters_of_one_circle_that_share_no_angle_do_not_meet` failed with `left: Coincident, right: None` while
  the same test's first assertion measured `7.071067811865475`. **The fix is one branch and three helpers:**
  `arc_arc`'s concentric arm now returns `None` for a radius mismatch and delegates to `one_circle` otherwise;
  `one_circle` compares the two angular coverages and answers `Coincident` only for a shared **length**, `Point`
  for a single shared angle, `None` for no shared angle; `coverage` turns a signed sweep into intervals of
  `[0, TAU]`; `angle_apart` is the shortest unsigned angle between two angles.
  **T2 planned to reuse `arc_contains_angle` and did not, and the reason is the whole point of the task.** That
  helper decides whether a *point* lies on an arc, with the tolerance slack already folded in, so it cannot
  separate "touches at one angle" from "shares a length" — which is exactly the distinction `Coincident` is
  documented to carry. Reusing it would have reproduced the same class of error one level down.
  **A bug in that first version, found by reading the diff and not by a test.** `coverage` took
  `[start, start + |sweep|]`, so a **negative** sweep — an arc drawn clockwise — was reported as covering the
  quadrant *after* its start instead of the one before it. Every test this feature had written used positive
  sweeps, so the suite was green; the distance proptest draws signed sweeps (`distance.rs:318-335`), so the bug
  would have surfaced there as a brand-new flake. The fix keeps `(start + sweep, start)` for a negative sweep,
  and `a_clockwise_quarter_covers_the_angles_it_runs_back_through` pins it — **RED observed by reintroducing the
  old line on purpose** (`right: Coincident`) and then restoring the fix. Recording it because a green suite
  proved nothing here; reading the code did.

- **L4 (T3 and T4: what is preserved, what the proptest was allowed to keep claiming, and the seed).**
  Four behaviour tests, each one rule: disjoint quarters do not meet (with `5√2` as the second assertion, so the
  two functions are *compared* rather than each trusted), overlapping quarters share a length, quarters sharing
  one angle meet at that point (including the pair that meets across the seam at angle 0), and a full-turn arc
  covers every quarter — reachable because `Arc::new` clamps a sweep at `TAU` (`arc.rs:32-37`) and rejects more.
  `concentric_circles_either_coincide_or_miss` (`tests/intersect.rs:195-203`) still passes **unedited**, which is
  the preservation S4 demanded; `tests/intersect` went 18 (master) → **23 passed, 0 failed**.
  **The proptest's docstring was the second false statement in this story, and it is corrected rather than
  satisfied by a filter.** It claimed the generator draws distinct centres and radii; `any_arc`
  (`distance.rs:317-352`) guards nothing. It now says the case is reachable and arbitrated, and that the
  generator stays unfiltered on purpose: a distinctness guard would delete the evidence instead of answering it.
  **The pinned seed was stale and is gone.** `cc 53f0ee71…` names an arc-plus-line pair, not two arcs; the file's
  own record could not explain it, and `cargo test -p cam-geom --test distance` passed at HEAD *with* it, so it
  pinned a case that no longer fails. It was removed by running it first, never by assuming it was noise — S6's
  whole rule. The file keeps its proptest template header.

- **L5 (T5: the consumers, and the gate this sandbox would not run until the linker was told to use one thread).**
  **Four of the six consumers can see an arc/arc answer and were re-read one by one; two cannot, and that is a
  property of their arguments rather than a hope.** `medial::seam_is_clear` compares `Segment::Line` against
  `Segment::Line` (`medial.rs:970-973`), and `region_offset`'s single `intersect` call is always handed
  `Segment::Line` first (`region_offset.rs:212`), so `arc_arc` is unreachable from both. The four that can:
  `boolean.rs:170` used to set `re_cut = true` for disjoint same-circle arcs without ever adding a split point
  (the other arc's ends are not interior to this one) and now falls to `None`; `winding.rs:270` used to answer
  `Touching` for two arcs that share no point and now answers nothing (`winding.rs:248`); `trim.rs:126` and
  `trim.rs:287` skipped cutting a pair that needed no cut either way, so their behaviour is unchanged; and
  `validate.rs:85` used to report a self-intersection at the first arc's start for a pair that does not intersect.
  **Every suite that touches them is green, measured per binary** because the workspace link is what this session
  could not do in one shot: `validate` 10, `winding` 9 (`boolean` is reached through `region.rs:181`), `trim` 7,
  `region_offset` 17, `medial` 14, `region` 16, `region_distance` 11, `medial_coverage` 3, `intersect` 23,
  `distance` 14 at 4 000 cases.
  **The gate, and the environment that had to be told twice.** `scripts/check.sh` on this session's cgroup aborted
  inside `rust-lld` (`std::__throw_system_error` → `ld terminated with signal 6`) on targets unrelated to this
  change: `cam-ui`'s `shell`, `cam-sim`'s `oracle`, a `proc-macro2` build script. The cause is measured, not
  guessed: this shell runs in a service cgroup with **`pids.max = 64`**
  (`/sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/app.slice/gentle-shell-*.service`) against ~45
  tasks in use, and lld wants more threads than the remainder allows. The same defect took out the native review
  runtime earlier in the session, which is a Go binary and shares no code with any of this. **With
  `RUSTFLAGS="-C link-arg=-Wl,--threads=1"` the whole gate runs: exit 0, 96 suites, 1 171 passed, 0 failed, 0
  ignored**, which is the previous record's 1 166 plus exactly the five tests T1 and T3 added. The flag is a
  *linker* argument and changes no compiled semantics; it is recorded here so nobody reads 1 171 as a different
  build of the code. `cargo clippy --workspace --all-targets -- -D warnings` exits 0 with no warnings, and `cargo
  fmt --check` is green — the second only after the config this entry had to restore.
  **One incident belongs to the repository and not to this fix.** `cargo fmt --check` failed on a **pristine
  `git archive HEAD` tree** with 2 909 diffs and the repository has no `rustfmt.toml`: every crate is two-space and
  rustfmt wanted four. With `tab_spaces = 2` the entire tree is clean, so the config was simply never versioned and
  the gate could not have passed for anyone. The owner chose to restore it, and that is `4ddbb72` — a separate
  commit, because it is repository hygiene and not this defect.
  **Not claimed here:** the gate was run with a different `RUSTFLAGS` than the 1 166 run, so it proves the tests,
  not that a bare `scripts/check.sh` passes in an unconstrained shell.

- **L6 (independent verification: seven specs met, three of this document's own numbers wrong, and the case nobody had
  pinned).** A read-only verifier that was told the log is not evidence re-derived every claim. **Verdicts: S1-S7
  all `met`, no blockers, no correction round.** Its own oracle — angle coverage computed from boundary midpoints
  instead of the interval list `intersect` builds — swept 308 same-circle arcs (23 starts including `-TAU` and
  `TAU + π/8`; 10 sweeps from `π/8` to `±TAU`) over **94 864 ordered pairs: 0 mismatches** against the expected
  `Coincident`/`Point`/`None` split of 66 922 / 7 478 / 20 464, and the same grid asserted
  `(distance == 0) == !intersect.is_empty()` on every pair. That is the answer this entry wanted and could not give
  itself: the two functions now agree everywhere the grid can see.
  **It also found the case the original proptest needed a million draws to reach,** and at `master` it reproduces:
  `first = Arc(center (-5,11), r 1, start π, sweep π/2)`, `second = Arc(center (-5,11), r 1, start π/4, sweep -π/2)`
  — `distance 0.7653668647301798 and intersect Coincident disagree`. At HEAD `intersect` answers `None` and the
  reference distance is unchanged, which is the sharpest form of "one function was wrong, not two".
  **What it corrected in this document, and these are corrections, not quibbles:** L4 said `tests/intersect` went
  **19 → 23**; master is **18**, and 19 was this feature's own intermediate RED state — the honest statement is
  18 → 23, and it is now what this document says. L5 listed `region` 11 and `region_distance` 16 **transposed**
  (they are 16 and 11), and cited `medial.rs:968-972` where the `intersect` call is at `:973`. All three are fixed
  in place rather than annotated, because a record that keeps a wrong number beside the right one teaches the next
  session to trust neither.
  **The seed's fate is now measured in both directions**: restoring `cc 53f0ee71…` gives 14 passed at HEAD *and* at
  master, so it was already stale before this change and removing it lost nothing; and the master failure appended a
  **new** seed by itself (`cc 34160a92…`), which is the second failure mode S6 exists for.
  **S7 held under an independent read**: no `cam_geom::intersect` or `segment_to_segment` call outside the crate;
  `cam-view`'s `segments_cross` (`pick.rs:129`) is a local cross-product helper, not this function; `medial` and
  `region_offset` cannot reach `arc_arc` because their first argument is a `Segment::Line`, as L2 claimed.
  **Two advisories this document does not act on, recorded so nobody rediscovers them as bugs:**
  `Intersections::Point` is documented as "a transverse crossing" (`intersect.rs:24-26`) while a shared endpoint is
  not transverse — the new arc/arc answer uses `Point` there, matching what `collinear_overlap` already did for
  lines (`intersect.rs:142-153`), so the stale part is the enum's wording and not this change. And an overlap
  smaller than the angular tolerance answers `Point`, the same convention `collinear_overlap` uses for
  `overlap > tolerance`, observable only at a `1e-10` radian overlap.
  **What the verifier did not verify, correctly stated:** the workspace-wide gate (96 suites / 1 171 passed),
  `clippy --workspace` and `fmt --check`. It ran `-p cam-geom` only and said so, which is why L5's hedge about a
  bare `scripts/check.sh` stays exactly as it is.

- **L7 (the oracle becomes a test, and this entry corrects the size this document first claimed for it).** The
  verifier's grid was scratch; the rule deserves a committed one. `546220f` appends two oracle helpers and three
  tests to `tests/intersect.rs`: **190 arcs** (19 start angles — `k·π/8` for `k` in `0..16`, plus `-π`, `-TAU` and
  `TAU + π/8` — crossed with 10 sweeps from `π/8` to `-TAU`), **36 100 ordered pairs**, asserting per pair that the
  answer matches the midpoint-measure oracle, that it is the `Coincident`/`Point`/`None` the oracle classifies, and
  that `segment_to_segment == 0.0` exactly when `intersect` is not empty. It runs in 0.05 s. Two cases keep their
  names: the pair the proptest found at `master` (distance `0.7653668647301798`) and a full turn against a sweep too
  small to matter, both directions.
  **A correction against this document's own brief, made before anyone could read it as cover:** the grid the
  parent asked for was described as 308 arcs / 94 864 pairs, which was the *verifier's* wider scratch sweep (23 start
  angles) and not the set the parent actually specified. What is committed is the smaller, precisely enumerated grid
  above. The number in L6 belongs to the verification run and to nothing else.
  **One advisory from the writer worth keeping, because it is a boundary and not a doubt:** the oracle's thresholds
  (`measure > 1e-6`, and `1e-12` slack in `angle_covered`) are not provably equivalent to the kernel's
  `angular = TOL/radius + 1e-12` for arbitrary inputs. They agree on this grid only because every non-zero overlap in
  it is a multiple of `π/8`, far above both thresholds. A future refinement that makes overlaps arbitrarily small
  would have to reconcile them; today's rule does not need it.
  **The gate, re-run after these tests:** exit 0, **96 suites, 1 174 passed, 0 failed, 0 ignored** — the 1 171 of L5
  plus exactly the three added. Same `RUSTFLAGS="-C link-arg=-Wl,--threads=1"` caveat as L5, and for the same
  measured reason: the session's cgroup still caps tasks at 64.
  **Slice boundary, so the size is not misread.** Five commits on `fix/cam-geom-arc-consistency`: `e12dc61` (the fix
  and its five tests, 215 authored lines) and `546220f` (the oracle, 159) are the **work unit**, 374 lines together;
  `4ddbb72` (`rustfmt.toml`) is repository hygiene that this defect needed in order to be gated; `da5e978` and the
  commit carrying this entry are the record. Nothing in the work unit can be split further without leaving a commit
  whose tests do not run.

- **L8 (the review, and it approved without a correction).** The owner asked for it explicitly after the verifier
  came back clean. Lineage **`review-66c509705f3d15fe`**, target `sha256:d9aa7b29…`, tier `medium`, lenses
  **`review-risk` + `review-reliability`**, 7 paths / 584 changed lines, correction budget **200**. Result:
  **approved**, no correction round opened, no blocker, and the budget spent is zero. The group forecast named 2
  model runs over the Pi host relay; both were prepared and submitted, the risk lens returned no finding, and the
  authority is **burned** (`gentle-ai.review-acknowledged/v1`, consumed revision `sha256:e54afb34…`).
  **Two advisories came back, both `informational`, both from `reliability`, neither of them a correction:**
  `R3-001` at `crates/cam-geom/src/intersect.rs:301-307` and `R3-002` at `crates/cam-geom/tests/intersect.rs:484-490`.
  Their **text is not quoted here on purpose** — it lives in the native receipt, and a document that retypes a
  finding from memory is the failure mode the previous feature's L11 named. What can be said without inventing
  anything is where they are: the first is the `touch → Point` construction for a single shared angle, the second
  is the oracle's own classification threshold (`measure > 1.0e-6`). The review's own statement about them is the one
  that matters for the next session: *none opened a correction, none reopens this review, and no correction
  transition is offered for this candidate; treat them as separate later work, never as a reason to re-run review on
  this candidate.*
  **What this receipt does not authorise:** delivery. Commit, push, pull request and merge stay ordinary repository
  policy, and the branch is still five commits sitting on top of `master` with nothing published.
