# camui-toolpath — the shell learns which tool cuts, and how long it takes

The last two patterns of the reference screenshots that Campilot cannot take: the **current-tool card** at the
top of a toolpath form, and the **estimated cutting time**. Both are blocked by the same thing — the shell has no
access to the cutting tool library, and `cam-app` has no vocabulary for a toolpath at all.

## Specs

- **S1 — the goal, from the reference.** `design/Screenshots-reference/README.md` rows `03-27-41` (the
  `Novo percurso — Chanfro` flyout) and `03-24-32` (the per-item form with the estimated time). The owner's
  authorisation for this feature is recorded in L1 below.
- **S2 — the card's fields are exactly the frame's.** Read off the frame, verbatim: **`T4 - Fresa V 90°`**,
  then **`Fresa V (V-bit) · Ø 12,7 mm · 90° · 2 corte(s)`**, a duplicate `Ø12,7  90°` on the glyph, and the button
  **`Escolher outra ferramenta…`**. That is number, name, type, diameter, included angle and flute count — and
  nothing else. The frame shows **no material, no machine, and no feed, plunge or spindle rate on the card**, so
  the card must not grow them: the numbers a person reads there are the ones the shape needs, not the ones the
  machine has.
- **S3 — resolution is reused, never re-implemented.** The number becomes a tool through the same rule the
  emitter uses. `ToolLibrary::tool_for_number` (`cam-store/src/tools.rs:544-573`) takes no material and no
  machine, and `Document` carries neither, so the card cannot claim to show "the rates for this material".
- **S4 — the library is a snapshot, not a second connection.** `ToolLibrary` owns a rusqlite `Connection` and is
  not `Clone` (`cam-store/src/tools.rs:213-216`), so the shell receives `Vec<StoredTool>`; `StoredTool` is
  `Clone`. No I/O of its own, no default path, no process-wide singleton.
- **S5 — the estimate is published arithmetic, not new arithmetic.** The seconds exist: `move_seconds`
  (`cam-ui/src/view3d.rs:705-754`) already implements the law — plunge rate for a Z-only move, feed rate
  otherwise, arcs along their own circle, helix as the hypotenuse — and `MoveStream::cut_length`
  (`cam-move/src/lib.rs:267-292`) measures the path. A second time formula would be a second opinion.
- **S6 — no new external dependency, and no new edge that closes a cycle.** `cam-store/src/lib.rs:9` forbids that
  crate depending on `cam-toolpath` or any UI crate, so `cam-ui → cam-store` is a straight downstream edge.
  `cam-toolpath` must not gain a `cam-store` edge; the `StoredTool → ToolSpec` mapping belongs where both types
  are already known.
- **S7 — authoring is the missing layer, not the missing button.** `cam_app::Command`
  (`cam-app/src/command.rs:113-380`) has 19 variants and none concerns a toolpath; authoring today exists only as
  `cam-cli toolpath` (`cam-cli/src/main.rs:637`), which pushes a `Toolpath` with its `Operation` directly. This
  feature adds the vocabulary below the UI; a form without it would be a dead control.

## Tasks

| ID | Task | Specs | Route | Risk note |
| --- | --- | --- | --- | --- |
| T1 | A pure `stored_tool_for_number(&[StoredTool], u32) -> Option<&StoredTool>` in `cam-store`, used by `ToolLibrary::tool_for_number` and by the shell so the two cannot disagree; `cam-ui` gains the `cam-store` edge; `Shell::set_tools(Vec<StoredTool>)`; the current-tool card in the inspector, showing exactly S2's fields plus an explicit "T7 is not in the library" state | S1 S2 S3 S4 S6 | worker | — |
| T2 | Publish the seconds: a `pub fn seconds_for(program: &MoveStream) -> f64` lifted out of `move_seconds`/`steps_for`, and the estimated cutting time beside the card, formatted by the `clock_text` T11 added | S1 S5 | worker | — |
| T3 | The toolpath vocabulary in `cam-app`: `AddToolpath`, `SetToolpathToolNumber`, `SetToolpathOperation`, `DeleteToolpath`, each with a refusal that names the number or the entity, plus the `next_toolpath` counter if paid for | S7 | worker | **item 3** — `cam_app::Command` is a contract others consume; its own commit and its own review candidate |
| T4 | `cam-toolpath::program_for(&Document, &[ToolSpec]) -> Result<MoveStream, _>`, the body moved out of `cam-cli::build_segment` so the estimate describes the document's toolpaths and the preview stops being `cam-desktop`'s sample | S5 S6 S7 | worker | **item 3** — the emitter's law moves crates; differential against the CLI's own output |

