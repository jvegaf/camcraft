# The script panel edits the shell's document, and the file is written by Save

Feature document for closing the residual `odd/tasks/022-script-panel-mount.md` S3 left open: the mounted
`ScriptPanel` runs a script against a path on disk, so the shell's in-memory `Document` never changes.
Read **until `## Log`** before touching code; the Log is the record, not the brief.

## Specs

- **S1 (user, verbatim option label chosen for "Cuando apretás Run en el panel, ¿dónde viven las
  mutaciones del script?"):** "Sólo memoria; el archivo lo escribe Save". A script run from the shell
  mutates the shell's in-memory `Document` and marks it modified; the file on disk is written only by
  the shell's own Save, exactly as any other edit. One source of truth, and no second writer.
- **S2 (repo law that binds every task here):** the panel runs `cam-script` in process — never a spawned
  CLI — and one behaviour keeps one implementation. `cam-cli script` is a command this request does not
  change: it still reads and writes a file, and its output, diagnostics, exit codes and documents must
  stay byte-identical. That is a differential obligation, not an aspiration.
- **S3 (the contract change, stated where the user can read it):** a script run from the shell no longer
  writes the file. The document the panel produced is lost if the shell closes without saving, which is
  the same exposure every palette edit already carries and is why the modified state must be visible.
  The CLI keeps its write path, so `cam-cli script` and the panel now differ in where the result lives —
  deliberately, and named in the docs rather than discovered.
- **S4 (validation is not duplicated):** every mutation still goes through `Document::validate`, in the
  one place `cam-cli script` already does it. The new entry point must not grow a second validation
  policy, and a refused script must leave the in-memory document exactly as it was.

## Tasks

| ID | Task | Specs | Route | Commit |
| --- | --- | --- | --- | --- |
| T1 | ✅ delivered `63e7aa4` — `cam-script`'s entry point takes a `&mut Document` instead of only a path, so a caller can run a script against a document it owns | S1 S2 S4 | parent | `63e7aa4` |
| T2 | ✅ delivered `63e7aa4` — The panel calls that entry point with the shell's document, reports what the run did, and marks the document modified | S1 S3 | parent | `63e7aa4` |
| T3 | ✅ delivered `63e7aa4` — The disk write leaves the panel's path: `cam-cli script` keeps it, the shell does not, and the differential proves the command is unchanged | S2 S3 | parent | `63e7aa4` |
| T4 | ✅ delivered `63e7aa4` — The record: `docs/m10-documents.md`'s "the panel edits a file on disk, not the shell's document" stops being true, and says what replaced it | S3 | parent | `63e7aa4` |
| T5 | ✅ delivered `5332383` — `R4-001` of the review: a run that produced the document it was given is not an edit, so it records no undo step and leaves the modified state alone | S1 | parent | `5332383` |

## Log

- **L1 (user, verbatim, in sequence):** "que tenemos pendiente" → "vale" → "continua" → then the choice
  recorded as S1. The session's opening question was answered from the record: M0-M10 delivered, no next
  milestone row, ten unrecoverable advisories, and the mount's residuals. The owner chose the next stage
  ("El panel edita el documento del shell") and then the semantics (S1).
- **L2 (the reconnaissance attempt, and its failure).** A first `gentle-ai-explore` run for the candidate
  milestones **timed out** and returned nothing; the roadmap evidence was gathered inline instead
  (`odd/roadmap/index.md:145-176`, `:153-156`, `:980`). A second, strictly bounded explorer mapped this feature:
  the document lives in `cam_app::AppState` (`crates/cam-app/src/lib.rs:104-105`), not in `Shell`; `Shell`
  keeps only `document_path` (`crates/cam-ui/src/lib.rs:1141`); the dirty flag is `AppState::is_dirty()`
  (`cam-app:175-177`) and the undo-aware wholesale replacement is `replace_document_undoable`
  (`cam-app:161`); `run_script_to` (`crates/cam-cli/src/script.rs:164`) is a clean library seam with no
  argv types. The one fact that shaped the design: `Session` needs `document` **and** an optional `path`,
  because `toolpath`, `emit` and `simulate` resolve `tools_ref` and the post-processor beside the
  document.
- **L3 (T1, and the defect the new tests found in the old code).** `run_script_on(script, &mut Document,
  Option<&Path>, reports, notices)` runs the whole script against a clone and replaces the caller's
  document only when every line was accepted, which is S4 for free. `open` and `save` are refused on the
  line that tries (`ALREADY_OPEN`, `NO_WRITE`); `Session::document_path` separates "no document" from "a
  document with no path yet", so an untitled document says what it is missing instead of pretending it
  does not exist. **RED first, and the first RED was not the one planned**: the five new tests in
  `crates/cam-cli/tests/script_in_memory.rs` failed to compile (no such function); after the entry point
  existed, `open_is_refused…` failed on the *column* — the refusal was located at the path argument when
  the rule is about the command — so it moved to the word. Then the panel tests found a real defect in
  shipped code: `validate` demanded a document **path** because it went through `open_document`, which
  needs both, so an untitled document could not even be validated. `validate` now asks for
  `Session::the_document`, and `open_document` delegates. `cam-cli` is **26 passed** (21 the CLI's own,
  unchanged, plus the 5 new) — including the differential test that already pins the script's report
  reaching a sink byte-identical to the binary's stdout.
- **L4 (T2, and the mount's own promises re-pointed).** `ScriptPanel::run(document, document_path)`
  returns whether the run succeeded, and `ui` returns `Option<Document>`: the panel mutates a **copy** and
  hands it back only on success, so a refusal can never leave half a script in the shell's document. The
  shell commits it with `AppState::replace_document_undoable` (`crates/cam-ui/src/chrome.rs`), which is
  one undo step **and** sets the modified state — S1's "marks it modified" with no second flag. `chrome`
  now needs `&mut AppState`, so `script_dialog` takes it and `lib.rs:1754` passes it; `Shell` gained
  `script_panel()`, because a widget a headless test cannot press is a widget nothing pins.
  **The mount's seven panel tests encoded the old semantics** — their fixtures ran `open <file>` and one
  asserted the `save` line — so `crates/cam-ui/tests/script_panel.rs` was re-pointed: the fixtures are a
  `Document` value now, `valid_script` is `validate`, and the save-line test became
  `save_is_refused_because_the_shell_writes_the_document`. Two promises are new and follow from S1/S4:
  `a_run_hands_back_the_document_it_mutated` (the passed-in document is untouched; the returned one holds
  the sheet, the part and the placement) and the refusal test above. `cam-ui` script_panel: **8 passed**;
  shell: **127 passed**, including `a_run_from_the_script_modal_edits_the_shells_document` (palette →
  modal → `Run`, then the document holds the sheet and `is_dirty()` is true) and
  `a_refused_run_leaves_the_shells_document_and_its_modified_state_untouched` (line 1 accepted, line 2
  `shet`: nothing kept, not dirty, `line 2:1` painted).
- **L5 (T4).** `docs/m10-documents.md`'s second residual now says the mount reads and writes **the shell's
  document**, names `run_script_on`, the transaction, the two refusals and the undo step — and states the
  cost where an operator reads it: a run from the panel writes no file, so closing the shell without
  saving loses it.
- **L6 (T3, the gate, and the differential).** The disk write left the panel's path without any code being
  deleted: `cam-cli script` still calls `run_script_to`, which still reads the file, still lets `save`
  write it and still prints `save`'s notice to stderr, while the panel calls `run_script_on` and never
  reaches a file. Because `crates/cam-cli/src/script.rs` is shared code for a command this request never
  named, the differential ran: the scripted command over **11 scripts** (accepted run with sheet + save,
  located failure, `emit`, `simulate`, a second `open`, `validate` with no document, `toolpath` with no
  document, `emit` with no document, a missing CSV, `cutlist`, an empty script) against the previous tree
  in `/var/tmp/campilot-m11-diff.WoyT1o/base` and against this one — **stdout, stderr and exit code
  identical in all eleven, and the document the accepted run wrote is byte-identical**
  (`DOCUMENT_BYTES=identical`). `VERDICT=no regression in cam-cli script`. The harness lives at
  `/var/tmp/campilot-m11-differential.sh`, outside the repository, and is not committed.
  **The gate**: `./scripts/check.sh` — `fmt --check`, `clippy -D warnings` and the whole suite — green at
  `CHECK_EXIT=0`; `cargo test --workspace -- --list` counts **1525 tests**. The first gate run failed on
  `cargo fmt --check` (the harness's own formatting, nothing semantic); `cargo fmt --all` was the fix.
  **`assess`** answers `risk: medium`, `nativeRisk: unassessable` — *"untracked files require an explicit
  declaration"*, the same trap `odd/tasks/021-debt-advisories.md` L7 named — with the plan
  `writerSelfVerification: true`, `independentVerifier: false`, `writerProfileSource: runtime`. The native
  review was therefore not started: this candidate is medium, no Task Size high-risk item fired, and the
  self-verification the plan asks for is the gate plus the differential above.
- **L7 (how it closed in git, and why it is one commit).** The owner chose **one commit on `master`** holding
  the code, the tests and the docs together, over the two work units this change would otherwise split
  into (`feat(cam-cli)` the entry point, `feat(cam-ui)` the panel and the shell). Nine paths travel: seven
  modified (`cam-cli/src/script.rs`, `cam-ui/src/{chrome,lib,script_panel}.rs`,
  `cam-ui/tests/{script_panel,shell}.rs`, `docs/m10-documents.md`) and two new
  (`crates/cam-cli/tests/script_in_memory.rs`, this document). The commit message carries the evidence
  L6 measured, so the gate and the differential are readable from `git log` and not only from here.
- **L8 (the native review: approved, receipt burned, and six advisories whose words did not survive).**
  Lineage **`review-cf16da5a0b71e7b8`** over `7cb6fcd..63e7aa4` — the commit L7 made, 9 paths, 910 changed
  lines. Tier **`high`**, on a `process_boundary` signal naming `crates/cam-ui/src/lib.rs` ("code that
  starts other processes"), not a judgement about the change; correction budget **200**, of which **zero**
  was spent. Two START attempts: the first with an explicit lens selection was refused
  `native-start-input-invalid` / `lenses-invalid`, `lineage_created: false`, nothing burned — the same
  inert selection `odd/tasks/021-debt-advisories.md` L10 recorded — so the provider chose the four lenses
  itself: **risk, resilience, readability, reliability**. Four host-relay reviewer runs (prompts 80 115,
  80 129, 80 161 and 80 140 bytes; results 944, 4 302, 2 805 and 2 766), closure **`approved` on the last
  admitted event**, no correction, and the receipt burned: `gentle-ai.review-acknowledged/v1`, consumed
  revision `sha256:506f7b2a356f1076b478767fd04b0edf02b4aa52bf762bd0730ae32386d213fc`.
  Its six findings are **all `informational`** — none opened a correction and none reopens the lineage —
  and **their wording is not in the envelope**: it lists id, lens, severity, location and disposition, so
  what follows is the anchor and, where the code still says what the finding was about, this document's
  reading of it, marked as such rather than presented as the reviewer's words (the store blindness of
  `odd/tasks/021-debt-advisories.md` L9, one review later):
  - **`R2-001`** readability, `WARNING`, `crates/cam-cli/tests/script_in_memory.rs:112-126` — the `save`
    refusal test. Read: it asserts the message *contains* `writes no file` instead of pinning the whole
    diagnostic, so a rewrite of that sentence would pass the test.
  - **`R2-002`** readability, `SUGGESTION`, `crates/cam-cli/src/script.rs:325-327` — the `document_path`
    match. Read: the `(_, Some(path))` arm decides on one field while the other two decide on both, so
    the arm order carries meaning a reader has to reconstruct.
  - **`R2-003`** readability, `SUGGESTION`, `crates/cam-cli/src/script.rs:760-762` — the guard at the top of
    `command_save`. Read: the same "the shell owns it" guard the `command_open` edit added, written twice,
    with no helper naming the rule once.
  - **`R3-001`** reliability, `SUGGESTION`, `crates/cam-ui/src/script_panel.rs:255-260` — the clone-and-commit
    block. Read: the accepted path is pinned (the shell test), and what is not is the panel's own edge — a
    successful run that changed nothing.
  - **`R4-001`** resilience, `WARNING`, `crates/cam-ui/src/chrome.rs:1405-1407` —
    `app.replace_document_undoable(document)`. **Read, and the one worth acting on:** the commit is
    unconditional, so a script that succeeds and changes nothing still records an undo step and marks the
    document modified; a guard comparing the returned document with the one passed in would leave a no-op
    run a no-op. This is an inference from the anchor, not the reviewer's sentence. **Closed in its own
    candidate**, because the inference was reproducible: `a_run_that_changed_nothing_does_not_mark_the_
    document_modified` failed first (a successful `# nothing to do` script left `is_dirty()` true), and
    the guard now commits only when the document the engine produced differs from the one it was given.
  - **`R4-002`** resilience, `SUGGESTION`, `crates/cam-ui/src/script_panel.rs:256-261` — the same block. Read:
    the whole document is cloned on the frame that pressed `Run`, on the UI thread, which is the cost the
    module's own "synchronous, deliberately" rule already accepts for the run but does not name for the copy.
  Per the provider, these are **separate later work and never a reason to re-run this candidate**. The one
  with a behavioural edge is `R4-001`: a future task that guards the commit would close it, and would need
  its own candidate and its own review.
- **L9 (`R4-001` closed in its own candidate, and the review of that candidate was not due).** `5332383`
  carries T5: RED first — `a_run_that_changed_nothing_does_not_mark_the_document_modified` failed with
  `is_dirty()` true after a successful `# nothing to do` script — then the commit in `chrome.rs` became
  conditional on the document the engine produced differing from the one it was given, which is one
  comparison because `Document` derives `PartialEq`. The test asserts the run painted no diagnostic, the
  document is the one it was given, the modified state is clear **and** `undo()` answers `false`. `cam-ui`
  shell: **128 passed**; panel: **8 passed**; gate green at **1526 tests**.
  `assess` on that commit (`baseRef: 451d586`, `committedOnly`) answers `risk: medium`, 3 paths, 60 lines,
  **`reviewDue: false`**, reason **`under_budget`** — so no review was started for it, which is the
  provider's own measurement rather than this session's judgement. `inspect` did offer `review.start` for
  the **accumulated** diff against `7cb6fcd` (target `sha256:5eaf9283…`, lineage `review-fa34607b7e771994`),
  and starting that would have re-reviewed `63e7aa4`, whose receipt is already burned — exactly what L8's
  closure forbids. Nothing was started, so that lineage does not exist and there is no authority to abandon.
- **L10 (the floor state for the next session).** The feature is **closed**. The last **executable** commit is
  `5332383`, with this record's own `docs(odd):` commit on top of it; the tree is clean, and four commits
  sit local and unpushed: `63e7aa4` (the change), `451d586` (its review), `5332383` (`R4-001`) and the sync
  that carries this paragraph. The review lineage `review-cf16da5a0b71e7b8` is approved with its
  receipt burned (`sha256:506f7b2a…`); no authority is open for this feature. What a later session
  inherits: five advisories from L8 that were left open (`R2-001`, `R2-002`, `R2-003`, `R3-001`, `R4-002`,
  all `informational`), the ten unrecoverable advisories named in `odd/tasks/021-debt-advisories.md`, and the
  store blindness that cost L8's six findings their wording — the one method rule this feature proved
  twice, on its own receipts.
- **L11 (delivered).** The owner pushed. `origin/master` is `098b2e9` and the branch is level with it —
  nothing ahead, nothing behind — so L10's "local and unpushed" describes the state before this entry, the
  way L11 of `odd/tasks/022-script-panel-mount.md` supersedes its own L6. The delivery that this repository
  leaves to its owner was taken: the four commits are on the remote, this documentation entry is the only
  thing the tree carries past them, and the feature is closed on both sides.
