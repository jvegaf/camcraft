# shell-save-document — the window writes the `.c2d` it opened

The window opens a document now and still writes none: the README says so twice, and the review that
approved opening left four informational advisories that all point at what a document carries beyond its
model. This feature closes them, and it is the one where a mistake reaches the user's file.

## Specs

- **S1 — the decision.** The owner's words, in order: *"pushea"* (delivery of the open feature), then, to the
  parent's proposal *"si arrancás por Save, avisame y hago el mismo camino"*, *"me parece bien, hazlo"*. The
  requirement is the parent's own sentence that proposed it: *"Guardar es donde esos `WARNING` se vuelven
  reales"*.
- **S2 — Save and Save As, and nothing else.** `Save` writes to the path the window already recorded and is
  **refused** with a message when there is none, pointing at Save As. `Save As` asks the host for a path (the
  same shape Open uses: an intent out of the shell, `rfd` in the host, a path back in as an action) and, on
  success, becomes the current path. Both write through `DocumentFile::save`, which validates first and
  writes a sibling temp file then renames — never a truncating write in place.
- **S3 — the archive's other entries survive.** `DocumentFile` exists so that "the entries this build does
  not interpret are part of the document rather than something to drop on the floor"
  (`cam-store/src/document.rs:16-19`). Opening currently throws them away
  (`cam-ui/src/lib.rs:2330` passes only `opened.document`), so the shell must carry them and `Save` must
  write them back. `save_document` — which builds a `DocumentFile` with no extras — is **not** used by the
  window. This is advisory `R3-extras-dropped`'s content, and the reason it is not cosmetic.
- **S4 — the dirty flag stops lying, and protect what is dirty.** A document that was just opened is exactly
  what is in its file, so opening clears the dirty flag (today `replace_document_undoable` leaves it set, and
  the bar says `· modified` over an untouched file). A save clears it too, through `mark_saved`, which exists
  and has never been called. And because the window can now save, opening a **dirty** document is refused
  with a message telling the user to save or undo first — the explicit rule advisory
  `R3-dirty-open-unproved` says no test covers. Nothing is discarded silently, and the refusal leaves
  document, history, path and name untouched. **Amended after the verifier ran (L5):** an open also enters
  **no** history entry, because an undo that crossed a document boundary left the recorded path describing a
  different drawing, and the next save overwrote the wrong file. The owner took that decision explicitly, as
  *"Abrir deja de ser deshacible"*, and it supersedes `shell-open-document`'s S4.
- **S5 — a failed save changes nothing.** On any write or validation error the dirty flag stays as it was, the
  recorded path stays as it was, and the status line says `refused: …`. The file on disk is the atomic
  writer's business — it either has the old bytes or the new ones, never half of each.
- **S6 — what must not change.** `cam-cli`'s write path and its extras-dropping `save_document` (a CLI
  document that authors a new file legitimately has no extras to keep); the Open behaviour and its six tests
  except where S4 deliberately clears the dirty flag; the app bar's pinned order gains Save and Save As and
  nothing else moves; `rfd` stays in `cam-desktop`.
- **S7 — recorded, not hidden.** Saving a legacy `c2d/document@1` file rewrites it as `@2`, because `migrate()`
  upgrades in memory and this build only writes `@2` (`cam-store/tests/document.rs:160-190` pins that as
  intended). Extras are file-level: they are preserved from the last load and are **not** part of the undo
  history, which snapshots bare `Document`s. A `tools_ref` is serialized and survives the round trip, but the
  window still does not resolve it — that stays the next feature.
- **S8 — risk.** Task Size item 1: this writes stored user data. The writer is atomic and validates first, and
  the independent verifier takes this change alone.

## Tasks

- T1 — `cam-ui` (S2, S3, S4, S5): `UiAction::{Save, SaveFile{path}, RequestSaveAs}`, extras carried on the
  shell, the write, the dirty rules, the bar's Save/Save As, and the tests. Route: delegated worker.
  Commit: `c053705`.
- T1b — a requirement added after reading the worker's report, not in the original spec: a bare
  `handle_action` must not reach the writer, because it has no shell and therefore no extras. Closed by type
  rather than by comment — the private handler takes `Option<&mut DocumentFileState>` — with a test that
  fails if the write happens. Route: delegated worker. Commit: `c053705`.
- T2 — the host (S2, S6): `rfd`'s `save_file()` answering `RequestSaveAs`, seeded with the current name.
  Route: parent inline. Commit: `c053705`.
- T3 — the documents (S1, S2, S7): README, `camui-shell.md` L45, and `odd/roadmap.md` — which needed
  nothing, checked row by row. Route: delegated worker. Commit: the commit that carries this line.
- T4 — gate, commits, independent verification, native review (S8). Route: parent. Commit: `c053705` plus
  the record commits.

## Log

- **L1 (the request, verbatim).** *"me parece bien, hazlo"*, accepting the parent's *"Si arrancás por Save,
  avisame y hago el mismo camino"*, in a session that had just pushed the open feature.