T1 and T2 are independent and can land in either order. T3 needs T1's list to pick a number from. T4 is what makes
T2's estimate *about the user's document* instead of about `cam-desktop`'s hard-coded `sample_tool()`.

## Log

- **L1 (owner, verbatim).** The two dispositions this feature rests on: *"dale con todo"* (the three fronts after
  the merge) and *"autorizado"*, given to the parent's question — *"¿se autoriza meter `cam-store`/`cam-toolpath` en
  `cam-ui` y abrir autoría de toolpaths en el shell?"* Both were declined earlier for a measured reason and are now
  granted: T14 was recorded as unauthorised in L39/L40 of `camui-shell.md` precisely because the shell could not
  reach the library and the number would have had to be invented.
- **L2 (evidence, the map).** A read-only exploration measured four things that decide the plan. `ToolLibrary` is a
  rusqlite connection opened by path, not `Clone` (`cam-store/src/tools.rs:213-216`), holding `StoredTool
  {geometry, cutting}` (`:205-210`) assembled from `tool_entity` joined to `tool_geometry` (`:125-149`) and
  `tool_cutting_data` (`:152-174`). A toolpath carries only `tool_number` (`cam-model/src/lib.rs:830`) and the
  document only a **filename** in `tools_ref` (`:846-848`); the resolution to feed and plunge lives in
  `cam-cli::build_segment` (`cam-cli/src/main.rs:1881-1910`), which is private to that binary and returns `Failure`.
  `cam_app::Command` has no toolpath variant (grep for `toolpath` over `crates/cam-app` is empty). The seconds exist
  but are private (`cam-ui/src/view3d.rs:705-754`) and reachable only by constructing a `Preview3d`, which needs a
  `Heightfield`; the shell's program today is `cam-desktop`'s sample (`cam-desktop/src/main.rs:153-190`), unrelated
  to `Document.toolpaths`.
- **L3 (the riskiest assumption, falsified by reading the frame).** The exploration's plan wagered that "no field of
  the current-tool card needs data `tool_for_number` does not return", and flagged that if the card named a material
  or a machine, T1 would become a schema change requiring `cutting_data_for` plus a document-side material field.
  The frame settles it against that fear: the card shows the tool's identity and its shape's two numbers, and no
  material, machine or rate at all. **T1 stays read-only.** The card is smaller than the first sketch, not larger —
  the sketch proposed showing feed, plunge and spindle, and the frame does not show them (S2).
- **L4 (a capability that failed, and why it did not become a guess).** The delegated explorer could not render the
  frame: its `find` fails with `failed to spawn thread: WouldBlock` on this machine, `read` on a directory is
  `EISDIR`, and the README elides the filename. It refused to answer the card's fields from the caption, because
  those questions are about pixels. The parent read the PNG directly instead of accepting a provisional verdict.
  Worth remembering for this harness: **an explorer here cannot necessarily list a directory**, so any brief that
  depends on discovering a filename should either hand the exact path over or expect to do that step itself.

