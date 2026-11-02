# shell-open-document — the window opens a `.c2d`, chosen at runtime

The editor window has always been handed one hard-coded sample document (`cam-desktop/src/main.rs:57`),
and every record says so: the app bar reads `untitled.c2d` because "the document has no file name yet"
(`cam-ui/src/chrome.rs:335-336`), and `help.rs:492` tells the user "There is no file dialog yet, so a host
opens the file by path". The loader that would open a file already exists, already validates and already
migrates; `AppState::replace_document` is even documented "Used when opening a file" and nothing calls it.
This feature connects the three.

## Specs

- **S1 — the decision.** The owner's words, in order: *"vale"*, accepting the parent's offer to start with
  *"abrir `.c2d` desde el shell, que es aditivo y no rompe nada"*, then this choice verbatim from the
  question the parent asked: option *"Diálogo nativo con rfd"* — *"El botón Open abre el diálogo del
  sistema. Es la feature real, pero suma una dependencia que el registro del proyecto dejó explícitamente
  sin decidir."* The dependency is therefore taken on purpose, not by drift: the record at
  `odd/tasks/m3-import.md:113` ("`rfd` or equivalent is a later decision") is decided here.
- **S2 — the capability.** The system's Open button (the glyph that exists unused at
  `cam-ui/src/icons.rs:1545-1552`) asks the host for a path; the host shows the platform file dialog; the
  chosen path is loaded through `cam_store::document::DocumentFile::load`, which is the same call the CLI's
  private loader makes and therefore the same archive scan, migration and `validate()`. On success the window shows that
  document and the app bar shows its file name where it shows `untitled.c2d` today.
- **S3 — a refused open is not a destructive open.** A path that cannot be read, is not a `.c2d`, or holds
  an invalid document leaves the document, the history, the dirty flag and the app-bar name exactly as
  they were, and the status line says what happened. The import action already behaves this way
  (`cam-ui/src/lib.rs:968-972`, pinned by `shell.rs:1598-1619`); opening follows it rather than inventing
  a gentler failure.
- **S4 — nothing stale, and nothing silently lost.** Opening must clear every piece of state derived from
  the previous document: the 3D preview (`Shell.preview`), the tool snapshot (`Shell.tools`) and the
  material-size drafts (`Shell.material_size`), all of which survive a bare `replace_document` today. The
  swap goes through the **undoable** document replacement, because the shell has no Save and an
  accidental open would otherwise destroy edits with no way back; one undo returns the previous document.
- **S5 — what must not change.** `cam-cli`'s behaviour and exit codes; the import path and its refusal
  message; the shell's own dependency set — `cam-ui` stays free of dialog crates and filesystem-handle
  ownership beyond the load it already performs for import, so `rfd` is a dependency of the **host** crate
  `cam-desktop` only; and the `untitled.c2d` label when no file has been opened.
- **S6 — risk.** Task Size item 5: this adds a dependency to the delivered binary. The app bar's action
  list is a pinned contract (`shell.rs:1977-1989`) and moves with a test update, on purpose.

## Tasks

- T1 — the capability in `cam-ui` (S2, S3, S4): the action, the load, the three resets, the name in the
  bar, and its tests. Route: delegated worker. Commit: `c70c1c4`.
- T2 — the host's dialog (S1, S2, S5): `rfd` in `cam-desktop`, the Open intent answered with a path.
  Route: parent inline. Commit: `c70c1c4`.
- T3 — the documents (S1, S2, S5): `README.md`, the two shell records, and `docs/` — which turned out to
  need nothing, because nothing in it was stale about this (checked, not assumed). Route: delegated
  worker. Commit: the commit that carries this line.
- T4 — gate, commits, native review (S6). Route: parent. Commit: `c70c1c4` plus the record commits.

## Log

- **L1 (the request, verbatim).** *"vale"*, in answer to the parent's *"Si querés, arranco por el de
  `.c2d`, que es aditivo y no cambia ningún contrato documentado"*, and the chosen surface, verbatim:
  *"Diálogo nativo con rfd"*.
- **L2 (reconnaissance, before any write).** The load path is `DocumentFile::load`
  (`cam-store/src/document.rs:29`, archive scan `:40-64`, missing entry `:67-70`, `migrate()` `:74`,
  `validate()` `:75-78`), and the CLI's *private* `open` helper (`cam-cli/src/main.rs:218-223`, used by
  `validate`, `inspect`, `check`, `emit` and `simulate` — there is no `open` subcommand) adds only exit-code
  classification on top of it, so the window can reuse the loader without re-implementing validation.
  `AppState::replace_document` (`cam-app/src/lib.rs:147-154`) clears history and selection and sets
  `dirty = false`; the undoable variant (`:161-167`) is what import uses. `cam-ui` already depends on
  `cam-store` and already reads files for import (`cam-ui/src/lib.rs:2213-2218`), so the load belongs
  where the action is; `cam-desktop` does not depend on `cam-store` and does not need to.
  What survives a bare swap and must be reset: `Shell.preview` (`cam-ui/src/lib.rs:770`), `Shell.tools`
  (`:771-777`) and the material-size drafts seeded from `Job::default_sheet()` rather than the document
  (`:833-838`). Two tests pin the state this feature moves: the `untitled.c2d` literal
  (`cam-ui/tests/shell.rs:583`) and the app-bar action list (`:1977-1989`).