- **L2 (reconnaissance, before any write).** `DocumentFile::save` already exists (`cam-store/src/document.rs:93`),
  validates at `:95-101`, and writes through `write_atomically` (`:168-188`): a sibling `.{name}.tmp` then a
  rename. `DocumentFile` carries `extras: BTreeMap<String, Vec<u8>>` (`:20-25`) and `to_bytes` writes them back
  under their original names (`:127-136`). The extras-dropping `save_document` (`:148`) is what every CLI
  caller uses (`cam-cli/src/main.rs:434`, `:1413`, `:1516`).
  What the window has today: `document_path: Option<PathBuf>` on the shell (`cam-ui/src/lib.rs:808`), documented
  as "a record of what was read and not a file handle"; `mark_saved()` in `cam-app` (`cam-app/src/lib.rs:180-182`)
  called by nothing but a test; and history snapshots of bare `Document`s (`:62-64`), so extras cannot ride the
  undo stack. The app bar's pinned order is `Undo, Redo, Open, Fit the job` (`cam-ui/tests/shell.rs:2383-2400`),
  and the `SAVE` and `SAVE_AS` glyphs already exist unused (`cam-ui/src/icons.rs:254`, `:267`, `Icon::Save`
  `:1547`, `Icon::SaveAs` `:1549`) with no `command_icon` arm.
- **L3 (two things found that the feature must fix rather than inherit).** Opening leaves the document **dirty**,
  because it swaps through `replace_document_undoable` (`cam-app/src/lib.rs:166` sets `dirty = true`), which was
  written for import; the result is a bar that reads `· modified` over a file nobody has touched. And opening
  while dirty silently discards edits — recoverable only because undo happens to hold them, which is exactly the
  state a Save feature must not leave standing.
- **L4 (the implementation, and the one thing the first version got wrong).** The `cam-ui` unit landed Save,
  SaveFile and the Save As intent, carried the archive's extras on the shell, cleared the dirty flag on a
  successful open, and refused an open over unsaved work; the host answered the Save As intent with `rfd`'s
  `save_file()`, seeded with the current document name, and built clean. The unit was honest about not
  having observed a RED in order — it wrote the implementation first — and proved the two riskiest pins are
  not vacuous by **mutation**: dropping `*extras = carried` made the extras round-trip test fail at
  `shell.rs:2206`, and disabling the dirty refusal made the other fail at `:2330`.
  **What I rejected in its report:** it left a bare `handle_action(SaveFile)` writing a document whose
  archive had lost its extras, documented as "hosts must go through `apply_action`". A comment does not stop
  a caller, and silently dropping the entries this feature exists to preserve is the damage S3 forbids — so
  T1b turned it into a refusal enforced by the type of the private handler, with a test that notices if the
  write ever comes back. The parent also corrected two stale claims in the code's own comments (`lib.rs`
  saying the undoable replacement exists "because the shell has no Save", and a test's assertion message
  saying the same) rather than leaving them to be read as reasoning.
  **Gate:** exit 0, **96 suites, 1 192 passed, 0 failed** — the previous 1 184 plus exactly the eight added,
  with `fmt --check` and `clippy -D warnings` inside it, the whole run alone on this box (`CARGO_BUILD_JOBS=1`
  and the linker thread cap, or it dies spawning threads).
- **L5 (the independent verifier, and what it found that we had not).** Verdicts: R1, R2, R4 and R5 **PASS**;
  R3 passed every clause **except** "leaves no temporary behind", which it reproduced and failed. What it
  handed back, and what was done with each:
  **Fixed, both with RED observed before the fix:** **A1** — a failed rename left `.{name}.tmp` beside the
  user's file, pre-existing in `write_atomically` (`cam-store` byte-identical to the base) but newly reachable
  from the window, and a direct contradiction of S5; the writer now unlinks its own temporary and returns the
  same error. **A2** — `SaveFile { path: "" }` wrote `.document.tmp` into the process working directory; both
  save arms now refuse an empty path before any filesystem call.
  **Decided by the owner:** **A6** — open `a.c2d`, open `b.c2d`, `Undo`, `Save` overwrote `b.c2d` with a's
  drawing and dropped its archive entries. That is the class this feature exists to prevent, and it came from
  the undoable open the *previous* feature promised. The owner chose *"Abrir deja de ser deshacible"* over
  recording it as a limit or making undo forget the path; the open now swaps through `replace_document`, so no
  history entry can cross a document boundary and the invariant holds by construction. Both new tests fail if
  the arm is ever made undoable again. It costs the owner one honesty, recorded here: an accidental open is no
  longer one key from being undone, and since the shell keeps no recent-file list, getting back is manual.
  The dirty refusal is what makes that safe — no edits are lost — and the help text that still promised
  "one undo brings the drawing back" was corrected, because a shipped UI sentence contradicting the rule is
  its own defect.
  **Recorded and deliberately not fixed:** **A3** a save through a symlink replaces the link and leaves the
  target stale; **A4** a missing parent directory is created instead of failing; **A5** the writer can emit an
  archive `load` refuses, because a NaN coordinate serialises as `null` and `validate()` does not check
  finiteness — latent, since both user-facing entry points that could produce a non-finite value already
  refuse it; and **A8** a Save As copies the source file's uninterpreted entries into the new file, which is
  S7 working as specified. The first three are behaviours of the shared writer that `cam-cli` also inherits;
  changing them is a separate decision, not a fix to smuggle into this feature.
  **Not verified by it, and named rather than assumed:** the whole-workspace gate; the desktop host's dialog
  (read, not executed — no display); zip directory entries, which `load` skips and a save therefore drops; and
  a mid-write `ENOSPC`, whose invariant is argued from write-temp-then-rename plus the three failure probes.
  Every finding above was reproduced by the verifier itself, with the reproducer, which is why they are
  recorded as findings and not as worries.