- **L5 (T1 delivered, and the table's order changed for a real dependency).** `ad2676a`, gate **1 139 tests across
  93 suites** by the parent's own run. The resolution rule now exists once — `stored_tool_for_number` in `cam-store`,
  with `ToolLibrary::tool_for_number` calling it — so the emitter and the card agree by construction instead of by two
  copies of a query. The card shows the toolpath's name, then the tool's number and name, then type, diameter, included
  angle and flute count, and **nothing the frame does not show**. Differential over nine CLI steps against a build of
  the base commit: identical exit codes and byte-identical stdout, stderr and `out.nc`, including two tools sharing T1
  (so the ordering rule is observable) and the refusal naming an absent tool.
  **Two gaps stated, not filled.** `cam-desktop` passes an **empty** snapshot: the shell opens no `.c2d` files and
  `Document.tools_ref` is a filename only `cam-cli` resolves, so any default path would have been invented. And
  `tool_type` is a `u8` whose mapping `cam-store` documents as unestablished, so the card draws the raw code rather
  than a table nobody evidenced. Both are the kind of hole a later slice should fill with a decision, not a guess.
  **The table's order was wrong about T2 and T4.** The estimated time cannot be shown honestly before the shell can
  turn *the document's* toolpaths into a program: today the only program in the shell is `cam-desktop`'s sample, so
  T2's visible half would have displayed a duration for something the user did not author. **T4 runs first**, then T2
  publishes the seconds and displays them against a program that belongs to the document.
  **Environment fact worth keeping**, because it cost two agents a diagnosis: this session's cgroup shipped
  `pids.max=64` with 52 already in use, so `cargo build` and even a directory listing died with
  `failed to spawn thread … Resource temporarily unavailable`. That is the same failure L4 of this document recorded
  for the explorer's `find` — it was never a slow machine. It was raised to 4096 for the gentle-shell service to run
  the gate; the original value is 64 and reverting it is a one-line write to that cgroup file.

- **L6 (T4 delivered; the law moved instead of being copied).** `256798a` on `feat/camui-toolpath-t4`, branched
  from `master` at `5cb0652` so the contract move has its own reviewable branch. Gate **exit 0, 94 suites, 1 142
  tests** (+3, +1 suite = `crates/cam-toolpath/tests/program.rs`) by the parent's own run.
  `cam-toolpath::program_for(&Document, &[ProgramTool]) -> Result<MoveStream, ToolpathError>` is
  `cam-cli::build_segment`'s body **moved, not rewritten**: the strategy dispatch, the cone-or-cylinder law and its
  refusals are the same statements. What changed is who owns the failure — `Failure` is a type about process exit
  codes, so the refusals became `ToolpathError` variants with `#[error]`, and `tests/errors.rs` gained a table
  pinning each sentence byte for byte. `ProgramTool` = `ToolSpec` + the flat at the tip, because the flat is read by
  the program and by no strategy. S6 holds by construction: `cam-cli::program_tools` does the `cam-store` →
  `cam-toolpath` mapping where both types are known, and a number the library does not hold is left out of the list
  so the builder names it — the sentence is the same whether the tool is missing from a list of specs or from a
  database.
  **Differential, because this is the emitter's law.** `git archive HEAD` into a scratch tree, both trees built in
  isolation, the same unmodified `crates/cam-cli/tests/cli.rs` — **121 passed / 0 failed on both**. Those 121 pin the
  emitted bytes rather than describing them: three hard-coded golden programs (pocket+drilling and the island
  variant), the 15-line expected profile program, the arc-word flips at three origins, and every refusal's exact
  stderr. A textual audit agrees: 229 of the 339 lines removed from `main.rs` have an exact counterpart in
  `cam-toolpath`; the other 110 are the ones whose type changed, the two call sites and the signature. `Segment` is
  not replaced — `use cam_move::{… Segment as MoveSegment}` keeps it apart from `cam_geom::Segment`.
  **Two things stated, not hidden.** RED was not recorded for the three `tests/program.rs` cases; they arrived with
  the move, so what stands behind them is the differential, not a red run anybody saw. And the commit is **808
  insertions / 347 deletions** against a ~400-line review slice, one commit on purpose: a move split in two would
  leave a tree whose emitter builds nothing.
  **Slice-boundary note, so a number is not misread.** The harness reports this branch at 7 605 authored lines across
  23 commits against `origin/master`. That is `camui-shell`'s delivered work plus T1 accumulating on an unpushed
  `master` (L36 of `camui-shell.md`), not one slice; T4 is the single commit `256798a`. The branch is T4's review
  candidate, and `256798a` is the work unit inside it.
  **Order is now T2.** The estimate can be published and shown against a program that belongs to the document,
  which was the whole reason T4 came first (L5). T3 stays last: it needs T1's list to pick a number from.

- **L7 (T4's review: approved, one lineage, three advisories, no correction).** The native review ran on the
  work-unit candidate `256798a` alone — base `master` @ `5cb0652`, **5 paths / 1 185 lines**, tier **high**
  (`process_boundary` on `cam-toolpath/src/lib.rs`), four lenses (`review-risk`, `review-resilience`,
  `review-readability`, `review-reliability`), `correction_budget: 200`, forecast of 4 model runs and the owner's
  `"1"` to spend them. Lineage `review-2630423c51d0b9e1`, state **approved**, authority **burned** with
  `gentle-ai.review-acknowledged/v1`.
  **No correction was required and no blocker was raised.** Three findings came back, all `informational` and
  explicitly non-blocking — the provider's own statement is that none opened a correction, none reopens the review,
  and no correction transition is offered for this candidate:

  | ID | Lens | Location | Severity |
  | --- | --- | --- | --- |
  | R2-001 | readability | `crates/cam-toolpath/src/lib.rs:73-76` | SUGGESTION |
  | R2-002 | readability | `crates/cam-toolpath/src/lib.rs:328-333` | SUGGESTION |
  | R4-001 | resilience | `crates/cam-toolpath/src/lib.rs:1019` | WARNING |

  They are **later work, never a reason to re-run this review**. The message bodies live in the native receipt, not
  in the closure projection this session saw, so this document records their identity and location and nothing more
  — quoting a finding's text from memory would be inventing it.
  **What the review did NOT cover, stated so it is not assumed:** the candidate was T4's work unit, so `camui-shell`'s
  T8/T9 gap (L37 of `camui-shell.md`) is still unreviewed. And the harness separately offered to review **the whole
  branch against `origin/master`** (`c24f6905`, 24 commits, 26 files, target `sha256:5dbb1fda…`, lineage
  `review-552701826bd439cd`). That start was **not** taken: it is the accumulated feature branch rather than a work
  unit, and it would have opened a second live lineage over the same trees as this one. It remains unstarted, not
  burned.

- **L8 (owner, verbatim: `b`. T2 splits in two, and a crate is born).** T2 as written could not land, and the
  reason is a hole in this document rather than in the code. Measured dependency graph: `cam-store` depends on
  `cam-units`/`cam-geom`/`cam-model` and S6 forbids it `cam-toolpath`; `cam-toolpath` depends on
  `cam-units`/`cam-geom`/`cam-move`/`cam-model` and S6 forbids it `cam-store`; `cam-app` sees neither `cam-store`
  nor `cam-toolpath`; **`cam-cli` is the only crate that sees both**, which is why T4's `program_tools` — the
  `StoredTool -> ProgramTool` mapping — ended up private inside that binary. `cam-ui` holds `cam-store` but not
  `cam-toolpath`, so it cannot reach `program_for` at all, and the estimate can only be *about the document* if it
  does. Three ways out were put to the owner — duplicate the mapping, move it to a crate that sees both, or widen
  S6 and duplicate it under a shared test. The owner answered **`b`**.
  **So T2 becomes two work units.** **T2.0** is the extraction: a new workspace crate `cam-program` depending on
  `cam-model`, `cam-store`, `cam-toolpath` and `cam-units`, holding the mapping as one public function with a typed
  error instead of `cam-cli`'s binary-local `Failure`; `cam-cli` keeps its `Failure` mapping and must stay
  **byte-identical** — the 121 unchanged integration cases in `crates/cam-cli/tests/cli.rs` are the differential, and
  it is a refactor with no behaviour change to declare. **T2.1** is the task the table already describes: publish
  `seconds_for` and draw the estimate beside the card. Splitting them keeps the extraction reviewable on its own and
  keeps `seconds_for`'s RED honest.
  **The S6 consequence, stated so it is not re-litigated.** The new crate gains edges this document did not
  anticipate; what it does **not** do is create a cycle — `cam-store` still refuses `cam-toolpath` and `cam-toolpath`
  still refuses `cam-store`, because the join lives one level above both. A third copy of the mapping is what the
  owner refused.

- **L9 (T2.0 delivered: the join has a crate, and the binary kept its one implementation).** `ee155ff`, gate
  **exit 0, 96 suites, 1 147 tests** by the parent's own run (+5 tests, +2 suite lines: `cam-program`'s unit suite
  and its doc-test suite). New member `crates/cam-program` (`cam-model`, `cam-move`, `cam-store`, `cam-toolpath`,
  `cam-units`), **402 insertions against the ~400 slice** — this one lands inside the guideline, unlike T4.
  Its surface is one function: `program_for(&Document, &ToolLibrary) -> Result<MoveStream, ProgramError>`.
  `program_tools` is moved **verbatim and stays private** — a caller holding `ProgramTool`s calls
  `cam-toolpath::program_for` directly, and widening the mapping is a later decision, not something this slice
  assumed. `ProgramError` has **two causes and no wording of its own**: `#[error("{0}")]` delegates `Display` to
  `cam_store::StoreError` or `cam_toolpath::ToolpathError`, because the sentences an operator reads belong to the
  failure that produced them, and `Failure` — a type about process exit codes — stays in the binary. `#[from]`
  replaces the old `map_err(|e| Failure::input(e.to_string()))` at both call sites.
  **Differential:** `crates/cam-cli/tests/cli.rs` is unedited and still reads `121 passed; 0 failed`; `git status`
  for `cam-store` and `cam-toolpath` is empty, so neither gained the edge S6 forbids, and `cam-cli` ends with one
  implementation instead of two. **RED**, with the moved body stubbed to `unimplemented!()`: `1 passed; 4 failed`;
  the fifth (the wrapper test) passed in both states because it asserts `Display` delegation — stated, not counted.
  **Environment, again from L5.** The first attempt died with `spawn … EAGAIN`: the cgroup's `pids.max` had
  regressed from 4096 back to **64**, saturated at 64/64. The worker wrote **nothing** rather than leave unverified
  code, which was the right call and cost one turn. Raised to 4096 with the owner's explicit authorisation (a write
  outside the repository, so it was asked for by absolute path); the task then re-ran from a clean tree. If `EAGAIN`
  returns, it is that file, not a slow machine.
  **Next is T2.1**, and it is now possible at all: `cam-ui` gains the `cam-program` edge, `view3d.rs` publishes
  `seconds_for` over the one formula `move_seconds` already implements, and the `clock_text` readout draws beside
  the card. Nothing about it needs the mapping widened.
  **The harness's other candidate, for the record (third offer).** Targets `sha256:5dbb1fda…`, `sha256:85b7ec67…`
  and `sha256:7ce326156…` all carried the **same `paths_digest` (`sha256:87486b28…`)** — the accumulated branch
  against `origin/master`, 26 files / 24 commits, changed only in the document's own bytes. Each was declined and
  none was burned; the reminder fires once per mutation generation, so it will keep arriving until the owner either
  reviews that branch deliberately or decides to leave it.

