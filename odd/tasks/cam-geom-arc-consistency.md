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
  the preservation S4 demanded; `tests/intersect` went 19 → **23 passed, 0 failed**.
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
  `Segment::Line` (`medial.rs:968-972`), and `region_offset`'s single `intersect` call is always handed
  `Segment::Line` first (`region_offset.rs:212`), so `arc_arc` is unreachable from both. The four that can:
  `boolean.rs:170` used to set `re_cut = true` for disjoint same-circle arcs without ever adding a split point
  (the other arc's ends are not interior to this one) and now falls to `None`; `winding.rs:270` used to answer
  `Touching` for two arcs that share no point and now answers nothing (`winding.rs:248`); `trim.rs:126` and
  `trim.rs:287` skipped cutting a pair that needed no cut either way, so their behaviour is unchanged; and
  `validate.rs:85` used to report a self-intersection at the first arc's start for a pair that does not intersect.
  **Every suite that touches them is green, measured per binary** because the workspace link is what this session
  could not do in one shot: `validate` 10, `winding` 9 (`boolean` is reached through `region.rs:181`), `trim` 7,
  `region_offset` 17, `medial` 14, `region` 11, `region_distance` 16, `medial_coverage` 3, `intersect` 23,
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