- **L6 (the review: approved, four advisories, and two failed attempts to start it).** Lineage
  **`review-6e601ec08a007c8e`**, target `sha256:986a350c…`, tier **`high`** on the provider's own
  `process_boundary` signal, lenses **`review-risk` + `review-reliability`**, 11 paths / **1 354 changed
  lines**, correction budget 200. Result: **approved on the first admitted event**, no correction round, no
  blocker, budget unspent. Both reviewers ran over the Pi host relay (risk: 97 654 B prompt, 2 522 B result;
  reliability: 97 679 B / 2 600 B) and the authority is **burned** (`gentle-ai.review-acknowledged/v1`,
  consumed revision `sha256:0f30ccf4…`).
  **Two failed attempts to start it, recorded because both were avoidable and neither was the provider's
  fault:** the first was rejected `base-ref-unresolvable` because the parent copied the commit id with a
  digit missing — the provider's message named the problem exactly, no lineage was created, and there was
  nothing to reconcile. The second resolved the id but died in `git grep` with `failed to create thread:
  Resource temporarily unavailable`, the box's own process limit, again with no mutation and no lineage; the
  controller reconciled through target-scoped STATUS and the third attempt created the lineage. Both are the
  same lesson this session learned four times over: this box runs one thing at a time, and the limit is the
  user's process count, not the project's.
  **Four advisories came back, all `informational`, all `WARNING`, none a correction**, and their text is not
  quoted here on purpose — it lives in the native receipt. What can be said from the locations: `R1-001` at
  `cam-ui/src/lib.rs:2503` and `R3-002` at `:2445` sit in the save path's own bookkeeping; `R3-001` at
  `:2711`; and `R1-002` at `crates/cam-store/src/document.rs:183-188`, which is **exactly the region the
  verifier's A1 punched a hole in** and where the fix now unlinks the writer's temporary on a failed rename.
  The review saw that region with the fix in place and approved, so what it names there is something beyond
  the fix, not the leftover itself. A future reader should treat all four as the review's own statement says:
  separate later work, never a reason to re-run this review.
  **What this receipt does not authorise:** delivery. Push, pull request and merge stay ordinary repository
  policy and the owner's decisions.

- **L7 (S7's last clause is now historical: the window resolves the `tools_ref` it preserves).** S7's first two
  sentences stand unqualified — a legacy `c2d/document@1` file is rewritten as `@2`, and extras are file-level
  and outside the undo history — and its third, "a `tools_ref` is serialized and survives the round trip, but
  the window still does not resolve it — that stays the next feature", is answered by
  `odd/tasks/shell-tools-ref.md`. The answer is about opening, which is why nothing in this document's save path
  moved: on a successful open the shell resolves the loaded document's `tools_ref` **relative to the document's
  own directory** through `tools_beside_document` (`crates/cam-ui/src/lib.rs:2752-2798`) and hands
  `ToolLibrary::tools()` to `set_tools` (`:1081-1090`), so a document this window saved with a `tools_ref`
  naming a library beside it now resolves that library when it is opened again. The round trip S7 describes is
  untouched: the open reads the field the save wrote and writes nothing back into the document. Two properties
  of the resolution matter to a reader of *this* document, because both concern data this writer produces. A
  library that is missing, unreadable or not a library **does not cost the drawing** — the file opens, the card
  says the tool's number is not in the library it was given, and the status line keeps the open's own sentence
  and appends the library and the reason (`:1100-1102`) — so a `tools_ref` written here can never make a saved
  document unopenable. And a reference that is **absolute or carries a `..`** is refused before anything is
  read: the window reads nothing outside the document's directory and creates nothing anywhere, because
  `ToolLibrary::open` (`crates/cam-store/src/tools.rs:220-242`) creates the parent directory and the file it is
  pointed at when the path is absent, which is why existence is checked first (`lib.rs:2788-2795`). Six tests in
  `crates/cam-ui/tests/shell.rs:1971-2203` pin it. **The code is uncommitted in the working tree** when this
  entry was written, so it cites no commit. **No gate was run for this entry**: it is documentation.