- **L10 (two reviews approved, T2.1 delivered, and a pre-existing flake found).** Both work units of this feature
  that carry a contract now hold a native receipt. **T4** — lineage `review-2630423c51d0b9e1`, candidate `256798a`
  alone (5 paths / 1 185 lines), **approved**, authority burned, 0 blockers, 3 advisories (R2-001, R2-002
  readability, R4-001 resilience WARNING). **T2.0** — lineage `review-526430ccfe87cf3e`, candidate
  `ad4be20..ee155ff` (7 paths / 498 lines), **approved**, burned, 0 blockers, 2 advisories (R2-001
  `cam-program/src/lib.rs:65-68`, R2-002 `cam-program/Cargo.toml:17-23`). All advisory, none opened a correction and
  none reopens anything: they are later work, listed here by id and location only, because their bodies live in the
  native receipt and would otherwise be reconstructed from memory.
  **T2.1 delivered `d617648`** — 8 paths, 323 insertions, inside the ~400 slice. Gate **exit 0, 96 suites, 1 153
  tests** by the parent's own run (+6), `crates/cam-cli/tests/cli.rs` still unedited at **121/0**. The estimate is
  the first thing in this feature that describes the operator's document rather than a demonstrator's sample.
  **The hole L8 did not foresee, and the fix.** `cam-program`'s T2.0 entry point takes a `&ToolLibrary`, and the
  shell has none: S4 hands it a `Vec<StoredTool>` snapshot and no I/O. T2.1 as first scoped was therefore written
  against an API the UI could never call. `cam-program` gained `program_for_snapshot(&Document, &[StoredTool])` over
  `cam_store::tools::stored_tool_for_number` — the rule T1 already published — and the mapping was refactored into one
  shared loop plus `program_tool(number, &geometry, &cutting)`, so both entry points resolve through a single
  per-record rule. One rule, two sources, still no third copy.
  **One time formula, verified by reading it.** `move_seconds` is the law; `total_seconds` projects the running
  total; `seconds_for` and `Preview3d::duration()` are its two readers. The estimate is a **line of its own**, not a
  fourth card field (S2), and when no program can be built it prints the emitter's sentence instead of `0:00` beside
  a card that has just said the tool is missing.
  **RED, three stubs, three real counts:** `program_for_snapshot` stubbed → `5 passed; 2 failed`; `seconds_for`
  stubbed → `25 passed; 2 failed`; field present but nothing drawn → `4 passed; 3 failed`. The pre-existing tests
  passed in the RED run and after, which is what says the refactor kept its ground.
  **Two things stated rather than hidden.** The estimate rebuilds the program **on every frame** it is drawn — a
  cache needs a document revision the shell does not carry, so it was left honest at O(moves) per frame rather than
  guessed at. And **the gate is flaky for a reason that predates this feature**: `cam-geom --test distance`'s
  `the_distance_is_zero_exactly_when_the_segments_meet` finds two concentric arcs where `intersect` answers
  `Coincident` while `segment_to_segment` answers 7.071. It reproduces on a pristine `git archive HEAD` tree with the
  same persisted seed, so it is **not ours** — but it can fail a gate run and it appends a seed to the tracked
  `crates/cam-geom/tests/distance.proptest-regressions`. That file was restored to HEAD content, never edited. This is
  a real defect in `cam-geom` and it deserves its own feature document; it is recorded here so nobody spends another
  turn proving it is not the current change.
  **Slice boundary, so the number is not misread.** The harness reports this branch at 8 331 authored lines across 29
  commits against `origin/master` — that is `camui-shell`'s delivered work plus T1, T4 and T2 riding on an unpushed
  `master`, not one slice. T2.1 is `d617648`, 323 insertions.
  **Next is T3**, now unblocked: T1 gave the shell a tool list to pick a number from, and the vocabulary in `cam-app`
  is the last piece. It is item 3 for the same reason T4 was, so it gets its own commit and its own candidate.

