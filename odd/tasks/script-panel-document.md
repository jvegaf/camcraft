# The script panel edits the shell's document, and the file is written by Save

Feature document for closing the residual `odd/tasks/script-panel-mount.md` S3 left open: the mounted
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
| T1 | `cam-script`'s entry point takes a `&mut Document` instead of only a path, so a caller can run a script against a document it owns | S1 S2 S4 | parent | working tree |
| T2 | The panel calls that entry point with the shell's document, reports what the run did, and marks the document modified | S1 S3 | parent | working tree |
| T3 | The disk write leaves the panel's path: `cam-cli script` keeps it, the shell does not, and the differential proves the command is unchanged | S2 S3 | parent | working tree |
| T4 | The record: `docs/m10-documents.md`'s "the panel edits a file on disk, not the shell's document" stops being true, and says what replaced it | S3 | parent | working tree |

## Log

- **L1 (user, verbatim, in sequence):** "que tenemos pendiente" → "vale" → "continua" → then the choice
  recorded as S1. The session's opening question was answered from the record: M0-M10 delivered, no next
  milestone row, ten unrecoverable advisories, and the mount's residuals. The owner chose the next stage
  ("El panel edita el documento del shell") and then the semantics (S1).
- **L2 (the reconnaissance attempt, and its failure).** A first `gentle-ai-explore` run for the candidate
  milestones **timed out** and returned nothing; the roadmap evidence was gathered inline instead
  (`odd/roadmap.md:145-176`, `:153-156`, `:980`). A second, strictly bounded explorer mapped this feature:
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
  declaration"*, the same trap `odd/tasks/debt-advisories.md` L7 named — with the plan
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
