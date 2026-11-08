# Reviewable modules — the shell must fit in a reviewer's context

Feature document opened by the owner's decision that closed M13's review story: the provider refused three of
M13's four candidates with `lens_context_budget_exceeded`, and splitting the *commits* cannot fix it because the
cost is the **files**. Read **until `## Log`** before touching code.

## Specs

- **S1 (the owner's decision, verbatim):** "3". The three options put in front of them were (1) disable RDD for
  this clone, (2) record the three units as unreviewed, (3) **split the modules first and then review**. Three was
  chosen, so this milestone exists and the review switch stays on.
- **S2 (the measurement that forced it).** A native review's lens context is built from the **files a candidate
  touches**, not from its hunks. Two data points decide everything below: T4 (`7ed58b6`) touched `theme.rs` (945
  lines) and `design/system.md` (254) — **~1,200 lines of file context** — and was reviewed end to end, approval
  and burned receipt. `c4da057` (T2) touched `view3d.rs` (2,130), `tests/view3d.rs` (2,374) and `lib.rs` (3,738) —
  **~8,200 lines** — and was refused at `preflight` with `lens_context_budget_exceeded`, `mutation_performed:
  false`, no authority created. The provider's own continuation: *"split this change into a chained sequence of
  smaller reviewable commits, each under the budget"*. T1 (~14,000) and T3 (~8,800) are worse. **Splitting a
  commit does not help when the file is the cost.**
- **S3 (the target, and it is falsifiable).** No file under `crates/cam-ui/src` or `crates/cam-ui/tests` exceeds
  **~800 lines**, and every file a M13 candidate touches is small enough that its **cumulative** context stays
  under the one measurement that is known to pass (~1,200 lines) with room for the diff. The proof is not a
  number in this document: it is a **successful `review.start` for `0c194cb`, `c4da057` and `a078015`**, each of
  which failed before.
- **S4 (behaviour is untouchable).** This milestone moves code and nothing else. No signature changes, no
  renames beyond what a move forces, no new tests, no deleted tests, no assertion edited. The gate is the proof:
  **1607 tests, the same names**, before and after every slice — and where a move needs a helper to leave a
  private module, the visibility change is the only line that differs.
- **S5 (order, and why).** The test files go first: `tests/shell.rs` (5,894) is both the largest file and the
  evidence a reviewer reads, and a test file is the safest thing in the repository to move because nothing
  depends on it. Then `tests/view3d.rs` (2,374), then the source in descending size — `view3d.rs` (2,130),
  `chrome.rs` (2,672), `lib.rs` (3,738).
- **S6 (what this is not).** Not a redesign, not a rename pass, not a chance to fix the two `chrome.rs` sentences
  that still name `cam-cli`, and not a reason to touch the 44 advisories. A moved line is a moved line.

## Tasks

| ID | What | S | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ **delivered** — `crates/cam-ui/tests/shell.rs` (5,894) became a **38-line root** plus fifteen behaviour modules under `tests/shell/` and a 474-line `support`; largest file **676**. 136 tests with the same names, workspace **1607**, gate green | S3 S4 S5 | worker | — |
| T2 | ✅ **delivered** — `crates/cam-ui/tests/view3d.rs` (2,374) became a **31-line root** plus `support` and six concern modules (scene 413, playback 421, playback_bar 389, split 330, simulation 221, overlay 146); largest file **456**. 47 tests, same names, workspace **1607**, gate green | S3 S4 S5 | worker | — |
| T3 | ✅ **delivered** — `crates/cam-ui/src/view3d.rs` (2,130) is now `view3d/`: `mod.rs` 328 (public paths + the five unit tests, whose names stayed `view3d::tests::*`), `scene` 443, `playback` 562, `chrome` 278, `shell` 573. Largest file **573**. Nine `pub(super)` markers, no `pub(crate)`. Workspace **1607**, gate green | S3 S4 S5 | worker | — |
| T4 | ✅ **delivered** — `crates/cam-ui/src/chrome.rs` (2,672) is now `chrome/` with nine files: `mod` 178, `app_bar` 159, `palette` 368, `ribbon` 692, `layers` 18, `status` 44, `job` 700, `overlays` 169, `paint` 429. Largest **700**. Every `chrome::` call site preserved, six `pub(super)` markers, no new `pub(crate)`. Workspace **1607**, gate green | S3 S4 S5 | worker | — |
| T5 | `crates/cam-ui/src/lib.rs` (3,738) splits into the action table, the shell state, and the frame | S3 S4 S5 | worker | — |
| T6 | The proof S3 asks for: attempt `review.start` for `0c194cb`, `c4da057` and `a078015`, record each outcome, and — where one still fails — record the file and the number that refused it | S3 | inline | — |

## Log

- **L1 (the owner's decision, verbatim).** "3" — the third of the options this session offered after the
  provider refused M13's second candidate: *"(3) Partir los módulos primero (`view3d.rs`, `chrome.rs`, `lib.rs`,
  y sobre todo `tests/shell.rs`) en unidades de ~800 líneas, y recién entonces revisar."* Recorded as S1.
- **L2 (what the refusal actually said).** `lens_context_budget_exceeded`, `phase: preflight`,
  `mutation_performed: false`, `authority_applicability: not_evaluated`, `retry_safe: true`, `next_action: stop`
  — and a continuation that asks for smaller candidates. No authority was created by either refusal, so nothing
  had to be abandoned or repaired, and neither attempt cost a token: the stop happens before the lens context is
  built.
- **L3 (the review switch stays on, and the reviews are deferred to T6 by the owner's own decision).** Receipt-driven
  development is still enabled and the worktree keeps producing unreviewed candidates — the feature document, and
  then each slice the worker lands. Neither is reviewed, and the disposition is S1 rather than an oversight: the
  owner chose to **split first and review after**, so the review of this milestone's work happens once at T6, over
  the three M13 candidates that failed, where a passing `review.start` is the milestone's acceptance criterion
  (S3). Reviewing an intermediate slice would freeze a half-split file, which is the exact failure mode this
  milestone exists to cure, and would spend a four-run review (measured: 144,780 tokens, US$0.235) on a tree that
  the next slice moves again.
- **L4 (T1 delivered, and the one deviation).** `5,894 → 38` lines at the root, fifteen areas (`authoring` 647,
  `rulers` 676, `tool_library` 645, `save` 600, `files` 504, `program` 362, `palette` 330, `material` 315,
  `panels` 287, `help` 257, `status` 250, `script` 187, `layout` 164, `view` 141, `document` 125) and a
  `support` module of 474 lines holding the 24 helpers. **Largest file 676.** `cargo test -p cam-ui --test
  shell` reports **136 passed** before and after, `--list` gives the same 136 names, and the workspace stays at
  **1607**. Every original top-level item was moved with a block-level byte comparison and the multiset of the
  5,445 non-blank lines is identical — the only textual differences are the `pub(crate)` on the shared helpers,
  two signatures rustfmt re-wrapped (bodies diffed identical), the per-module `use` blocks, and the root's
  declarations.
  **The deviation, measured rather than assumed:** a crate root at `tests/shell.rs` does **not** resolve
  `mod x;` to `tests/shell/x.rs` — rustc looks for `tests/x.rs`, which would also collide with the existing
  `tests/palette.rs` and `tests/tool_library.rs` targets. The root therefore carries
  `#[path = "shell/<area>.rs"]` on each declaration. The canonical alternative is a root at
  `tests/shell/main.rs` with bare `mod` lines, which this task forbade; switching later is deleting `shell.rs`
  and moving the same sixteen lines, with the area files untouched.
- **L5 (T2 delivered, and the two comments that moved for a reason).** `2,374 → 31` lines at the root, `support`
  456 plus six modules, **largest 456**. `cargo test -p cam-ui --test view3d` reports **47 passed** before and
  after, the same 47 names once the module prefix is stripped, and the workspace stays at **1607**. Proved the
  way T1 was: 79 non-`use` top-level items each verbatim in exactly one module (0 placement problems) and the
  multiset of 2,183 non-blank content lines identical, with 33 `pub(crate)` lines as the only textual difference.
  **Two comment blocks changed position rather than moving with their file**, and the reason is worth keeping: a
  section banner and a `///` sentence sat above a test they did not document (a pre-existing mis-attribution —
  `colours_in_the_3d_panel`'s doc described the colour audit while textually preceding the clock test), and the
  S8 banner preceded a fixture instead of heading the split-chrome tests. Both now sit above what they describe,
  line for line. S6 says a moved line is a moved line; this is the one place where leaving it would have moved a
  lie, and it is recorded so the reviewer reads it as a correction rather than as a smuggling.
- **L6 (T3 delivered; the source half of the split, and its one real cost).** `view3d.rs` 2,130 → `view3d/` with
  five files, **largest 573**: `mod.rs` keeps every public path (`clock_text`, `seconds_for`, `bar_fill`,
  `progress_text`, `speed_text`, `next_speed`, `vector_lines`, the four constants, `Layer`, `Layers`,
  `Preview3d`) and deliberately keeps the module's five unit tests so their full names stay `view3d::tests::*`;
  `scene` holds projection and painting, `playback` the state and the clock, `chrome` the marks, `shell` the
  `impl Shell` surface whole. Proved by comparison: 102 items each verbatim in exactly one file, 0 placement
  problems, and the multiset of 1,998 non-blank content lines identical apart from the four extra `impl`
  wrappers and three rustfmt re-wraps whose bodies were diffed. Workspace **1607**, gate exit 0.
  **The cost, recorded rather than glossed:** nine intra-doc links in the module header no longer resolve —
  `Job`, `Role`, `Role::StockEdge`, `Role::Border`, `Role::Cut` twice, `Role::TextDim`,
  `Palette::stock_shade` and `build_surface` now live in child modules, so `cargo doc` goes from 30 to 39
  unresolved-link warnings. The wording was not touched (S4 forbids it) and a doc-only `use` is not available:
  `unused_imports` fires on imports referenced only by doc comments, and the gate is `-D warnings`. L27 already
  recorded rustdoc as not-a-gate with 34 such links at `2a09faa`, so this is house tolerance — but it grew, and
  a later documentation task should re-point those links.
- **L7 (T4 delivered; the largest surface, split where the code already broke).** `2,672 → 2,757` across nine
  files, **largest 700**, `mod.rs` 178 holding the header, the declarations (`Tab`, `Category`,
  `ToolSubject`) and the re-exports with their original visibility. The boundaries are the surfaces the file
  already had — app bar, rail and palette, ribbon and task editor, layers, job and material, overlays, and one
  `paint` module for the widgets they share. Proved like the rest: 51 top-level items and 41 `impl Shell`
  methods each verbatim in exactly one file, and the non-blank line multiset differs only by the six extra
  `impl Shell {`/`}` wrappers. Six `pub(super)` markers, each required by a sibling, none reaching outside
  `chrome`; the twelve `chrome::` call sites in `lib.rs`, `strategy.rs`, `tool_library.rs` and the two
  integration tests resolve unchanged, which the build proves.
  **One doc block documented two items** — `section_toggle`'s paragraph had no blank `///` before `Glyph`'s, so
  lines 2138–2148 formed one block above `enum Glyph` while `section_toggle` carried no doc at all. The seam is
  where the text itself breaks, and the two halves concatenate byte-for-byte back to the original: T2's L5
  precedent again, no word changed.
  **Debt that grew, recorded:** `cargo doc --document-private-items` goes from 43 to 45 unresolved links. Both
  new ones are the price of the move — `[Command]` in `mod.rs`'s header (a module that holds no code cannot
  carry a doc-only `use`) and `[Shell::material_dialog]` in `paint`'s `field` doc (which does not import
  `Shell`). L6 established rustdoc as not-a-gate; a documentation task re-points them together with L6's nine.

