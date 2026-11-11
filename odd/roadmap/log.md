## 8. Log

- **L1 (user requirement).** A cross-platform 2.5D CAM application, planned through ODD: the request asked
  for that plan and its milestones, with Windows/Linux support named as the extra requirement. The owner's
  sentence also named a third-party product; the repository name hygiene sweep
  (`odd/tasks/028-repository-name-hygiene.md`) removed that name and the words that went with it.
- **L2 (user decisions, verbatim option labels):** "Rust + wgpu/egui (o Rust + Qt bindings)";
  "Todo menos 3D y Laser"; "Formato propio abierto (JSON/ZIP) + import DXF/SVG/PDF/EPS";
  "Implementar nesting heurístico propio en v1".
- **L3 (analysis):** the technical analysis that preceded this plan is folded into the rows above; the
  dossier it was written to was deleted by the name-hygiene sweep, which records that in its own Log.
- **L4 (plan):** this document. Next executable artifact is
  `odd/tasks/001-m0-foundation.md`.
- **L5 (after M10, two features and no milestone).** M0–M10 were the whole plan, so the work that followed
  started from the debt the milestones left rather than from a plan row. **The advisory debt**
  (`odd/tasks/021-debt-advisories.md`): six advisories closed — four with the reviewer's own words recovered
  from a sibling lineage, plus `1ab52143/R1-STEP-DOS` (the search step is now bounded by
  `MAX_SEARCH_ANCHORS`, four million, the same ceiling the simulator's cell grid and the medial axis's
  lattice use) and `R3-CAP-BOUNDARY` (the cap's inclusive edge, proved by installing the mutated
  comparison) — ten unrecoverable because the review store keeps a receipt's lineage and not its findings.
  **The script panel's mount** (`odd/tasks/022-script-panel-mount.md`): `ScriptPanel` is drawn as S9's modal
  from a palette command, `Esc` closes it, and a test pins that the frame's canvas is byte-identical with
  it open and closed — the recorded claim that the wiring "would move every existing shell golden" was
  measured and is false, since this crate keeps no pixel goldens at all. Six advisories were carried in
  and four were answered: three by the fixes above and one — a **BLOCKER filed against
  `egui::Id::unique`** by the mount's refuter — by contradicting it with evidence, because the constructor
  exists (`egui-0.37.0/src/id.rs:91`) and the spelling it demanded is deprecated. That authority was
  **abandoned** rather than left open: `operator_disposition` by the owner, recorded as
  `gentle-ai.review-reclaim-record/v1` on 2026-11-05 and moved to
  `review-transactions/quarantine/review-5eb75a623cbfc72e-441271739`, because no correction could satisfy
  the finding without failing `clippy -D warnings` (`odd/tasks/022-script-panel-mount.md`, L6-L8).
- **L6 (after M10, the third feature: the panel edits the shell's document).**
  `odd/tasks/023-script-panel-document.md`, delivered in three commits — `63e7aa4` (the change),
  `451d586` (its review) and `5332383` (`R4-001`) — which close the residual the mount left open: a script
  run from the panel used to edit a file on disk and reach the document only when the file was reopened.
  The owner chose the shape in two decisions, both quoted verbatim in that document: the panel runs against
  the shell's document ("El panel edita el documento del shell") and the mutations live in memory, with the
  file written only by the shell's Save ("Sólo memoria; el archivo lo escribe Save"). `run_script_on` takes
  the caller's `&mut Document` and the whole run is one transaction; `open` and `save` are refused on the
  line that tries; the shell commits through `AppState::replace_document_undoable`, and only when the run
  changed something. Evidence: gate green at 1526 tests, `cam-ui` shell 128 and panel 8, `cam-cli` 26, and a
  differential of `cam-cli script` — a command the request never named — over eleven scripts against the
  previous tree with stdout, stderr, exit code and the written document identical. Its native review,
  lineage `review-cf16da5a0b71e7b8` over `7cb6fcd..63e7aa4`, closed **approved** on the last admitted event
  with zero of its 200-line correction budget spent; the receipt is burned. Its six advisories are all
  informational — `R4-001` was closed in its own candidate once its anchor reproduced the edge — and their
  wording did not survive the store, which is the blindness L5 above already names, one review later.
- **L7 (the blindness, measured and answered).** Four losses of the same kind — M9's, M10's, the debt
  review's own `R3-001` and L6's six — stopped being an anecdote and became a document:
  `odd/reports/review-findings-survival.md`. It measures what each contract leaves behind in
  `.git/gentle-ai` — a `v2` lineage keeps a 4.6–23.9 KB `review-state.json` with the admitted artifacts and
  their prose inside, a `compact-v2` lineage keeps a **259-byte** terminal-consumption record and nothing
  else, rejected results are the only ones written to disk, and an abandoned authority keeps more than an
  approved one. It also records the three changes that would end the loss — as a record, not a report: this
  repository is not a fork of that tooling and has no upstream to file against. The second half is binding
  here:
  findings are written down in the session that receives them **before** the receipt is burned, an inferred
  subject is marked as an inference, no finding is closed by a plausible-looking edit, and the quarantine
  residue is read before any finding is declared unreadable.

