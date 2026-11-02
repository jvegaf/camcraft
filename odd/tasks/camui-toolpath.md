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
