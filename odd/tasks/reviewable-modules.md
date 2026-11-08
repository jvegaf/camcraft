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
- **S3 (the target, restated in L8 because the first version was unachievable).** No file under
  `crates/cam-ui/src` or `crates/cam-ui/tests` that a M13 concern touches exceeds **~800 lines**, so a **future**
  candidate over that concern carries a few hundred lines of file context instead of thousands. Measured after
  the five slices, in today's files: the 3D half's concern is ~1.2k (`view3d/` 573 + `tests/view3d/` 456 +
  `lib.rs` 131), the shell's chrome ~1.5k (`chrome/` 700 + `tests/shell/` 676 + `lib.rs` 131), and T1's
  post-and-write concern ~0.5k (`main.rs` 332 + `lib.rs` 131 + `tests/shell/` 38) — against the 8.2k and 14k
  that refused M13's candidates. **The original wording asked for a successful `review.start` on `0c194cb`,
  `c4da057` and `a078015`; L8 records why that cannot be the test.**
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
| T6 | ✅ **delivered as a measurement** (see L8): the three historical ranges **cannot** be re-reviewed — a review freezes the candidate's own tree, which still holds the pre-split files — so the milestone's proof is the forward context measured per concern, recorded here with its numbers | S3 | inline | — |
| T8 | ✅ **delivered** — 71 → **0** unresolved links and the doc build 78 → 12 warnings; two names became code spans because a private item in a module whose header resolves at the crate root cannot be linked without code (`SPAN_MM`, `Capture`); `Group::History`, a variant that never existed, now points at `Group::File` | S3 S4 | worker | `5af825d` |
| T8-old | The 71 unresolved intra-doc links the seven slices created, plus `Art`'s sentence that points at a header its file does not have — re-pointed with `cargo doc` as the proof (count before and after), no code change | S3 | worker | — |
| T9 | ✅ **delivered** — the three sentences M13 made false now name what exists: the Toolpaths task authors a toolpath, the 3D half simulates the drawing's own toolpaths, and the empty half says *press Simulate in the picture*. One test assertion moved with its subject (`panels.rs:77`) | S4 | inline | — |
| T10 | ✅ **delivered** — light `Cut` `#a86200` → **`#8b5100`**, the lightest value under the luminance cap the deepest stop imposes (0.11528; `#8c5200` breaks it at 2.99:1). Every existing ratio improved (panel 4.76→6.41, canvas 4.51→6.08, lit face 3.05→4.11, deepest stop 2.25→**3.03**), the uncovered pair became a proof that walks five depths, and the workspace went 1607 → **1608** by adding it | S4 | worker | — |
| T11 | Two follow-ups T10 named and did not take: the light `CutSoft` wash was never the premultiplication of the signal (29% lighter even before) and is now further from it; and `design/system.html` + `design/camui-shell/04-tokens.html` were **already stale before the split** — neither carries T4's `#e8cb9d`/`#cfad7b`/`#664d2e` at all | S4 | owner | — |
| T7 | ✅ **delivered** — the one M13 concern still without a review path is split: `strategy.rs` 1,137 → `strategy/` (mod 532, cards 256, editor 169, list 119, quick 104) and `tests/strategy.rs` 1,285 → root 30 + seven modules (largest 378); `tool_library.rs` 1,234 → `tool_library/` (dialog 514, drawing 229, family 155, form 128, shell 100, tree 94, mod 52) and `tests/tool_library.rs` 1,179 → root 28 + seven (largest 371). **`icons.rs` 2,135, `theme.rs` 945 and `tests/input.rs` 881 stay whole by decision** (L9). The only three files over 800 left in `cam-ui` | S3 | worker | `86e9947`, `f0d74a1` |

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
- **L8 (T5 delivered, and the acceptance criterion corrected by measurement).** `lib.rs` 3,738 → **131** plus
  `shell/` (ten files, largest `mod.rs` 511) and `actions/` (two, largest `handle.rs` 609). Public paths proved
  stable by compiling a scratch crate against the built rlib with every name the integration tests use: zero
  errors. 296 cam-ui tests, 136 shell, 47 view3d, **1607** workspace, gate exit 0. Deviations
  recorded: **32 markers**, fifteen of them `Shell` fields now `pub(crate)` — forced, because a private field in
  the crate root was crate-visible by construction and seven sibling modules read those fields (87 compile
  errors before the first one); the keyword records the reach they already had. And `cargo doc` went 45 → 69
  unresolved intra-doc links, every one of them a link that only resolved because the root had every name in
  scope.

  **The correction, and it is the milestone's most useful output.** S3 originally demanded a successful
  `review.start` for `0c194cb`, `c4da057` and `a078015` as the proof. That test is **unachievable**, and the
  reason is structural: a review freezes **the candidate's own tree**, so the range `89ed3b3..c4da057` still
  carries `view3d.rs` 2,130 and `tests/view3d.rs` 2,374 — files that exist only as history now. Splitting today
  cannot retroactively shrink a tree that was committed yesterday, and re-running those three STARTs would fail
  identically and cost nothing but would prove nothing either. What the five slices actually change is the
  **forward** context, and that is measurable today: a future change to the 3D half carries ~1.2k instead of
  8.2k, to the chrome ~1.5k instead of 14k, to the post-and-write path ~0.5k. The milestone's real proof is a
  candidate that does not exist yet, so this document states the numbers and lets the next review be the test.
  **And the limit**: the crate still holds seven files above ~800 lines that no task here named — T7 lists them,
  because a promise of "every file under 800" would be false.
