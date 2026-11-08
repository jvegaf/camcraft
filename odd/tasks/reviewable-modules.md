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
| T1 | `crates/cam-ui/tests/shell.rs` (5,894) becomes `tests/shell/` — one module per behaviour area plus a `support` module holding the helpers every test already shares — with **every test name, assertion and helper body unchanged** and the gate green at the same count | S3 S4 S5 | worker | — |
| T2 | `crates/cam-ui/tests/view3d.rs` (2,374) gets the same treatment (scene, chrome, playback, simulation) | S3 S4 S5 | worker | — |
| T3 | `crates/cam-ui/src/view3d.rs` (2,130) splits by concern (projection and scene / chrome and layout / playback) with `impl Shell` blocks spread as needed | S3 S4 S5 | worker | — |
| T4 | `crates/cam-ui/src/chrome.rs` (2,672) splits by surface (app bar, rail, ribbon, task editor, layers) | S3 S4 S5 | worker | — |
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