- **L11 (T2.1 reviewed, and where the three lenses agreed).** Lineage `review-4bc03c52d5e44810`, candidate
  `1c99a3b..599d897` — 9 paths / 416 lines, **approved**, authority burned, **no correction and no blocker**. Seven
  advisories came back, all `informational`:

  | ID | Lens | Location | Severity |
  | --- | --- | --- | --- |
  | R2-001 | readability | `crates/cam-ui/src/chrome.rs:804-807` | WARNING |
  | R2-002 | readability | `crates/cam-program/src/lib.rs:77-83` | SUGGESTION |
  | R2-003 | readability | `crates/cam-ui/tests/tool_card.rs:243-261` | SUGGESTION |
  | R2-004 | readability | `crates/cam-ui/src/view3d.rs:708-709` | SUGGESTION |
  | R3-001 | reliability | `crates/cam-ui/src/chrome.rs:804` | WARNING |
  | R3-002 | reliability | `crates/cam-ui/tests/tool_card.rs:255-259` | SUGGESTION |
  | R4-001 | resilience | `crates/cam-ui/src/chrome.rs:804` | WARNING |

  **The one thing worth reading twice, and it is a location and not a verdict.** Three independent lenses —
  readability, reliability and resilience — landed on the **same single line**, `chrome.rs:804`, which is the line
  T2.1 added to draw the estimate. The review admitted it as non-blocking, and this document does not contradict the
  review: but a place three lenses agree on is the strongest signal this feature produced, and it is the first thing
  a later slice should read before touching that line. Their texts live in the native receipt, not here — quoting them
  from memory would be inventing them, which is exactly the failure this rule exists to prevent.
  **Where the feature stands.** The two reference-screenshot patterns that opened this document are both covered: the
  current-tool card (T1, `ad2676a`) and the estimated cutting time (T2.1, `d617648`, now measured over the document's
  own toolpaths rather than a demonstrator's sample). T4 and T2.0 and T2.1 each hold an approved native receipt.
  **T3 is what remains**, and it is the only task in the table that opens new surface: the toolpath vocabulary in
  `cam-app`. Every other task either reused a rule that existed or moved one that did.

- **L12 (state of this document, for whoever opens it next).** Written to be read in ten seconds instead of twenty
  entries. **Branch** `feat/camui-toolpath-t4`, from `master` @ `5cb0652`, **unpushed** — push, PR and merge are the
  owner's decisions and none has been taken. **Gate at the last measured run:** exit 0, 96 suites, **1 153 tests**,
  0 failed; `crates/cam-cli/tests/cli.rs` unedited at 121/0 throughout. **Three receipts burned**, one per work unit:
  T4 `review-2630423c51d0b9e1`, T2.0 `review-526430ccfe87cf3e`, T2.1 `review-4bc03c52d5e44810`. **Ten advisories**
  carried forward across the three (3 + 2 + 7 counted with repeats), none blocking, listed by id and location only in
  L7, L10 and L11. **Two debts that are not this feature's:** the `cam-geom` proptest flake (L10) and `camui-shell`'s
  unreviewed T8/T9 (L7 of that document). **One reminder loop:** the harness offers a review of the accumulated
  branch against `origin/master` on every mutation generation — five offers so far, all declined, none burned,
  because it is not a work unit. If a future session sees that offer, the answer is in this paragraph.

- **L13 (T3 delivered, and the review it did not get — by the lifecycle's own rule).** `762b0d7`, 2 paths, **469
  insertions** against the ~400 slice, kept as one commit on purpose because `cam_app::Command` is a contract others
  consume. Gate **exit 0, 96 suites, 1 166 tests** by the parent's own run (+13); `cam-cli/tests/cli.rs` unedited at
  **121/0**; `cam-app`'s `commands` suite at **87/0** with the thirteen new cases inside it; the known `cam-geom`
  flake did not trip and `distance.proptest-regressions` is byte-identical to HEAD.
  **`next_toolpath` was not paid for, so it was not invented.** `Document` has no toolpath counter, `AppState` carries
  only `next_entity`, and the single `TPn` minter in the tree is `cam-cli::next_toolpath_id`, private to that binary.
  `AddToolpath` therefore takes a caller-supplied `toolpath_id`, exactly as `AddRectangle` takes `entity_id`, which
  keeps `apply` a pure function of its arguments. **The consequence is a task for later, not a hole:** the shell must
  mint the id, and giving `AppState` a toolpath counter is a decision nobody has taken.
  **The boundary held.** No refusal names a tool number, because no rule for one exists — `tool_number` is a `u32`
  with no rule in `cam-model::validate` and `cam-cli`'s `parse_count` accepts `0`, so the only tool-number refusal in
  the tree is library membership, which is the emitter's and the card's (L5). Reaching it would have meant a
  `cam-store` edge on `cam-app`; `crates/cam-app/Cargo.toml` holds **zero** occurrences of it, and the worker stopped
  at the seam and said so rather than crossing it. Two new `CommandError` variants (`UnknownToolpath`,
  `DuplicateToolpath`); `EmptySelection` was already there and is reused, not duplicated.
  **RED twice, and the second one is the interesting one.** The thirteen new cases with stubbed bodies gave
  `FAILED. 75 passed; 12 failed` — the thirteenth passes in that state because `label()` needs no body, stated rather
  than counted. The second RED came from the test's own premise: the operation-swap test first failed with
  `profiling an open line is legal: Invalid(OpenContourNotCuttable { … })`, because `Profile` needs closed geometry
  and nothing can start there. The test starts from `Engrave`, the operation that exists for an open vector.
  **The review, and why there is none.** `assess` returned **`nativeRisk: medium`, `reviewDue: false`**
  (`under_budget`), on `executable_change` rather than `process_boundary` — unlike T4, T2.0 and T2.1, which all came
  back `high` and were reviewed. The lifecycle is explicit that a review starts only when `reviewDue` is true or the
  owner asks, and the tier native measures wins over an agent's own estimate, so **no review was started and no model
  runs were spent**. That is a decision the rule made, not a shortcut: recorded here so nobody later reads T3's
  missing receipt as an oversight. If the owner wants it reviewed anyway, that is one request away.
  **L12's numbers are superseded by this entry:** the gate is 1 166 tests and the receipt count is **three**, because
  T3 never had one to burn.

- **L14 (the feature closes).** Every task in the table is delivered: **T1** `ad2676a` (the current-tool card),
  **T4** `256798a` (the emitter's law moved into `cam-toolpath`), **T2.0** `ee155ff` (the join has a crate),
  **T2.1** `d617648` (the estimate, over the document's own toolpaths), **T3** `762b0d7` (the authoring vocabulary).
  **T2 was split** by the owner's `b` (L8) and both halves landed. The two reference-screenshot patterns that opened
  this document are both taken. Three native receipts burned (T4, T2.0, T2.1); T3 owed none.
  **Where it stands, so the next session does not have to work it out.** Branch `feat/camui-toolpath-t4`, from `master`
  @ `5cb0652`, **unpushed and unmerged** — push, PR and merge are the owner's decisions and none has been taken. The
  gate is green at 96 suites / **1 166 tests**. Ten advisory findings across the three reviewed units are carried
  forward, all non-blocking, listed by id and location only in L7, L10 and L11; the sharpest of them is the single
  line `crates/cam-ui/src/chrome.rs:804`, where three independent lenses converged (L11). Two debts belong to other
  features: the `cam-geom` proptest flake (L10) and `camui-shell`'s unreviewed T8/T9.
  **One thing this feature leaves true and unfinished:** the shell still cannot open a `.c2d`, so `Document.tools_ref`
  remains a filename only `cam-cli` resolves and `cam-desktop` still hands the shell an empty tool snapshot (L5). The
  card and the estimate are honest about that state — either names what is missing instead of drawing a number — but
  the feature that makes the shell open a document is not this one.