- **L9 (T7 decided: split the one that matters, keep the three that do not).** The owner took the recommendation
  verbatim — *"seguimos tu recomendacion"* — so the last two surfaces M13 still could not review get the same
  treatment: `strategy.rs` (1,137) with `tests/strategy.rs` (1,285), and `tool_library.rs` (1,234) with
  `tests/tool_library.rs` (1,179). Together that is the ~3.1k a toolpath-list candidate was still carrying, and it
  is the one remaining M13 concern without a review path.
  The other three stay whole **on purpose**: `icons.rs` (2,135) is a glyph catalogue whose diff is data,
  `theme.rs` (945) is a table of colours and the proofs that hold them — and it is already reviewed, in T4's own
  lineage, at 945 lines — and `tests/input.rs` (881) sits just over the line. A milestone that claims "every file
  under 800" would be lying; this one claims "every file a reviewer of behaviour must read".
- **L10 (the milestone's numbers, final).** Seven slices: `tests/shell.rs` 5,894 → largest **676**;
  `tests/view3d.rs` 2,374 → **456**; `src/view3d.rs` 2,130 → **573**; `src/chrome.rs` 2,672 → **700**;
  `src/lib.rs` 3,738 → **131** (children ≤ 609); `src/strategy.rs` 1,137 → **532**; `src/tool_library.rs`
  1,234 → **514**. `find crates/cam-ui -name '*.rs' | xargs wc -l` now reports exactly **three** files over
  800 — `icons.rs` 2,135, `theme.rs` 945, `tests/input.rs` 881 — which are the three T7 and L9 decided to leave
  whole, so the claim this document can stand behind is "every file a reviewer of behaviour must read is under
  ~800", not "every file".
  Every slice passed the same bar and paid the same two debts: a **move proved by comparison** (each original
  item verbatim in exactly one new file, and the non-blank line multiset identical apart from declarations,
  `impl` wrappers and forced visibility markers), and **`cargo doc` links that no longer resolve** — 30 at the
  start of the milestone, **71** now, every one of them a link that resolved only because a single file held
  every name in scope. A doc-only `use` cannot pay it (`unused_imports` under `-D warnings`), so it is one
  documentation task's work: re-point the links, and fix `Art`'s "see this module's header", which now points at
  a file that has none.
  **What remains unproven is the only thing that matters**: no candidate has been reviewed since the split. The
  measurement says the forward context dropped from 8.2k to ~1.2k for the 3D half and from 14k to ~1.5k for the
  chrome, but a shorter list of files is not an approval, and this document does not claim one.
- **L11 (the split landed and the review still refused — so the threshold is the finding).** After the seven
  slices, `review.start` over the `tool_library` split alone (`baseRef 86e9947`, one commit, sixteen files,
  **~2.5k lines of file context** across `tool_library/` 1,272 and `tests/tool_library/` 1,199) was refused at
  `preflight` with `lens_context_budget_exceeded` again: `mutation_performed: false`, no authority, and — worth
  saying — **zero tokens**, because the stop happens before any lens context is built. Together with T4's pass
  that brackets the budget: **~1.2k of file context is reviewed end to end; ~2.5k is refused.**
  Three consequences, and the third is the one that matters:
  1. "No file over ~800" was necessary and **not sufficient**. A candidate also may not touch many files at once:
     fourteen files of 150 lines cost the same as one of 2,100.
  2. **A move of this size cannot be reviewed at all** — which is the paradox of this milestone: the refactor that
     makes behaviour reviewable is itself too big to review. Its proof is not a reviewer's verdict but a
     mechanical one, and every slice carries it: each original item verbatim in exactly one new file, the
     non-blank line multiset identical apart from declarations and visibility markers, and the gate green at the
     same 1607 tests.
  3. What the budget can hold is a **change**, not a **move**: T4 (280 changed lines over 1.2k of files) is the
     shape that fits, and that shape is what the seven slices now make possible for the shell's own surfaces.
  The recommendation this leaves for the owner is therefore narrow and honest: keep RDD for behaviour, and treat
  pure moves as gate-and-comparison evidence rather than review subjects — or disable it for a move with
  `gentle-ai review mode disable --scope clone`, the provider's own `D` route, and re-enable it for the next
  behaviour change.
- **L12 (the RDD policy for pure moves, decided).** The owner took the recommendation: **RDD stays on for behaviour
  and is not spent on moves.** A move's truth is arithmetic — every original item verbatim in exactly one new file,
  the non-blank line multiset identical apart from declarations and visibility markers, and the gate green at the
  same test count — and that is stronger evidence than a reviewer's verdict on a diff nobody can hold. A move that
  cannot fit the budget is not a reason to disable anything: it simply does not become a review candidate. When a
  candidate *does* change behaviour, RDD applies as before, and T4 is the proof that it earns its cost (it found a
  CRITICAL no test could see). If a future move's diff ever needs a reviewer's opinion rather than its own
  arithmetic, the provider's `D` route (`gentle-ai review mode disable --scope clone`, then `enable`) is the
  escape hatch — used around that move, never as a standing setting.
  The consequence is a rule this repository now writes down instead of rediscovering: **a work unit is a candidate
  only if it fits ~1.2k of file context; a move is proved by comparison; a behaviour change under that budget is
  reviewed.**
- **L13 (the three debts reopened as tasks rather than left as prose).** T8 is the 71 doc links, T9 the two
  `chrome/job.rs` sentences M13's own simulation made false, T10 the paper amber at 2.25:1. Each has a location and
  a number; none of them is a reason to reopen the milestone.
- **L14 (T8 and T9 delivered; the debt paid, and the mechanism named).** `cargo doc --document-private-items`
  went **71 → 0** unresolved links (build 78 → 12 warnings) over two rounds and twenty-one files, and the
  filtered diff — every changed line that is not `///`, `//!` or `//` — is empty, which is what S4 asked of a
  documentation task. Two links were not fixed but dissolved: `SPAN_MM` and `Capture` name items private to a
  module whose header resolves at the crate root, so reaching them needs a visibility change, and a
  `pub(crate) use` written so rustdoc can resolve a link is code written for documentation — refused once
  before (L6) and refused again here. Their sentences are untouched; the names are code spans.
  **The mechanism is the finding**: rustdoc resolves a module header in the **parent's** scope when `lib.rs`
  puts an outer `///` on the `pub mod` declaration — the outer and inner docs merge and the header loses the
  module's own names. That one rule is 18 of the 20 breaks, and it also corrects L6/L7/L8/L10: **20 of those 71
  predated the milestone entirely**, in six files `git log ea58baf^..HEAD` shows untouched. A milestone that
  claimed to have created them all would have been wrong, and the number is now separable: 51 created by the
  splits, 20 inherited.
  T9 then closed the second debt in four sentences and one assertion: `chrome/job.rs` said a toolpath "comes
  from `cam-cli toolpath` and lands here finished", `ribbon.rs` said 3D "plays a program that `cam-cli`
  produced and a host handed in", and the empty half said "no program loaded — simulate one with `cam-cli
  simulate` and hand it in". M13 made all three false, and the fix names what now exists: the Toolpaths task,
  the drawing's own simulation, and the `Simulate` control in the picture. `tests/shell/panels.rs:77` moved
  with its subject, which is the only test line that changed.