- **L3 (the two delegates, and what each one corrected).** The `cam-ui` unit came back green — 163 tests in
  the crate, `clippy -D warnings` and `fmt --check` clean — with seven new `shell.rs` tests and two pins
  moved on purpose (the app bar now reports `Undo, Redo, Open, Fit the job`, and the `untitled.c2d`
  assertion keeps its literal because it is still true with nothing open). Its honest limitation, worth
  keeping: for a brand-new public API the intended pre-implementation failure is a *compile* failure, not
  a per-assertion one, so the RED evidence is `variant not found in UiAction` rather than a failing
  assertion. It also disclosed a footgun instead of hiding it: `Shell::apply_action` is the only place
  that can see the shell, so a host calling `handle_action(OpenFile)` directly would get the swap and the
  history step but not the name or the resets — documented on the action, and the desktop host goes
  through `apply_action`, which is where this feature's calls land.
  The docs unit refused two things and was right on both: it would not cite a commit hash for code that was
  still uncommitted (it wrote "uncommitted in the working tree when this entry was written" instead), and
  it would not write `cam-cli open` as a command, because there is no `open` subcommand — it is a private
  helper used by `validate`, `inspect`, `check`, `emit` and `simulate`. That shorthand was in **this
  document** (S2, L2) and the parent fixed it here rather than leaving a false command in the
  specification. It also established that `odd/roadmap.md` needed no edit at all, because no row there
  ever said the editor could not open a document — this feature has no roadmap trace, only this document
  and two appended Log entries elsewhere.
- **L4 (the gate, and a dependency that is a decision).** **Exit 0, 96 suites, 1 184 passed, 0 failed** —
  the previous 1 177 plus exactly the seven added, with `fmt --check` and `clippy -D warnings` inside it.
  Two failures were paid before that and neither was a code defect: a `rustc` **ICE** (`the compiler
  unexpectedly panicked`) while rebuilding `epaint`, because `rfd`'s feature unification pulled it back
  in, and then an attempt that never started because every `fork` returned `Resource temporarily
  unavailable`. Both were process contention — a delegated docs worker was alive during them — and the ICE
  did not reproduce with the build alone (`Finished dev profile` in 51.29 s, compiling `epaint`,
  `rfd 0.15.4`, `ashpd 0.11.1`). The lesson is not "retry": this box runs one build at a time, and a
  delegate editing prose is not free.
  **What the dependency costs, stated rather than discovered later:** `rfd 0.15` with its default features
  adds ten packages to `Cargo.lock` — `ashpd`, `async-fs`, `async-net`, `futures-channel`, `pollster`,
  `ppv-lite86`, `rand`, `rand_chacha`, `rand_core`, `urlencoding` — needs no system libraries at build
  time, and needs a **desktop portal running** to show the dialog at runtime. That is the price of S1's
  choice, recorded here instead of buried in a lock file.
  **What the gate cannot cover:** the dialog itself. The window needs a display, which this project already
  states (`cam-desktop/src/main.rs`), so `rfd` returning a path is verified by building — and everything
  after it is verified headless through `apply_action`, the same path the button's answer takes.
- **L5 (the review: approved, no correction, four advisories).** Lineage **`review-327fd8b1f8d37057`**,
  target `sha256:7f3a69e5…`, tier **`high`** — the provider's own `process_boundary` signal, raised by the
  dialog call in `cam-ui/src/lib.rs` — lenses **`review-risk` + `review-reliability`**, 12 paths / 952
  changed lines, correction budget **200**. Result: **approved on the first admitted event**, no correction
  round, no blocker, budget unspent. Both reviewers ran over the Pi host relay (prompts 76 798 and 76 823
  bytes, results 2 472 and 3 329), and the authority is **burned** (`gentle-ai.review-acknowledged/v1`,
  consumed revision `sha256:9735c094…`).
  **Operational note worth keeping:** the first START was rejected as `lenses-invalid` with
  `lineage_created: false`, and the only thing that changed between that attempt and the accepted one was
  the length of `lensesReason`. Nothing was created by the rejected attempt, so nothing needed
  reconciling — but the lesson is that a long prose reason can cost a round trip.
  **Four advisories came back, all `informational`, none of them a correction**, and their text is not
  quoted here on purpose — it lives in the native receipt, and retyping a finding from memory is the
  failure the previous features' logs named. What can be said from the locations alone:
  `R1-001` (risk, WARNING) at `cam-ui/src/lib.rs:1013` and `R3-stale-document-path` (reliability, WARNING)
  at `:1014-1019` both point at the open bookkeeping block in `apply_action`;
  `R3-dirty-open-unproved` (SUGGESTION) at `tests/shell.rs:1781-2012` points at what the open tests do
  **not** cover; and `R3-extras-dropped` (SUGGESTION) at `lib.rs:2321-2329` points at the loader path
  S2 deliberately chose to keep the archive extras. Two of these are WARNINGs and they are not dressed
  down here: a future **Save** feature meets all four head-on, because they are about what a document
  carries beyond its model and what happens to it when it is swapped. A review that approved this
  candidate is still a review that has not seen a Save path, because there is none to see.
  **What this receipt does not authorise:** delivery. Push, pull request and merge stay ordinary
  repository policy and the owner's decisions.
